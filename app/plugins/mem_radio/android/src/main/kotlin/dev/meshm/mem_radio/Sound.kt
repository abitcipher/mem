package dev.meshm.mem_radio

import android.annotation.SuppressLint
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder

/**
 * Plays and records 16-bit mono PCM for pairing by sound. MEM's Rust side turns the
 * contact code into sound and back; this class only moves samples. The microphone runs only while
 * the sound pairing screen is open.
 */
internal class Sound(private val events: Events) {
    interface Events {
        fun soundIn(data: ByteArray)
    }

    @Volatile private var recording = false
    private var recorder: Thread? = null
    /** The frame playing now, so [stop] can cut it short. */
    @Volatile private var playing: AudioTrack? = null

    /** Plays `pcm` once on the loudspeaker; calls `done` when it finished (or failed). */
    fun play(pcm: ByteArray, done: (Boolean) -> Unit) {
        Thread {
            val ok = try {
                val track = AudioTrack.Builder()
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                            .build(),
                    )
                    .setAudioFormat(
                        AudioFormat.Builder()
                            .setSampleRate(SAMPLE_RATE)
                            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                            .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                            .build(),
                    )
                    .setTransferMode(AudioTrack.MODE_STATIC)
                    .setBufferSizeInBytes(pcm.size)
                    .build()
                try {
                    track.write(pcm, 0, pcm.size)
                    playing = track
                    track.play()
                    // Static tracks report no end: wait for the samples, plus a margin.
                    Thread.sleep(pcm.size / 2 * 1000L / SAMPLE_RATE + 150)
                    synchronized(this) {
                        if (playing === track) playing = null
                        track.stop()
                    }
                    true
                } finally {
                    synchronized(this) { track.release() }
                }
            } catch (_: Exception) {
                false
            }
            done(ok)
        }.start()
    }

    /** Starts streaming microphone samples in chunks to [Events.soundIn]. */
    @SuppressLint("MissingPermission") // checked by the plugin before calling
    fun startRecording(): Boolean {
        if (recording) return true
        val min = AudioRecord.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (min <= 0) return false
        val record = try {
            // Voice recognition: no automatic gain or noise suppression on most phones, which
            // would otherwise smear the tones.
            AudioRecord(
                MediaRecorder.AudioSource.VOICE_RECOGNITION,
                SAMPLE_RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                maxOf(min, CHUNK * 4),
            )
        } catch (_: Exception) {
            return false
        }
        if (record.state != AudioRecord.STATE_INITIALIZED) {
            record.release()
            return false
        }
        recording = true
        recorder = Thread {
            try {
                record.startRecording()
                val buf = ByteArray(CHUNK)
                while (recording) {
                    val n = record.read(buf, 0, buf.size)
                    if (n > 0) events.soundIn(buf.copyOf(n))
                }
                record.stop()
            } catch (_: Exception) {
            } finally {
                record.release()
            }
        }.also { it.start() }
        return true
    }

    /** Stops the microphone and cuts short the frame playing now. */
    fun stop() {
        synchronized(this) {
            try {
                playing?.stop()
            } catch (_: IllegalStateException) {
            }
            playing = null
        }
        recording = false
        recorder?.join(500)
        recorder = null
    }

    companion object {
        const val SAMPLE_RATE = 48_000
        /** 4096 bytes: about 43 ms of sound per event. */
        const val CHUNK = 4096
    }
}
