package com.homehub.net

import org.junit.Assert.*
import org.junit.Test

class Blake3Test {
    @Test fun standardVectors() {
        assertEquals("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", Blake3.hashHex(byteArrayOf()))
        assertEquals("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85", Blake3.hashHex("abc".toByteArray()))
    }
    @Test fun streamingTreeSurvivesChunkAndTransferBoundaries() {
        val bytes = ByteArray(4 * 1024 * 1024 + 1025) { (it % 251).toByte() }
        for (stride in listOf(63, 1024, 65537, 4 * 1024 * 1024)) {
            val stream = Blake3.Hasher()
            var offset = 0
            while (offset < bytes.size) {
                val size = minOf(stride, bytes.size - offset)
                stream.update(bytes, offset, size); offset += size
            }
            assertArrayEquals(Blake3.hash(bytes), stream.digest())
            assertThrows(IllegalStateException::class.java) { stream.update(byteArrayOf(1)) }
        }
    }
}
