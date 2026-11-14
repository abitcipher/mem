package dev.meshm.mem

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.DocumentsContract
import android.provider.Settings
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterEngineCache
import io.flutter.embedding.engine.dart.DartExecutor
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    // Without this lock Android filters incoming multicast, so mDNS discovery of nearby MEM
    // devices (cla-iroh) never sees anyone. Held only while the app is in the
    // foreground: MEM has no background service yet.
    private var multicastLock: WifiManager.MulticastLock? = null

    // One system-picker request at a time (backup files and the backup folder go through
    // the Storage Access Framework, so MEM needs no storage permission).
    private var pending: MethodChannel.Result? = null
    private var pendingBytes: ByteArray? = null

    // Tells Dart that InboundActivity left text for it.
    private var inboundChannel: MethodChannel? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        multicastLock = wifi?.createMulticastLock("mem-mdns")?.apply { setReferenceCounted(false) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.action == InboundActivity.ACTION_INBOUND) {
            inboundChannel?.invokeMethod("inboundText", null)
        }
    }

    override fun onResume() {
        super.onResume()
        multicastLock?.acquire()
    }

    override fun onPause() {
        multicastLock?.release()
        super.onPause()
    }

    // One engine for the whole process: the Dart app and the MEM node it owns keep
    // running when the activity is closed, as long as the process lives (ReachableService keeps it
    // alive when the user turned that on). Reopening the app attaches to the same engine.
    override fun provideFlutterEngine(context: Context): FlutterEngine =
        FlutterEngineCache.getInstance().get(ENGINE)
            ?: FlutterEngine(context.applicationContext).also {
                it.dartExecutor.executeDartEntrypoint(DartExecutor.DartEntrypoint.createDefault())
                FlutterEngineCache.getInstance().put(ENGINE, it)
            }

    override fun shouldDestroyEngineWithHost(): Boolean = false

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "mem/platform")
            .setMethodCallHandler(::onCall)
        inboundChannel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "mem/inbound")
    }

    private fun batteryUnrestricted(): Boolean {
        val power = getSystemService(Context.POWER_SERVICE) as PowerManager
        return power.isIgnoringBatteryOptimizations(packageName)
    }

    // Messaging is a core use for this exemption; the user confirms it in a system dialog.
    @SuppressLint("BatteryLife")
    private fun requestBatteryUnrestricted() {
        if (batteryUnrestricted()) return
        startActivity(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                .setData(Uri.parse("package:$packageName")),
        )
    }

    // Android 13+: without this the service still runs, but its notification is hidden.
    private fun askForNotifications() {
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQ_NOTIFY)
        }
    }

    private fun onCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            // The recovery phrase screens: no screenshots, no screen recording,
            // hidden in the recent-apps view.
            "setSecure" -> {
                if (call.arguments == true) {
                    window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                } else {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                }
                result.success(null)
            }
            // A QR code on screen (cla-qr): the brightness the user chose (full by
            // default, so the other camera sees contrast through glare; lower when that camera
            // washes the code out), and no screen timeout during a long transfer. The argument is
            // the brightness from 0.05 to 1, or null when the code is gone. Both are window
            // settings, so Android undoes them when MEM leaves the foreground.
            "setShowingCode" -> {
                val level = (call.arguments as? Number)?.toFloat()?.coerceIn(0.05f, 1f)
                window.attributes = window.attributes.apply {
                    screenBrightness = level ?: WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
                }
                if (level != null) {
                    window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                } else {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }
                result.success(null)
            }
            // Background reachability.
            "setReachable" -> {
                if (call.argument<Boolean>("on") == true) {
                    askForNotifications()
                    ReachableService.start(
                        applicationContext,
                        call.argument<String>("title") ?: "MEM",
                        call.argument<String>("text") ?: "",
                        call.argument<Boolean>("ble") == true,
                    )
                } else {
                    ReachableService.stop(applicationContext)
                }
                result.success(null)
            }
            // Push wake-ups post a notification when messages arrived.
            "askNotifications" -> {
                askForNotifications()
                result.success(null)
            }
            // Text shared into MEM, once.
            "takeInboundText" -> result.success(InboundActivity.take())
            // The first installed Telegram app, for "Send via Telegram".
            "telegramApp" -> result.success(TELEGRAM.firstOrNull(::installed))
            "shareTo" -> result.success(
                shareTo(call.argument<String>("package"), call.argument<String>("text")),
            )
            "batteryUnrestricted" -> result.success(batteryUnrestricted())
            // This build's name and number, shown in Settings.
            "appVersion" -> {
                val info = packageManager.getPackageInfo(packageName, 0)
                val code = if (Build.VERSION.SDK_INT >= 28) {
                    info.longVersionCode
                } else {
                    @Suppress("DEPRECATION")
                    info.versionCode.toLong()
                }
                result.success(mapOf("name" to info.versionName, "code" to code))
            }
            "requestBatteryUnrestricted" -> {
                requestBatteryUnrestricted()
                result.success(null)
            }
            "saveDocument" -> startPicker(result, REQ_SAVE, call.argument<ByteArray>("bytes")) {
                Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "application/octet-stream"
                    putExtra(Intent.EXTRA_TITLE, call.argument<String>("name"))
                }
            }
            "openDocument" -> startPicker(result, REQ_OPEN, null) {
                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                }
            }
            "pickFolder" -> startPicker(result, REQ_FOLDER, null) {
                Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            }
            // Files shared with contacts: picked once, read when a contact asks.
            "pickShareFiles" -> startPicker(result, REQ_SHARE_FILES, null) {
                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                }
            }
            "pickShareFolder" -> startPicker(result, REQ_SHARE_FOLDER, null) {
                Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            }
            "copyShared" -> {
                val uri = Uri.parse(call.argument<String>("uri"))
                val path = call.argument<String>("path") ?: ""
                val limit = call.argument<Number>("limit")?.toLong() ?: 0L
                val resolver = applicationContext.contentResolver
                // Off the main thread: a shared file can be 100 MiB.
                Thread {
                    val size = SharedFiles.copy(resolver, uri, path, limit)
                    runOnUiThread { result.success(size) }
                }.start()
            }
            "writeToFolder" -> result.success(
                writeToFolder(
                    Uri.parse(call.argument<String>("folder")),
                    call.argument<String>("name") ?: "backup.membackup",
                    call.argument<ByteArray>("bytes") ?: ByteArray(0),
                ),
            )
            else -> result.notImplemented()
        }
    }

    private fun startPicker(
        result: MethodChannel.Result,
        request: Int,
        bytes: ByteArray?,
        intent: () -> Intent,
    ) {
        if (pending != null) {
            result.error("busy", "another picker is open", null)
            return
        }
        pending = result
        pendingBytes = bytes
        startActivityForResult(intent(), request)
    }

    @Deprecated("Activity Result API is not used by FlutterActivity")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val result = pending ?: return
        val bytes = pendingBytes
        pending = null
        pendingBytes = null
        if (requestCode == REQ_SHARE_FILES) {
            if (resultCode != Activity.RESULT_OK || data == null) {
                result.success(null)
                return
            }
            try {
                result.success(SharedFiles.describePicked(contentResolver, data))
            } catch (e: Exception) {
                result.error("io", e.javaClass.simpleName, null)
            }
            return
        }
        val uri = data?.data
        if (resultCode != Activity.RESULT_OK || uri == null) {
            result.success(null)
            return
        }
        try {
            when (requestCode) {
                REQ_SAVE -> {
                    contentResolver.openOutputStream(uri, "wt")?.use { it.write(bytes ?: ByteArray(0)) }
                    result.success(true)
                }
                REQ_OPEN -> {
                    val read = contentResolver.openInputStream(uri)?.use { input ->
                        // Bounded like the decoder: a backup is at most 64 MiB.
                        val out = java.io.ByteArrayOutputStream()
                        val buf = ByteArray(64 * 1024)
                        var total = 0L
                        while (true) {
                            val n = input.read(buf)
                            if (n < 0) break
                            total += n
                            if (total > MAX_FILE) return@use null
                            out.write(buf, 0, n)
                        }
                        out.toByteArray()
                    }
                    result.success(read)
                }
                REQ_SHARE_FOLDER -> result.success(SharedFiles.describeTree(contentResolver, uri))
                REQ_FOLDER -> {
                    contentResolver.takePersistableUriPermission(
                        uri,
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
                    )
                    result.success(uri.toString())
                }
                else -> result.success(null)
            }
        } catch (e: Exception) {
            result.error("io", e.javaClass.simpleName, null)
        }
    }

    private fun installed(pkg: String): Boolean = try {
        packageManager.getPackageInfo(pkg, 0)
        true
    } catch (e: PackageManager.NameNotFoundException) {
        false
    }

    // Straight to one app's share target; only to the Telegram apps MEM knows.
    private fun shareTo(pkg: String?, text: String?): Boolean {
        if (pkg !in TELEGRAM || text == null) return false
        return try {
            startActivity(
                Intent(Intent.ACTION_SEND)
                    .setType("text/plain")
                    .setPackage(pkg)
                    .putExtra(Intent.EXTRA_TEXT, text),
            )
            true
        } catch (e: Exception) {
            false
        }
    }

    private fun writeToFolder(tree: Uri, name: String, bytes: ByteArray): Boolean = try {
        val parent = DocumentsContract.buildDocumentUriUsingTree(
            tree,
            DocumentsContract.getTreeDocumentId(tree),
        )
        val doc = DocumentsContract.createDocument(
            contentResolver,
            parent,
            "application/octet-stream",
            name,
        )
        if (doc == null) {
            false
        } else {
            contentResolver.openOutputStream(doc, "wt")?.use { it.write(bytes) }
            true
        }
    } catch (e: Exception) {
        false
    }

    private companion object {
        const val REQ_SAVE = 4101
        const val REQ_OPEN = 4102
        const val REQ_FOLDER = 4103
        const val MAX_FILE = 64L * 1024 * 1024 + 128
        const val REQ_NOTIFY = 4104
        const val REQ_SHARE_FILES = 4105
        const val REQ_SHARE_FOLDER = 4106
        const val ENGINE = "mem"
        val TELEGRAM = listOf(
            "org.telegram.messenger",
            "org.telegram.messenger.web",
            "org.thunderdog.challegram",
        )
    }
}
