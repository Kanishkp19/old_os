package com.homehub.net

import org.junit.Assert.*
import org.junit.Test

class QrPayloadTest {
    private val prefix = "homehub://pair?h=home&t=abcdefghijklmnopqrstuv&a=192.168.1.3%3A47802&"
    @Test fun fullFingerprintOverridesLegacyPrefix() {
        val full = "a".repeat(64)
        assertEquals(full, QrPayload.parse(prefix + "fp=" + "b".repeat(16) + "&fp_sha256=$full")?.caFingerprint)
        assertNotNull(QrPayload.parse(prefix + "fp=" + "a".repeat(16)))
    }
    @Test fun malformedAndNonLocalPairingLinksAreRejected() {
        assertNull(QrPayload.parse(prefix + "fp_sha256=invalid&fp=" + "a".repeat(16)))
        assertNull(QrPayload.parse(prefix + "fp=%XX"))
        assertNull(QrPayload.parse(prefix.replace("192.168.1.3", "8.8.8.8") + "fp=" + "a".repeat(16)))
        assertNull(QrPayload.parse(prefix.replace("homehub://pair?", "homehub://pair-rogue?") + "fp=" + "a".repeat(16)))
    }
}
