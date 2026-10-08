package com.homehub.screen

import org.junit.Assert.*
import org.junit.Test

class LanSdpTest {
    @Test fun onlyNumericPrivateHostCandidatesLeaveDevice() {
        val sdp = "v=0\r\na=candidate:1 1 UDP 123 192.168.1.2 50100 typ host\r\n" +
            "a=candidate:2 1 UDP 123 8.8.8.8 50100 typ host\r\n" +
            "a=candidate:3 1 UDP 123 10.0.0.1 50100 typ srflx\r\n" +
            "a=candidate:4 1 UDP 123 other.local 50100 typ host\r\n"
        val sent = LanSdp.hostOnly(sdp)
        assertTrue(sent.contains("192.168.1.2"))
        assertFalse(sent.contains("8.8.8.8")); assertFalse(sent.contains("other.local")); assertFalse(sent.contains("srflx"))
    }
    @Test fun missingLanCandidateFailsClosed() {
        assertThrows(IllegalArgumentException::class.java) { LanSdp.hostOnly("v=0\r\n") }
        assertFalse(LanSdp.privateAddress("127.0.0.1")); assertFalse(LanSdp.privateAddress("224.0.0.1"))
        assertFalse(LanSdp.privateAddress("example.com")); assertTrue(LanSdp.privateAddress("fd01::1"))
    }
}
