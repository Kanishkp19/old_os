package com.homehub.net

import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.X509TrustManager

/** Only a valid leaf signed directly by the scanned Home CA is accepted. */
class PinnedCaTrustManager(private val ca: X509Certificate) : X509TrustManager {
    override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) =
        throw CertificateException("Client trust is not available")
    override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
        if (chain.isEmpty()) throw CertificateException("Missing Home certificate")
        ca.checkValidity()
        if (ca.basicConstraints < 0) throw CertificateException("Home trust anchor is not a CA")
        chain[0].checkValidity()
        chain[0].verify(ca.publicKey)
        if (chain[0].issuerX500Principal != ca.subjectX500Principal)
            throw CertificateException("Home certificate issuer mismatch")
        val purposes = chain[0].extendedKeyUsage
        if (purposes != null && "1.3.6.1.5.5.7.3.1" !in purposes)
            throw CertificateException("Home certificate is not valid for server use")
    }
    override fun getAcceptedIssuers() = arrayOf(ca)
    companion object {
        fun verifyFingerprint(ca: X509Certificate, expected: String) {
            if (!expected.matches(Regex("[a-fA-F0-9]{16}|[a-fA-F0-9]{64}")))
                throw CertificateException("Invalid Home fingerprint")
            val actual = MessageDigest.getInstance("SHA-256").digest(ca.encoded)
                .joinToString("") { "%02x".format(it) }.take(expected.length)
            if (!MessageDigest.isEqual(actual.toByteArray(), expected.lowercase().toByteArray()))
                throw CertificateException("Home identity does not match the scanned code")
        }
    }
}
