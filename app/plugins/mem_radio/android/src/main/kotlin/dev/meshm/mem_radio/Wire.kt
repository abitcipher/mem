package dev.meshm.mem_radio

import java.security.SecureRandom
import java.util.UUID

/**
 * Everything MEM puts on the air or in its GATT service. Kept free of Android types so
 * it is unit-tested on the JVM.
 */
object Wire {
    /** The one fixed MEM service UUID: advertised, and the GATT service. */
    val SERVICE: UUID = UUID.fromString("94c2c50e-1936-4bfa-bddb-c2f28ccb9dd8")

    /** Readable: the peripheral's L2CAP PSM as two bytes, big-endian; 0 when it has none. */
    val PSM: UUID = UUID.fromString("0d7fac2f-e2a0-4fd4-93c5-f9ecb5c90524")

    /** GATT fallback, central → peripheral stream bytes (write with response). */
    val RX: UUID = UUID.fromString("5d3e74f8-3134-461f-b1ef-62e949c4035f")

    /** GATT fallback, peripheral → central stream bytes (notify). */
    val TX: UUID = UUID.fromString("a39b41f7-24dc-4174-a69d-ebd7031acb1d")

    /** Client Characteristic Configuration descriptor (Bluetooth SIG). */
    val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

    /** Link nonce length. */
    const val NONCE_LEN = 4

    /** A new link nonce at least this often. */
    const val NONCE_ROTATE_MS = 15L * 60 * 1000

    /** Largest ATT attribute value. */
    const val MAX_ATTR = 512

    private val random = SecureRandom()

    fun newNonce(): ByteArray = ByteArray(NONCE_LEN).also { random.nextBytes(it) }

    /**
     * Which side connects: the one with the smaller nonce, compared as unsigned
     * big-endian numbers. `null` when equal: neither connects, and the caller draws a new nonce.
     */
    fun weConnect(ours: ByteArray, theirs: ByteArray): Boolean? {
        for (i in 0 until NONCE_LEN) {
            val a = ours[i].toInt() and 0xff
            val b = theirs[i].toInt() and 0xff
            if (a != b) return a < b
        }
        return null
    }

    /**
     * The advertisement's content, and nothing else: the service UUID in the advertising data
     * and the nonce as that service's data in the scan response. No
     * name, no TX power, no manufacturer data. The two halves fit one legacy packet each:
     * flags 3 + UUID list 18 bytes, and service data 22 bytes.
     */
    data class Advert(val serviceUuid: UUID, val serviceData: Pair<UUID, ByteArray>) {
        val includesName get() = false
        val includesTxPower get() = false
    }

    fun advert(nonce: ByteArray): Advert {
        require(nonce.size == NONCE_LEN)
        return Advert(SERVICE, SERVICE to nonce.copyOf())
    }

    /** The nonce from a scan record's service data, if it is a MEM advertisement. */
    fun nonceOf(serviceData: ByteArray?): ByteArray? =
        serviceData?.takeIf { it.size == NONCE_LEN }?.copyOf()

    /** The PSM characteristic's value. */
    fun psmBytes(psm: Int): ByteArray =
        if (psm in 1..0xffff) byteArrayOf((psm shr 8).toByte(), psm.toByte()) else byteArrayOf(0, 0)

    fun psmOf(value: ByteArray?): Int =
        if (value == null || value.size != 2) 0
        else ((value[0].toInt() and 0xff) shl 8) or (value[1].toInt() and 0xff)

    /** GATT chunk size for an ATT MTU: `MTU − 3`, at most one attribute value. */
    fun chunkSize(mtu: Int): Int = (mtu - 3).coerceIn(20, MAX_ATTR)

    /** Cuts stream bytes into GATT chunks; the ATT layer keeps their order. */
    fun chunks(bytes: ByteArray, mtu: Int): List<ByteArray> {
        val size = chunkSize(mtu)
        return (bytes.indices step size).map { bytes.copyOfRange(it, minOf(it + size, bytes.size)) }
    }
}
