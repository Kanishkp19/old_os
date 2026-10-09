package com.homehub.net

import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import org.junit.Assert.fail
import org.junit.Test

class PinnedCaTrustManagerTest {
    @Test fun clientOnlyCertificateCannotImpersonateHomeServer() {
        val stream = requireNotNull(javaClass.getResourceAsStream("/pairing-certs.pem"))
        val certs = stream.use {
            CertificateFactory.getInstance("X.509").generateCertificates(it)
                .map { cert -> cert as X509Certificate }
        }
        val manager = PinnedCaTrustManager(certs[0])
        manager.checkServerTrusted(arrayOf(certs[1]), "RSA")
        try {
            manager.checkServerTrusted(arrayOf(certs[2]), "RSA")
            fail("A client-authentication certificate must not identify the Home server")
        } catch (_: CertificateException) {
            // A paired device cannot use its client certificate as the pairing endpoint.
        }
    }
}
