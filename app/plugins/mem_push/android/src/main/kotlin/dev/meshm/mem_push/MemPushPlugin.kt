package dev.meshm.mem_push

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import org.unifiedpush.android.connector.UnifiedPush

/**
 * Flutter side of push wake-ups.
 *
 * Calls from Dart: `distributors`, `register(distributor)`, `unregister`, `endpoint`,
 * `takePendingWake`, `wakeDone`, `notify(title, text)`. Calls to Dart: `endpoint(url)` when the
 * distributor gives a new endpoint (or `null`), and `wake` when a wake-up arrives.
 */
class MemPushPlugin : FlutterPlugin, MethodChannel.MethodCallHandler {
    private lateinit var context: Context
    private var channel: MethodChannel? = null

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        context = binding.applicationContext
        channel = MethodChannel(binding.binaryMessenger, "mem_push").also {
            it.setMethodCallHandler(this)
        }
        Wake.plugin = this
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel?.setMethodCallHandler(null)
        channel = null
        if (Wake.plugin === this) Wake.plugin = null
    }

    fun endpointChanged(url: String?) {
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            channel?.invokeMethod("endpoint", url)
        }
    }

    fun wake() {
        channel?.invokeMethod("wake", null)
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "distributors" -> result.success(UnifiedPush.getDistributors(context))
            "register" -> {
                call.argument<String>("distributor")?.let { UnifiedPush.saveDistributor(context, it) }
                UnifiedPush.register(context, messageForDistributor = "MEM")
                result.success(null)
            }
            "unregister" -> {
                UnifiedPush.unregister(context)
                Wake.saveEndpoint(context, null)
                result.success(null)
            }
            "endpoint" -> result.success(Wake.endpoint(context))
            "takePendingWake" -> {
                val p = Wake.pending
                Wake.pending = false
                result.success(p)
            }
            "wakeDone" -> {
                Wake.finished()
                result.success(null)
            }
            "notify" -> {
                notify(call.argument<String>("title") ?: "MEM", call.argument<String>("text") ?: "")
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** A notification without names or text: "new messages", nothing more. */
    private fun notify(title: String, text: String) {
        if (Build.VERSION.SDK_INT >= 26) {
            context.getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(MESSAGES, title, NotificationManager.IMPORTANCE_DEFAULT),
            )
        }
        val launch = context.packageManager.getLaunchIntentForPackage(context.packageName)
        val open = launch?.let {
            PendingIntent.getActivity(
                context,
                0,
                it,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }
        val n = NotificationCompat.Builder(context, MESSAGES)
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(open)
            .setAutoCancel(true)
            .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
            .build()
        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION, n)
        } catch (_: SecurityException) {
            // Notifications not allowed: the messages are there when the app opens.
        }
    }

    private companion object {
        const val MESSAGES = "mem-messages"
        const val NOTIFICATION = 3
    }
}
