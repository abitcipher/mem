package dev.meshm.mem

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.Manifest
import android.content.pm.PackageManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

/**
 * "Stay reachable in the background": keeps the MEM process, and so the Rust node and
 * its Wi-Fi encounters, running with the screen off.
 *
 * - A foreground service with a permanent, quiet notification: Android neither freezes nor
 *   kills the process, and Doze keeps its network access.
 * - A partial wake lock: the CPU stays up, so QUIC keep-alives (every 3 s) continue
 *   and peers do not time out.
 * - A Wi-Fi lock: the radio does not drop into power save with the screen off.
 * - A multicast lock: mDNS keeps finding nearby MEM devices (MainActivity holds its own lock only
 *   while in the foreground).
 *
 * With Bluetooth on in MEM the service also declares the `connectedDevice` type,
 * so Bluetooth encounters go on with the screen off.
 *
 * The notification shows anyone who sees the phone that MEM is running; the setting says so.
 */
class ReachableService : Service() {
    private var wake: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null
    private var multicast: WifiManager.MulticastLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // A restart by the system brings no intent: keep the text from the last start.
        val prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        intent?.getStringExtra(EXTRA_TITLE)?.let { prefs.edit().putString(EXTRA_TITLE, it).apply() }
        intent?.getStringExtra(EXTRA_TEXT)?.let { prefs.edit().putString(EXTRA_TEXT, it).apply() }
        if (intent?.hasExtra(EXTRA_BLE) == true) {
            prefs.edit().putBoolean(EXTRA_BLE, intent.getBooleanExtra(EXTRA_BLE, false)).apply()
        }
        val notification = notification(
            prefs.getString(EXTRA_TITLE, null) ?: "MEM",
            prefs.getString(EXTRA_TEXT, null) ?: "Reachable over Wi-Fi",
        )
        if (Build.VERSION.SDK_INT >= 34) {
            // Android 14 refuses the connectedDevice type without a Bluetooth permission.
            val ble = prefs.getBoolean(EXTRA_BLE, false) &&
                checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) ==
                PackageManager.PERMISSION_GRANTED
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING or
                    (if (ble) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0),
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        acquireLocks()
        // Restarted by the system after a kill; the app re-attaches when the user opens it.
        return START_STICKY
    }

    override fun onDestroy() {
        releaseLocks()
        super.onDestroy()
    }

    private fun acquireLocks() {
        val power = getSystemService(Context.POWER_SERVICE) as PowerManager
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        if (wake == null) {
            wake = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mem:reachable").apply {
                setReferenceCounted(false)
                acquire()
            }
        }
        if (wifi == null && wifiManager != null) {
            @Suppress("DEPRECATION")
            val mode = if (Build.VERSION.SDK_INT >= 29) {
                WifiManager.WIFI_MODE_FULL_LOW_LATENCY
            } else {
                WifiManager.WIFI_MODE_FULL_HIGH_PERF
            }
            wifi = wifiManager.createWifiLock(mode, "mem:reachable").apply {
                setReferenceCounted(false)
                acquire()
            }
        }
        if (multicast == null && wifiManager != null) {
            multicast = wifiManager.createMulticastLock("mem:reachable-mdns").apply {
                setReferenceCounted(false)
                acquire()
            }
        }
    }

    private fun releaseLocks() {
        wake?.takeIf { it.isHeld }?.release()
        wifi?.takeIf { it.isHeld }?.release()
        multicast?.takeIf { it.isHeld }?.release()
        wake = null
        wifi = null
        multicast = null
    }

    private fun notification(title: String, text: String): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val builder = if (Build.VERSION.SDK_INT >= 26) {
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, title, NotificationManager.IMPORTANCE_MIN).apply {
                    setShowBadge(false)
                },
            )
            Notification.Builder(this, CHANNEL)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this).setPriority(Notification.PRIORITY_MIN)
        }
        return builder
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setShowWhen(false)
            .build()
    }

    companion object {
        private const val CHANNEL = "mem-reachable"
        private const val NOTIFICATION_ID = 1
        private const val PREFS = "mem-reachable"
        const val EXTRA_TITLE = "title"
        const val EXTRA_TEXT = "text"
        const val EXTRA_BLE = "ble"

        /** Starts or updates the service with localized notification text. */
        fun start(context: Context, title: String, text: String, ble: Boolean) {
            val intent = Intent(context, ReachableService::class.java)
                .putExtra(EXTRA_TITLE, title)
                .putExtra(EXTRA_TEXT, text)
                .putExtra(EXTRA_BLE, ble)
            if (Build.VERSION.SDK_INT >= 26) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, ReachableService::class.java))
        }
    }
}
