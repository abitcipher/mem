package dev.meshm.mem_push

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.work.ForegroundInfo
import androidx.work.Worker
import androidx.work.WorkerParameters

/** One wake-up: let the app fetch from its hub, at most [Wake.MAX_WAKE_MS]. */
class WakeWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        Wake.run(applicationContext)
        return Result.success()
    }

    // Expedited work on Android 11 and older runs as a foreground service with this notice.
    override fun getForegroundInfo(): ForegroundInfo {
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "MEM", NotificationManager.IMPORTANCE_MIN),
            )
        }
        val n = NotificationCompat.Builder(applicationContext, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("MEM")
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .build()
        // Only called on Android 11 and older, where no service type is required.
        return ForegroundInfo(ID, n)
    }

    private companion object {
        const val CHANNEL = "mem-wake"
        const val ID = 2
    }
}
