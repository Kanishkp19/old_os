package com.homehub.net

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class DownloadIntegrityTest {
    @Test fun resumableRangeHasExactStartAndTotal() {
        assertEquals(4194304L, DownloadIntegrity.rangeStart("bytes 4194304-8388607/8388608", 8388608))
    }
    @Test fun changedObjectOrMalformedRangeCannotBeAppended() {
        listOf("bytes 4-9/11", "bytes 8-7/10", "bytes 4-10/10", "bytes */10", null).forEach {
            assertThrows(Exception::class.java) { DownloadIntegrity.rangeStart(it, 10) }
        }
    }
}
