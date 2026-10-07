package com.homehub.net

import android.content.Context
import com.homehub.queue.HubTrustStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.Socket
import java.security.KeyStore
import java.security.Principal
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import javax.inject.Inject
import javax.inject.Singleton
import javax.net.ssl.SSLEngine
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509ExtendedKeyManager
import javax.net.ssl.X509TrustManager

/**
 * mTLS client (API_SPEC §1). Device key lives in AndroidKeyStore (never in
 * the DB — BACKEND_SCHEMA §9). The Hub CA from pairing is the only trust
 * anchor (T3: TOFU via QR fingerprint).
 */
@Singleton
class HubClient @Inject constructor(
    @ApplicationContext private val context: Context,
    private val trustStore: HubTrustStore,
) {
    @Volatile
    private var cachedClient: OkHttpClient? = null

    private fun createSslContext(caCert: X509Certificate, pinnedTrustManager: X509TrustManager): SSLContext {
        val prefs = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE)
        val clientCertPem = prefs.getString("client_cert_pem", null)
            ?: throw IllegalStateException("Client certificate not found. Please pair with the Hub first.")

        val cf = CertificateFactory.getInstance("X.509")
        val clientCert = cf.generateCertificate(clientCertPem.byteInputStream()) as X509Certificate

        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val privateKey = (ks.getKey("homehub-device", null) as? PrivateKey)
            ?: throw IllegalStateException("Device private key not found in AndroidKeyStore. Please re-pair.")

        try {
            val testSig = java.security.Signature.getInstance("NONEwithECDSA")
            testSig.initSign(privateKey)
        } catch (e: Exception) {
            throw IllegalStateException("Device key cannot sign with NONEwithECDSA (requires re-pairing with Hub): ${e.message}")
        }

        val alias = "homehub-device"
        val customKeyManager = object : X509ExtendedKeyManager() {
            override fun getClientAliases(keyType: String?, issuers: Array<out Principal>?): Array<String> =
                arrayOf(alias)

            override fun chooseClientAlias(keyType: Array<out String>?, issuers: Array<out Principal>?, socket: Socket?): String =
                alias

            override fun chooseEngineClientAlias(keyType: Array<out String>?, issuers: Array<out Principal>?, engine: SSLEngine?): String =
                alias

            override fun getServerAliases(keyType: String?, issuers: Array<out Principal>?): Array<String>? =
                null

            override fun chooseServerAlias(keyType: String?, issuers: Array<out Principal>?, socket: Socket?): String? =
                null

            override fun chooseEngineServerAlias(keyType: String?, issuers: Array<out Principal>?, engine: SSLEngine?): String? =
                null

            override fun getCertificateChain(alias: String?): Array<X509Certificate> =
                arrayOf(clientCert, caCert)

            override fun getPrivateKey(alias: String?): PrivateKey =
                privateKey
        }

        return SSLContext.getInstance("TLSv1.3").apply {
            init(arrayOf(customKeyManager), arrayOf<TrustManager>(pinnedTrustManager), SecureRandom())
        }
    }

    @Synchronized
    private fun client(): OkHttpClient {
        cachedClient?.let { return it }

        val trust = runBlocking(Dispatchers.IO) { trustStore.load() }
            ?: throw IllegalStateException("not paired")
        val cf = CertificateFactory.getInstance("X.509")
        val caCert = cf.generateCertificate(trust.caCertPem.byteInputStream()) as X509Certificate

        val pinnedTrustManager = object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<X509Certificate>?, authType: String?) {}
            override fun checkServerTrusted(chain: Array<X509Certificate>?, authType: String?) {
                if (chain.isNullOrEmpty()) throw CertificateException("Empty certificate chain")
                val leaf = chain[0]
                try {
                    leaf.verify(caCert.publicKey)
                } catch (e: Exception) {
                    var verified = false
                    for (c in chain) {
                        try {
                            c.verify(caCert.publicKey)
                            verified = true
                            break
                        } catch (_: Exception) {}
                    }
                    if (!verified) {
                        throw CertificateException("Server certificate does not chain to pinned CA: ${e.message}", e)
                    }
                }
            }
            override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf(caCert)
        }

        val ssl = createSslContext(caCert, pinnedTrustManager)

        val newClient = OkHttpClient.Builder()
            .sslSocketFactory(ssl.socketFactory, pinnedTrustManager)
            .hostnameVerifier { _, _ -> true } // LAN: identity is the pinned CA, not DNS names
            .build()
        cachedClient = newClient
        return newClient
    }

    fun resetClient() {
        synchronized(this) {
            cachedClient = null
        }
    }

    fun baseUrl(): String {
        val trust = runBlocking(Dispatchers.IO) { trustStore.load() } ?: throw IllegalStateException("not paired")
        val host = trust.lastAddr.substringBefore(':')
        return "https://$host:47800"
    }

    suspend fun info(): JSONObject = get("/v1/info")

    suspend fun get(path: String): JSONObject = withContext(Dispatchers.IO) {
        val resp = client().newCall(Request.Builder().url(baseUrl() + path).get().build()).execute()
        if (!resp.isSuccessful) error("GET $path -> ${resp.code}")
        JSONObject(resp.body!!.string())
    }

    suspend fun createTransfer(
        name: String, size: Long, mime: String?, kind: String,
        clientItemId: String, rootHash: String, takenAt: Long?,
    ): JSONObject = withContext(Dispatchers.IO) {
        val body = JSONObject().apply {
            put("name", name); put("size", size); put("kind", kind)
            put("client_item_id", clientItemId); put("root_hash", rootHash)
            mime?.let { put("mime", it) }; takenAt?.let { put("taken_at", it) }
            put("chunk_size", 4 * 1024 * 1024)
        }
        val resp = client().newCall(
            Request.Builder().url(baseUrl() + "/v1/transfers")
                .post(body.toString().toRequestBody("application/json".toMediaType())).build()
        ).execute()
        if (!resp.isSuccessful) error("create transfer -> ${resp.code}")
        JSONObject(resp.body!!.string())
    }

    suspend fun putChunk(transferId: String, idx: Long, bytes: ByteArray, hashHex: String): Int =
        withContext(Dispatchers.IO) {
            val resp = client().newCall(
                Request.Builder()
                    .url(baseUrl() + "/v1/transfers/$transferId/chunks/$idx")
                    .put(bytes.toRequestBody("application/octet-stream".toMediaType()))
                    .header("X-Chunk-Hash", hashHex)
                    .build()
            ).execute()
            resp.code
        }

    suspend fun complete(transferId: String, rootHash: String): JSONObject = withContext(Dispatchers.IO) {
        val body = JSONObject().put("root_hash", rootHash)
        val resp = client().newCall(
            Request.Builder().url(baseUrl() + "/v1/transfers/$transferId/complete")
                .post(body.toString().toRequestBody("application/json".toMediaType())).build()
        ).execute()
        if (!resp.isSuccessful) error("complete -> ${resp.code}: ${resp.body?.string()}")
        JSONObject(resp.body!!.string())
    }

    suspend fun transferStatus(transferId: String): JSONObject = get("/v1/transfers/$transferId")

    suspend fun devices(): JSONArray = get("/v1/devices").getJSONArray("devices")

    suspend fun revokeDevice(deviceId: String): Boolean = withContext(Dispatchers.IO) {
        // API_SPEC §7: DELETE /v1/devices/{id} (admin scope).
        val resp = client().newCall(
            Request.Builder().url(baseUrl() + "/v1/devices/$deviceId").delete().build()
        ).execute()
        resp.isSuccessful
    }

    /**
     * Remote-input WebSocket (API_SPEC §9). Messages are JSON InputMsg frames;
     * the Hub enforces a 500 msg/s rate limit and a 60 s idle timeout, so the
     * client coalesces pointer moves before sending.
     */
    fun openRemoteInput(listener: okhttp3.WebSocketListener): okhttp3.WebSocket {
        val req = Request.Builder()
            .url(baseUrl().replaceFirst("https://", "wss://") + "/v1/remote/input")
            .build()
        return client().newWebSocket(req, listener)
    }
}
