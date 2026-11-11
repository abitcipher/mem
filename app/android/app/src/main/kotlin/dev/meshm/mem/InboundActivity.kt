package dev.meshm.mem

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Takes text another app hands to MEM ("Share → MEM", or MEM in the selected-text
 * menu), keeps it for the Dart side and brings MEM to the front. It shows nothing itself.
 */
class InboundActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = when (intent?.action) {
            Intent.ACTION_SEND -> intent.getCharSequenceExtra(Intent.EXTRA_TEXT)
            Intent.ACTION_PROCESS_TEXT -> intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)
            else -> null
        }?.toString()
        // Bounded: an armored bundle is at most ~88 KiB of text.
        if (text != null && text.isNotBlank() && text.length <= MAX_INBOUND) {
            pending = text
            startActivity(
                Intent(this, MainActivity::class.java)
                    .setAction(ACTION_INBOUND)
                    .addFlags(
                        Intent.FLAG_ACTIVITY_NEW_TASK or
                            Intent.FLAG_ACTIVITY_CLEAR_TOP or
                            Intent.FLAG_ACTIVITY_SINGLE_TOP,
                    ),
            )
        }
        finish()
    }

    companion object {
        const val ACTION_INBOUND = "dev.meshm.mem.INBOUND"
        const val MAX_INBOUND = 256 * 1024

        /** The text waiting for Dart, taken once. */
        @Volatile
        var pending: String? = null

        fun take(): String? = pending.also { pending = null }
    }
}
