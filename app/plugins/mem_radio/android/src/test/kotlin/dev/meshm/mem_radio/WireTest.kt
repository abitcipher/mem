package dev.meshm.mem_radio

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WireTest {
    private fun b(vararg v: Int) = ByteArray(v.size) { v[it].toByte() }

    @Test
    fun smallerNonceConnects() {
        assertEquals(true, Wire.weConnect(b(0, 0, 0, 1), b(0, 0, 0, 2)))
        assertEquals(false, Wire.weConnect(b(0, 0, 0, 2), b(0, 0, 0, 1)))
        // Unsigned: 0x80 is larger than 0x7f.
        assertEquals(true, Wire.weConnect(b(0x7f, 0xff, 0xff, 0xff), b(0x80, 0, 0, 0)))
        assertEquals(false, Wire.weConnect(b(0xff, 0, 0, 0), b(0x01, 0, 0, 0)))
        assertNull(Wire.weConnect(b(1, 2, 3, 4), b(1, 2, 3, 4)))
    }

    /** The advertisement holds the service UUID and the nonce, nothing else. */
    @Test
    fun advertisementCarriesOnlyUuidAndNonce() {
        val nonce = b(9, 8, 7, 6)
        val ad = Wire.advert(nonce)
        assertEquals(Wire.SERVICE, ad.serviceUuid)
        assertEquals(Wire.SERVICE, ad.serviceData.first)
        assertArrayEquals(nonce, ad.serviceData.second)
        assertFalse(ad.includesName)
        assertFalse(ad.includesTxPower)
        // Flags (3) + 128-bit UUID list (2 + 16) in the advertising data; service data
        // (2 + 16 + 4) in the scan response: each within a 31-byte legacy packet.
        assertTrue(3 + 18 <= 31)
        assertTrue(2 + 16 + Wire.NONCE_LEN <= 31)
    }

    @Test
    fun noncesAreFreshAndParsed() {
        val a = Wire.newNonce()
        assertEquals(Wire.NONCE_LEN, a.size)
        assertArrayEquals(a, Wire.nonceOf(a))
        assertNull(Wire.nonceOf(null))
        assertNull(Wire.nonceOf(b(1, 2, 3)))
        assertNull(Wire.nonceOf(b(1, 2, 3, 4, 5)))
    }

    @Test
    fun psmRoundTrips() {
        assertEquals(0x0081, Wire.psmOf(Wire.psmBytes(0x0081)))
        assertEquals(0xffff, Wire.psmOf(Wire.psmBytes(0xffff)))
        assertEquals(0, Wire.psmOf(Wire.psmBytes(0)))
        assertEquals(0, Wire.psmOf(Wire.psmBytes(-1)))
        assertEquals(0, Wire.psmOf(null))
        assertEquals(0, Wire.psmOf(b(1)))
    }

    /** GATT fallback: `ATT MTU − 3` chunks that join back to the stream. */
    @Test
    fun chunksJoinBack() {
        val bytes = ByteArray(1000) { it.toByte() }
        for (mtu in listOf(23, 185, 247, 517, 1000)) {
            val chunks = Wire.chunks(bytes, mtu)
            assertTrue(chunks.all { it.size <= Wire.chunkSize(mtu) })
            assertTrue(chunks.all { it.size <= 512 })
            val joined = chunks.fold(ByteArray(0)) { acc, c -> acc + c }
            assertArrayEquals(bytes, joined)
        }
        assertEquals(20, Wire.chunkSize(23))
        assertEquals(512, Wire.chunkSize(517))
        assertEquals(0, Wire.chunks(ByteArray(0), 23).size)
    }
}
