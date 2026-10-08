package com.homehub.net

object DeviceIdentity {
    /**
     * Minimal PKCS#10 CSR builder (DER, ECDSA P-256 / SHA-256).
     * Real apps may use BouncyCastle; this keeps the binary small.
     */
    fun csr(kp: java.security.KeyPair, cn: String): String {
        // CertificationRequestInfo ::= SEQUENCE { version, subject, spki, attributes[0] }
        val cnOid = byteArrayOf(0x06, 0x03, 0x55, 0x04, 0x03)
        val cnVal = der(0x0C, cn.toByteArray()) // UTF8String
        val rdn = der(0x30, cnOid + cnVal)
        val subject = der(0x30, der(0x31, rdn))
        // SPKI for EC P-256: alg id + bitstring(point)
        val algEc = byteArrayOf(0x06, 0x07, 0x2A, 0x86.toByte(), 0x48, 0xCE.toByte(), 0x3D, 0x02, 0x01)
        val algP256 = byteArrayOf(0x06, 0x08, 0x2A, 0x86.toByte(), 0x48, 0xCE.toByte(), 0x3D, 0x03, 0x01, 0x07)
        val algSeq = der(0x30, algEc + algP256)

        val spki = kp.public.encoded
        val attrs = der(0xA0.toByte(), byteArrayOf())
        val cri = der(0x30, byteArrayOf(0x02, 0x01, 0x00) + subject + spki + attrs)
        // Sign with SHA256withECDSA via JCA (AndroidKeyStore key).
        val sig = java.security.Signature.getInstance("SHA256withECDSA")
        sig.initSign(kp.private)
        sig.update(cri)
        val sigBytes = sig.sign()
        val sigAlg = byteArrayOf(0x06, 0x08, 0x2A, 0x86.toByte(), 0x48, 0xCE.toByte(), 0x3D, 0x04, 0x03, 0x02)
        val csr = der(0x30, cri + der(0x30, sigAlg) + der(0x03, byteArrayOf(0x00) + sigBytes))
        val b64 = android.util.Base64.encodeToString(csr, android.util.Base64.NO_WRAP)
        return "-----BEGIN CERTIFICATE REQUEST-----\n" + b64.chunked(64).joinToString("\n") + "\n-----END CERTIFICATE REQUEST-----\n"
    }

    private fun der(tag: Byte, content: ByteArray): ByteArray {
        val len = if (content.size < 128) {
            byteArrayOf(content.size.toByte())
        } else {
            if (content.size < 256) byteArrayOf(0x81.toByte(), content.size.toByte())
            else byteArrayOf(0x82.toByte(), (content.size shr 8).toByte(), content.size.toByte())
        }
        return byteArrayOf(tag) + len + content
    }

}
