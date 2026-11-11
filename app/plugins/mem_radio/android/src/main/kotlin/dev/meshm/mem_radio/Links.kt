package dev.meshm.mem_radio

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothSocket
import android.os.Build
import android.os.Handler
import java.io.IOException
import java.util.ArrayDeque
import java.util.concurrent.LinkedBlockingQueue

/**
 * One connection to a nearby MEM device, carrying a byte stream both ways. The
 * stream's framing is Rust's business (`cla-ble`); a link only keeps the bytes in order.
 *
 * Every method runs on the radio's worker thread, except the L2CAP reader and writer threads,
 * which post back to it.
 */
internal abstract class Link(val id: Long, val address: String) {
    /** Queues stream bytes; [Radio.Events.sent] reports `op` when written or failed. */
    abstract fun send(op: Long, bytes: ByteArray)

    /** Closes the connection. Unfinished writes are reported failed. */
    abstract fun close()
}

/** Whatever a link needs from the radio. */
internal interface LinkHost {
    val worker: Handler
    fun received(link: Link, bytes: ByteArray)
    fun sent(op: Long, ok: Boolean)
    fun lost(link: Link)
}

/** L2CAP connection-oriented channel: a plain socket stream (Android 10+). */
internal class L2capLink(
    id: Long,
    address: String,
    private val socket: BluetoothSocket,
    private val host: LinkHost,
    /** The GATT connection the central used to read the PSM; kept open with the channel. */
    private val gatt: BluetoothGatt?,
) : Link(id, address) {
    private val queue = LinkedBlockingQueue<Pair<Long, ByteArray>>()

    @Volatile
    private var closed = false

    private val reader = Thread({
        val buf = ByteArray(4096)
        try {
            val input = socket.inputStream
            while (!closed) {
                val n = input.read(buf)
                if (n < 0) break
                if (n > 0) {
                    val piece = buf.copyOf(n)
                    host.worker.post { host.received(this, piece) }
                }
            }
        } catch (_: IOException) {
        }
        host.worker.post { host.lost(this) }
    }, "mem-l2cap-read")

    private val writer = Thread({
        try {
            val out = socket.outputStream
            while (!closed) {
                val (op, bytes) = queue.take()
                if (op < 0) break
                try {
                    out.write(bytes)
                    out.flush()
                    host.worker.post { host.sent(op, true) }
                } catch (_: IOException) {
                    host.worker.post { host.sent(op, false) }
                    break
                }
            }
        } catch (_: InterruptedException) {
        }
        host.worker.post { host.lost(this) }
    }, "mem-l2cap-write")

    fun startThreads() {
        reader.start()
        writer.start()
    }

    override fun send(op: Long, bytes: ByteArray) {
        if (closed) host.sent(op, false) else queue.put(op to bytes)
    }

    @SuppressLint("MissingPermission")
    override fun close() {
        if (closed) return
        closed = true
        try {
            socket.close()
        } catch (_: IOException) {
        }
        gatt?.disconnect()
        gatt?.close()
        while (true) {
            val (op, _) = queue.poll() ?: break
            host.sent(op, false)
        }
        // Wakes a writer blocked on an empty queue.
        queue.put(-1L to ByteArray(0))
    }
}

/** A queue of writes cut into GATT chunks, one chunk in flight at a time. */
private class ChunkQueue(private val host: LinkHost) {
    private class Out(val op: Long, val chunks: List<ByteArray>, var next: Int = 0)

    private val queue = ArrayDeque<Out>()
    var inFlight = false
        private set

    fun add(op: Long, chunks: List<ByteArray>) {
        if (chunks.isEmpty()) host.sent(op, true) else queue.add(Out(op, chunks))
    }

    /** The chunk to write now, if any and none is in flight. */
    fun next(): ByteArray? {
        if (inFlight) return null
        val head = queue.peek() ?: return null
        inFlight = true
        return head.chunks[head.next]
    }

    /** The chunk in flight was written (or not). Returns false on failure. */
    fun done(ok: Boolean): Boolean {
        inFlight = false
        val head = queue.peek() ?: return ok
        if (!ok) return false
        head.next++
        if (head.next == head.chunks.size) {
            queue.poll()
            host.sent(head.op, true)
        }
        return true
    }

    fun failAll() {
        while (true) host.sent((queue.poll() ?: break).op, false)
        inFlight = false
    }
}

/** GATT fallback, central side: writes to RX with response, reads TX notifications. */
@SuppressLint("MissingPermission")
internal class GattClientLink(
    id: Long,
    address: String,
    private val gatt: BluetoothGatt,
    private val rx: BluetoothGattCharacteristic,
    private val mtu: Int,
    private val host: LinkHost,
) : Link(id, address) {
    private val queue = ChunkQueue(host)
    private var closed = false

    override fun send(op: Long, bytes: ByteArray) {
        if (closed) {
            host.sent(op, false)
            return
        }
        queue.add(op, Wire.chunks(bytes, mtu))
        pump()
    }

    private fun pump() {
        val chunk = queue.next() ?: return
        val started = if (Build.VERSION.SDK_INT >= 33) {
            gatt.writeCharacteristic(rx, chunk, BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT) ==
                BluetoothGatt.GATT_SUCCESS
        } else {
            @Suppress("DEPRECATION")
            rx.value = chunk
            rx.writeType = BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
            @Suppress("DEPRECATION")
            gatt.writeCharacteristic(rx)
        }
        if (!started) {
            queue.done(false)
            host.lost(this)
        }
    }

    /** `onCharacteristicWrite` for RX. */
    fun written(ok: Boolean) {
        if (queue.done(ok)) pump() else host.lost(this)
    }

    override fun close() {
        if (closed) return
        closed = true
        queue.failAll()
        gatt.disconnect()
        gatt.close()
    }
}

/** GATT fallback, peripheral side: notifies TX, receives RX writes. */
@SuppressLint("MissingPermission")
internal class GattServerLink(
    id: Long,
    address: String,
    private val server: BluetoothGattServer,
    val device: BluetoothDevice,
    private val tx: BluetoothGattCharacteristic,
    private val mtu: Int,
    private val host: LinkHost,
) : Link(id, address) {
    private val queue = ChunkQueue(host)
    private var closed = false

    override fun send(op: Long, bytes: ByteArray) {
        if (closed) {
            host.sent(op, false)
            return
        }
        queue.add(op, Wire.chunks(bytes, mtu))
        pump()
    }

    private fun pump() {
        val chunk = queue.next() ?: return
        val started = if (Build.VERSION.SDK_INT >= 33) {
            server.notifyCharacteristicChanged(device, tx, false, chunk) ==
                BluetoothGatt.GATT_SUCCESS
        } else {
            // The value is copied by the call; this thread is the only writer.
            @Suppress("DEPRECATION")
            tx.value = chunk
            @Suppress("DEPRECATION")
            server.notifyCharacteristicChanged(device, tx, false)
        }
        if (!started) {
            queue.done(false)
            host.lost(this)
        }
    }

    /** `onNotificationSent` for this device. */
    fun notified(ok: Boolean) {
        if (queue.done(ok)) pump() else host.lost(this)
    }

    override fun close() {
        if (closed) return
        closed = true
        queue.failAll()
        server.cancelConnection(device)
    }
}
