package com.homehub.net

import android.content.Context
import com.homehub.queue.HubTrustStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.ensureActive
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
        val privateKey = (ks.getKey(prefs.getString("key_alias", "homehub-device"), null) as? PrivateKey)
            ?: throw IllegalStateException("Device private key not found in AndroidKeyStore. Please re-pair.")

        try {
            val testSig = java.security.Signature.getInstance("NONEwithECDSA")
            testSig.initSign(privateKey)
        } catch (e: Exception) {
            throw IllegalStateException("Device key cannot sign with NONEwithECDSA (requires re-pairing with Hub): ${e.message}")
        }

        val alias = prefs.getString("key_alias", "homehub-device")!!
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

        val pinnedTrustManager = PinnedCaTrustManager(caCert)

        val ssl = createSslContext(caCert, pinnedTrustManager)

        val newClient = OkHttpClient.Builder()
            .sslSocketFactory(ssl.socketFactory, pinnedTrustManager)
            .hostnameVerifier { _, _ -> true } // Identity is the QR-pinned CA
            .followRedirects(false).followSslRedirects(false)
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
        return "https://${trust.lastAddr}"
    }

    class ApiException(val code: Int) : java.io.IOException("Home request failed ($code)")

    suspend fun info(): JSONObject = get("/v1/info")
    suspend fun request(method: String, path: String, body: Any? = null): String = withContext(Dispatchers.IO) {
        require(path.startsWith("/v1/") && !path.contains(".."))
        val requestBody = body?.toString()?.toRequestBody("application/json".toMediaType())
        val actualBody = if (method == "POST" || method == "PATCH")
            requestBody ?: "{}".toRequestBody("application/json".toMediaType()) else requestBody
        client().newCall(Request.Builder().url(baseUrl() + path).method(method, actualBody).build())
            .execute().use { response ->
                if (!response.isSuccessful) throw ApiException(response.code)
                response.body?.string().orEmpty()
            }
    }
    suspend fun get(path: String) = JSONObject(request("GET", path))
    suspend fun post(path: String, body: Any = JSONObject()) = JSONObject(request("POST", path, body).ifBlank { "{}" })
    suspend fun patch(path: String, body: JSONObject) { request("PATCH", path, body) }
    suspend fun delete(path: String) { request("DELETE", path) }
    suspend fun bytes(path: String): ByteArray = withContext(Dispatchers.IO) {
        client().newCall(Request.Builder().url(baseUrl() + path).get().build()).execute().use {
            if (!it.isSuccessful) throw ApiException(it.code)
            it.body?.bytes() ?: byteArrayOf()
        }
    }
    /** Stream into an app-private partial file; callers export only after verification. */
    suspend fun download(fileId: String, target: File): JSONObject = withContext(Dispatchers.IO) {
        val meta = get("/v1/files/$fileId")
        val hasher = Blake3.Hasher()
        var size = 0L
        client().newCall(Request.Builder().url(baseUrl() + "/v1/files/$fileId/content").get().build())
            .execute().use { response ->
                if (!response.isSuccessful) throw ApiException(response.code)
                requireNotNull(response.body).byteStream().use { input ->
                    target.outputStream().use { output ->
                        val buffer = ByteArray(256 * 1024)
                        while (true) {
                            kotlinx.coroutines.currentCoroutineContext().ensureActive()
                            val n = input.read(buffer)
                            if (n < 0) break
                            output.write(buffer, 0, n); hasher.update(buffer, 0, n); size += n
                        }
                        output.fd.sync()
                    }
                }
            }
        require(size == meta.getLong("size") && hasher.digestHex() == meta.getString("hash")) {
            "The downloaded copy did not pass verification"
        }
        meta
    }
    suspend fun createTransfer(name: String, size: Long, mime: String?, kind: String,
        clientItemId: String, rootHash: String, takenAt: Long?, backupSourceId: String? = null): JSONObject =
        post("/v1/transfers", JSONObject().apply {
            put("name", name); put("size", size); put("kind", kind)
            put("client_item_id", clientItemId); put("root_hash", rootHash)
            put("chunk_size", 4 * 1024 * 1024)
            mime?.let { put("mime", it) }; takenAt?.let { put("taken_at", it) }
            backupSourceId?.let { put("backup_source_id", it) }
        })
    suspend fun putChunk(transferId: String, idx: Long, bytes: ByteArray, hashHex: String): Int = withContext(Dispatchers.IO) {
        client().newCall(Request.Builder().url(baseUrl() + "/v1/transfers/$transferId/chunks/$idx")
            .put(bytes.toRequestBody("application/octet-stream".toMediaType()))
            .header("X-Chunk-Hash", hashHex).build()).execute().use { it.code }
    }
    suspend fun complete(transferId: String, rootHash: String) =
        post("/v1/transfers/$transferId/complete", JSONObject().put("root_hash", rootHash))
    suspend fun transferStatus(transferId: String) = get("/v1/transfers/$transferId")
    suspend fun devices(): JSONArray {
        val raw = request("GET", "/v1/devices")
        return if (raw.trim().startsWith("[")) JSONArray(raw) else JSONObject(raw).let {
            it.optJSONArray("devices") ?: it.getJSONArray("items")
        }
    }
    suspend fun revokeDevice(deviceId: String): Boolean { delete("/v1/devices/$deviceId"); return true }
    fun openRemoteInput(listener: okhttp3.WebSocketListener): okhttp3.WebSocket =
        client().newWebSocket(Request.Builder().url(baseUrl().replaceFirst("https://", "wss://") + "/v1/remote/input").build(), listener)
}
