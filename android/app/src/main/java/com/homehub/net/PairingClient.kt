package com.homehub.net

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.net.URLEncoder
import java.security.KeyPairGenerator
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import java.security.MessageDigest
import javax.inject.Inject
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

data class QrPayload(
    val hubId: String, val token: String, val caFingerprint: String,
    val addrs: List<String>, val name: String,
) {
    companion object {
        fun parse(raw: String): QrPayload? {
            if (!raw.startsWith("homehub://pair")) return null
            val q = raw.substringAfter('?')
            val params = q.split('&').mapNotNull {
                val kv = it.split('=', limit = 2)
                if (kv.size == 2) kv[0] to java.net.URLDecoder.decode(kv[1], "UTF-8") else null
            }.toMap()
            return QrPayload(
                hubId = params["h"] ?: return null,
                token = params["t"] ?: return null,
                caFingerprint = params["fp"] ?: return null,
                addrs = params["a"]?.split(',') ?: emptyList(),
                name = params["n"] ?: "Home Hub",
            )
        }
    }
}

/**
 * Pairing (TRD §5, SECURITY §5): the client generates its keypair, verifies
 * the Hub CA fingerprint from the QR BEFORE sending the one-time token (T3).
 */
class PairingClient @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    data class PairResult(
        val deviceId: String, val certPem: String, val caCertPem: String,
        val expiresAt: Long, val scopes: List<String>, val hubName: String,
    )

    suspend fun pair(payload: QrPayload): PairResult = withContext(Dispatchers.IO) {
        val addr = payload.addrs.firstOrNull() ?: error("no hub address in QR")

        // Bootstrap carries no token. Its certificate and CA are both verified
        // against the fingerprint physically scanned from the Hub.
        val bootstrapTrust = object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) = Unit
            override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) = Unit
            override fun getAcceptedIssuers() = emptyArray<X509Certificate>()
        }
        val bootstrapSsl = SSLContext.getInstance("TLSv1.3").apply {
            init(null, arrayOf<TrustManager>(bootstrapTrust), java.security.SecureRandom())
        }
        val bootstrap = OkHttpClient.Builder()
            .sslSocketFactory(bootstrapSsl.socketFactory, bootstrapTrust)
            .hostnameVerifier { _, _ -> true }
            .followRedirects(false).followSslRedirects(false).build()
        val cf = java.security.cert.CertificateFactory.getInstance("X.509")
        val caCert = bootstrap.newCall(Request.Builder().url("https://$addr/pair/ca").get().build())
            .execute().use { response ->
                require(response.isSuccessful) { "Unable to verify Home (${response.code})" }
                val pem = JSONObject(requireNotNull(response.body).string()).getString("ca_cert_pem")
                val ca = cf.generateCertificate(pem.byteInputStream()) as X509Certificate
                PinnedCaTrustManager.verifyFingerprint(ca, payload.caFingerprint)
                val leaf = response.handshake?.peerCertificates?.firstOrNull() as? X509Certificate
                    ?: error("Home did not provide a certificate")
                PinnedCaTrustManager(ca).checkServerTrusted(arrayOf(leaf), "EC")
                ca
            }
        bootstrap.connectionPool.evictAll()
        val pinning = PinnedCaTrustManager(caCert)
        val ssl = SSLContext.getInstance("TLSv1.3").apply {
            init(null, arrayOf<TrustManager>(pinning), java.security.SecureRandom())
        }
        val client = OkHttpClient.Builder().sslSocketFactory(ssl.socketFactory, pinning)
            .hostnameVerifier { _, _ -> true }.followRedirects(false).followSslRedirects(false).build()

        // Keep the current identity usable if an attempted re-pair fails.
        val alias = "homehub-device-${java.util.UUID.randomUUID()}"
        val ks = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val kpg = KeyPairGenerator.getInstance("EC", "AndroidKeyStore")
        kpg.initialize(android.security.keystore.KeyGenParameterSpec.Builder(
            alias, android.security.keystore.KeyProperties.PURPOSE_SIGN or
                android.security.keystore.KeyProperties.PURPOSE_VERIFY,
        ).setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(android.security.keystore.KeyProperties.DIGEST_NONE,
                android.security.keystore.KeyProperties.DIGEST_SHA256).build())
        val kp = kpg.generateKeyPair()
        try {
        // 3. Build a minimal PKCS#10 CSR (ECDSA P-256 + SHA-256).
        val csrPem = buildCsr(kp, "homehub")

        val body = JSONObject().apply {
            put("token", payload.token)
            put("device_name", android.os.Build.MODEL)
            put("platform", "android")
            put("model", android.os.Build.MODEL)
            put("app_version", "0.1.0")
            put("csr_pem", csrPem)
        }
        val json = client.newCall(Request.Builder().url("https://$addr/pair")
            .post(body.toString().toRequestBody("application/json".toMediaType())).build())
            .execute().use { response ->
                require(response.isSuccessful) { "Pairing failed (${response.code})" }
                JSONObject(requireNotNull(response.body).string())
            }
        val caCertPem = json.getString("ca_cert_pem")
        val issuedCa = cf.generateCertificate(caCertPem.byteInputStream()) as X509Certificate
        require(issuedCa.encoded.contentEquals(caCert.encoded)) { "Home identity changed during pairing" }
        val clientCertPem = json.getString("cert_pem")
        val issuedCert = cf.generateCertificate(clientCertPem.byteInputStream()) as X509Certificate
        issuedCert.verify(caCert.publicKey)
        issuedCert.checkValidity()
        require(issuedCert.publicKey.encoded.contentEquals(kp.public.encoded)) { "Incorrect device certificate" }
        val prefs = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE)
        val oldAlias = prefs.getString("key_alias", "homehub-device")
        require(prefs.edit().putString("client_cert_pem", clientCertPem)
            .putString("key_alias", alias).putString("device_id", json.getString("device_id")).commit())
        if (oldAlias != alias && oldAlias != null) runCatching { ks.deleteEntry(oldAlias) }

        PairResult(
            deviceId = json.getString("device_id"),
            certPem = clientCertPem,
            caCertPem = caCertPem,
            expiresAt = json.getLong("cert_expires_at"),
            scopes = json.getJSONArray("scopes").let { arr -> (0 until arr.length()).map { arr.getString(it) } },
            hubName = json.getJSONObject("hub").getString("name"),
        )
        } catch (e: Exception) {
            if (context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE).getString("key_alias", null) != alias)
                runCatching { ks.deleteEntry(alias) }
            throw e
        }
    }

    private fun sha256Hex(b: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(b).joinToString("") { "%02x".format(it) }

    private fun pemEncode(cert: X509Certificate): String {
        val b64 = android.util.Base64.encodeToString(cert.encoded, android.util.Base64.NO_WRAP)
        return "-----BEGIN CERTIFICATE-----\n" + b64.chunked(64).joinToString("\n") + "\n-----END CERTIFICATE-----\n"
    }

    /**
     * Minimal PKCS#10 CSR builder (DER, ECDSA P-256 / SHA-256).
     * Real apps may use BouncyCastle; this keeps the binary small.
     */
    private fun buildCsr(kp: java.security.KeyPair, cn: String): String {
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

    /** Extract the uncompressed EC point from an X.509 SPKI encoding. */
    private fun extractEcPoint(spki: ByteArray): ByteArray {
        // P-256 uncompressed point is 65 bytes starting with 0x04.
        val idx = spki.indexOf(0x04.toByte())
        return spki.copyOfRange(idx, idx + 65)
    }
}
