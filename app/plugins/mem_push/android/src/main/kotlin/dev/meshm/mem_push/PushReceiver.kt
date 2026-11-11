package dev.meshm.mem_push

import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

/**
 * Receives UnifiedPush events. The message content is ignored: every wake-up means
 * only "fetch now", and the hub sends the same byte each time.
 */
class PushReceiver : PushService() {
    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        Wake.saveEndpoint(this, endpoint.url)
        Wake.plugin?.endpointChanged(endpoint.url)
    }

    override fun onMessage(message: PushMessage, instance: String) {
        // Expedited work: allowed from the background and given time to run; one at a time.
        val work = OneTimeWorkRequestBuilder<WakeWorker>()
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .build()
        WorkManager.getInstance(this)
            .enqueueUniqueWork("mem-wake", ExistingWorkPolicy.KEEP, work)
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        Wake.plugin?.endpointChanged(null)
    }

    override fun onUnregistered(instance: String) {
        Wake.saveEndpoint(this, null)
        Wake.plugin?.endpointChanged(null)
    }
}
