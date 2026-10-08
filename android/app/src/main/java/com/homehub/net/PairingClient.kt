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
        fun parse(raw: String): QrPayload? = runCatching { parseValid(raw) }.getOrNull()
        private fun parseValid(raw: String): QrPayload? {
            if (!raw.startsWith("homehub://pair?") || raw.length > 8192) return null
            val q = raw.substringAfter('?')
            val params = q.split('&').mapNotNull {
                val kv = it.split('=', limit = 2)
                if (kv.size == 2) kv[0] to java.net.URLDecoder.decode(kv[1], "UTF-8") else null
            }.toMap()
            return QrPayload(
                hubId = (params["h"] ?: return null).takeIf { it.length in 1..128 } ?: return null,
                token = (params["t"] ?: return null).takeIf { it.matches(Regex("[A-Za-z0-9_-]{22}")) } ?: return null,
                caFingerprint = (params["fp_sha256"] ?: params["fp"] ?: return null).also {
                    if (!it.matches(Regex("[a-fA-F0-9]{16}|[a-fA-F0-9]{64}"))) return null
                },
                addrs = params["a"]?.split(',')?.filter { rawAddress ->
                    runCatching {
                        val address = java.net.URI("https://$rawAddress")
                        address.rawUserInfo == null && address.rawPath.orEmpty().isEmpty() && address.rawQuery == null &&
                            address.port == 47802 && com.homehub.screen.LanSdp.privateAddress(address.host.orEmpty().removePrefix("[").removeSuffix("]"))
                    }.getOrDefault(false)
                }?.takeIf { it.isNotEmpty() } ?: return null,
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
        val csrPem = DeviceIdentity.csr(kp, "homehub")

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
            .putString("key_alias", alias).putString("device_id", json.getString("device_id")).putString("hub_id", payload.hubId)
            .putString("scopes", json.getJSONArray("scopes").toString())
            .putLong("cert_expires_at", json.getLong("cert_expires_at"))
            .remove("renew_alias").remove("renew_csr").remove("renew_cert").commit())
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

}
