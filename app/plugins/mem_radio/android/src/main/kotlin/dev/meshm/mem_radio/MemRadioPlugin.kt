package dev.meshm.mem_radio

import android.Manifest
import android.app.Activity
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.location.LocationManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.embedding.engine.plugins.activity.ActivityAware
import io.flutter.embedding.engine.plugins.activity.ActivityPluginBinding
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.PluginRegistry

/**
 * Flutter side of the radio: `start/stop`, `send/close` per link, and an event
 * stream of links coming up, bytes, links going down, write results and radio states.
 */
class MemRadioPlugin :
    FlutterPlugin,
    ActivityAware,
    MethodChannel.MethodCallHandler,
    EventChannel.StreamHandler,
    PluginRegistry.RequestPermissionsResultListener {
    private lateinit var context: Context
    private lateinit var methods: MethodChannel
    private lateinit var eventChannel: EventChannel
    private val main = Handler(Looper.getMainLooper())
    private var sink: EventChannel.EventSink? = null
    private var radio: Radio? = null
    private var activity: ActivityPluginBinding? = null
    private var pendingPermissions: MethodChannel.Result? = null

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        context = binding.applicationContext
        methods = MethodChannel(binding.binaryMessenger, "mem_radio")
        methods.setMethodCallHandler(this)
        eventChannel = EventChannel(binding.binaryMessenger, "mem_radio/events")
        eventChannel.setStreamHandler(this)
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        sound.stop()
        radio?.stop()
        wifi?.stop()
        methods.setMethodCallHandler(null)
        eventChannel.setStreamHandler(null)
    }

    // ---- events ----

    override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
        sink = events
    }

    override fun onCancel(arguments: Any?) {
        sink = null
    }

    private fun emit(event: Map<String, Any>) {
        main.post { sink?.success(event) }
    }

    private val radioEvents = object : Radio.Events {
        override fun linkUp(link: Long) = emit(mapOf("type" to "up", "link" to link))
        override fun bytes(link: Long, data: ByteArray) =
            emit(mapOf("type" to "bytes", "link" to link, "data" to data))
        override fun linkDown(link: Long) = emit(mapOf("type" to "down", "link" to link))
        override fun sent(op: Long, ok: Boolean) = emit(mapOf("type" to "sent", "op" to op, "ok" to ok))
        override fun state(state: String) = emit(mapOf("type" to "state", "state" to state))
    }

    private fun radio(): Radio = radio ?: Radio(context, radioEvents).also { radio = it }

    // ---- Wi-Fi Direct ----

    private var wifi: WifiDirect? = null

    private val wifiEvents = object : WifiDirect.Events {
        override fun peer(id: String, ip: String, port: Int) =
            emit(mapOf("type" to "wifiPeer", "id" to id, "ip" to ip, "port" to port))
        override fun wifiState(state: String) = emit(mapOf("type" to "wifiState", "state" to state))
    }

    private fun wifi(): WifiDirect = wifi ?: WifiDirect(context, wifiEvents).also { wifi = it }

    private fun wifiSupported(): Boolean =
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_WIFI_DIRECT)

    private fun wifiPermissions(): Array<String> = if (Build.VERSION.SDK_INT >= 33) {
        arrayOf(Manifest.permission.NEARBY_WIFI_DEVICES)
    } else {
        arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
    }

    private fun wifiMissing(): List<String> = wifiPermissions().filter {
        context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED
    }

    // ---- sound: pairing cards only ----

    private val sound = Sound(object : Sound.Events {
        override fun soundIn(data: ByteArray) = emit(mapOf("type" to "soundIn", "data" to data))
    })

    private fun soundMissing(): List<String> = listOf(Manifest.permission.RECORD_AUDIO).filter {
        context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED
    }

    // ---- methods ----

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "status" -> result.success(status())
            "requestPermissions" -> requestPermissions(result, missingPermissions())
            "enableBluetooth" -> {
                val a = activity?.activity
                if (a == null || missingPermissions().isNotEmpty()) {
                    result.success(false)
                } else {
                    @Suppress("MissingPermission")
                    a.startActivity(Intent(BluetoothAdapter.ACTION_REQUEST_ENABLE))
                    result.success(true)
                }
            }
            "openLocationSettings" -> {
                activity?.activity?.startActivity(Intent(Settings.ACTION_LOCATION_SOURCE_SETTINGS))
                result.success(null)
            }
            "start" -> {
                if (missingPermissions().isNotEmpty()) {
                    result.success(false)
                } else {
                    radio().start(call.argument<Boolean>("lowPower") == true)
                    result.success(true)
                }
            }
            "stop" -> {
                radio?.stop()
                result.success(null)
            }
            "setLowPower" -> {
                radio?.setLowPower(call.arguments == true)
                result.success(null)
            }
            "send" -> {
                val link = call.argument<Number>("link")?.toLong()
                val op = call.argument<Number>("op")?.toLong()
                val bytes = call.argument<ByteArray>("bytes")
                if (link == null || op == null || bytes == null) {
                    result.error("args", "link, op and bytes are required", null)
                } else {
                    val r = radio
                    if (r == null) radioEvents.sent(op, false) else r.send(link, op, bytes)
                    result.success(null)
                }
            }
            "wifiStatus" -> result.success(
                mapOf(
                    "supported" to wifiSupported(),
                    "permissions" to wifiMissing().isEmpty(),
                    // Android 6–12 find Wi-Fi Direct peers only with Location on.
                    "locationNeeded" to (Build.VERSION.SDK_INT <= 32),
                    "locationOn" to (Build.VERSION.SDK_INT > 32 || locationOn()),
                    "running" to (wifi?.isRunning == true),
                ),
            )
            "wifiRequestPermissions" -> requestPermissions(result, wifiMissing())
            "soundRequestPermission" -> requestPermissions(result, soundMissing())
            "soundPlay" -> {
                val pcm = call.argument<ByteArray>("pcm")
                if (pcm == null) {
                    result.success(false)
                } else {
                    sound.play(pcm) { ok -> main.post { result.success(ok) } }
                }
            }
            "soundStart" -> result.success(soundMissing().isEmpty() && sound.startRecording())
            "soundStop" -> {
                sound.stop()
                result.success(null)
            }
            "wifiStart" -> {
                val id = call.argument<String>("id")
                val port = call.argument<Number>("port")?.toInt()
                if (!wifiSupported() || wifiMissing().isNotEmpty() || id == null || port == null) {
                    result.success(false)
                } else {
                    wifi().start(id, port)
                    result.success(true)
                }
            }
            "wifiStop" -> {
                wifi?.stop()
                result.success(null)
            }
            "wifiLeaveGroup" -> {
                wifi?.leaveGroup()
                result.success(null)
            }
            "close" -> {
                call.argument<Number>("link")?.toLong()?.let { radio?.close(it) }
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    private fun adapter(): BluetoothAdapter? =
        (context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager)?.adapter

    /** What the app needs to explain before BLE can run. */
    private fun status(): Map<String, Any> {
        val hasLe = context.packageManager.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)
        val adapter = adapter()
        val granted = missingPermissions().isEmpty()
        return mapOf(
            "supported" to (hasLe && adapter != null),
            "bluetoothOn" to (adapter?.isEnabled == true),
            "permissions" to granted,
            // Android 6–11 scan only with Location on.
            "locationNeeded" to (Build.VERSION.SDK_INT <= 30),
            "locationOn" to locationOn(),
            "running" to (radio?.isRunning == true),
        )
    }

    private fun locationOn(): Boolean {
        if (Build.VERSION.SDK_INT > 30) return true
        val lm = context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager ?: return false
        return if (Build.VERSION.SDK_INT >= 28) {
            lm.isLocationEnabled
        } else {
            @Suppress("DEPRECATION")
            Settings.Secure.getInt(
                context.contentResolver,
                Settings.Secure.LOCATION_MODE,
                Settings.Secure.LOCATION_MODE_OFF,
            ) != Settings.Secure.LOCATION_MODE_OFF
        }
    }

    private fun wantedPermissions(): Array<String> = if (Build.VERSION.SDK_INT >= 31) {
        arrayOf(
            Manifest.permission.BLUETOOTH_SCAN,
            Manifest.permission.BLUETOOTH_ADVERTISE,
            Manifest.permission.BLUETOOTH_CONNECT,
        )
    } else {
        arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
    }

    private fun missingPermissions(): List<String> = wantedPermissions().filter {
        context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED
    }

    private fun requestPermissions(result: MethodChannel.Result, missing: List<String>) {
        val a: Activity? = activity?.activity
        when {
            missing.isEmpty() -> result.success(true)
            a == null || pendingPermissions != null -> result.success(false)
            else -> {
                pendingPermissions = result
                a.requestPermissions(missing.toTypedArray(), REQUEST)
            }
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ): Boolean {
        if (requestCode != REQUEST) return false
        pendingPermissions?.success(
            grantResults.isNotEmpty() && grantResults.all { it == PackageManager.PERMISSION_GRANTED },
        )
        pendingPermissions = null
        return true
    }

    // ---- activity ----

    override fun onAttachedToActivity(binding: ActivityPluginBinding) {
        activity = binding
        binding.addRequestPermissionsResultListener(this)
    }

    override fun onDetachedFromActivityForConfigChanges() = onDetachedFromActivity()

    override fun onReattachedToActivityForConfigChanges(binding: ActivityPluginBinding) =
        onAttachedToActivity(binding)

    override fun onDetachedFromActivity() {
        activity?.removeRequestPermissionsResultListener(this)
        activity = null
        pendingPermissions?.success(false)
        pendingPermissions = null
    }

    private companion object {
        const val REQUEST = 4201
    }
}
