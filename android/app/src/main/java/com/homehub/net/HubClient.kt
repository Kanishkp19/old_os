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

    private fun createSslContext(caCert: X509Certificate, pinnedTrustManager: X509TrustManager, certOverride: String? = null, aliasOverride: String? = null): SSLContext {
        val prefs = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE)
        val clientCertPem = certOverride ?: prefs.getString("client_cert_pem", null)
            ?: throw IllegalStateException("Client certificate not found. Please pair with the Hub first.")

        val cf = CertificateFactory.getInstance("X.509")
        val clientCert = cf.generateCertificate(clientCertPem.byteInputStream()) as X509Certificate

        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val privateKey = (ks.getKey(aliasOverride ?: prefs.getString("key_alias", "homehub-device"), null) as? PrivateKey)
            ?: throw IllegalStateException("Device private key not found in AndroidKeyStore. Please re-pair.")

        try {
            val testSig = java.security.Signature.getInstance("NONEwithECDSA")
            testSig.initSign(privateKey)
        } catch (e: Exception) {
            throw IllegalStateException("Device key cannot sign with NONEwithECDSA (requires re-pairing with Hub): ${e.message}")
        }

        val alias = aliasOverride ?: requireNotNull(prefs.getString("key_alias", "homehub-device"))
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

    private val renewal = kotlinx.coroutines.sync.Mutex()
    private val prefs get() = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE)
    private fun savedScopes(): Set<String> = runCatching {
        val array = JSONArray(prefs.getString("scopes", "[]"))
        (0 until array.length()).map(array::getString).toSet()
    }.getOrDefault(emptySet())
    val scopes = kotlinx.coroutines.flow.MutableStateFlow(savedScopes())
    fun hasScope(scope: String) = scope in scopes.value
    suspend fun refreshScopes() {
        val me = get("/v1/devices/me")
        require(me.getString("id") == prefs.getString("device_id", null))
        val array = me.getJSONArray("scopes")
        require(prefs.edit().putString("scopes", array.toString()).commit())
        scopes.value = (0 until array.length()).map(array::getString).toSet()
    }

    /** Persist the CSR before sending, then persist the issued identity before activation.
     * A lost response or process death retries the same staged identity, never a fresh key.
     */
    suspend fun renewIfNeeded() = withContext(Dispatchers.IO) {
        renewal.lock()
        try {
            val cf = CertificateFactory.getInstance("X.509")
            val current = prefs.getString("client_cert_pem", null) ?: return@withContext
            val cert = cf.generateCertificate(current.byteInputStream()) as X509Certificate
            if (!prefs.contains("renew_alias") && cert.notAfter.time - System.currentTimeMillis() > 30L * 24 * 3600 * 1000) return@withContext
            if (!prefs.contains("renew_cert") && cert.notAfter.time > System.currentTimeMillis() &&
                prefs.getLong("renew_retry_at", 0) > System.currentTimeMillis()) return@withContext
            val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            var alias = prefs.getString("renew_alias", null)
            if (alias == null) {
                alias = "homehub-renew-${java.util.UUID.randomUUID()}"
                val generator = java.security.KeyPairGenerator.getInstance("EC", "AndroidKeyStore")
                generator.initialize(android.security.keystore.KeyGenParameterSpec.Builder(alias,
                    android.security.keystore.KeyProperties.PURPOSE_SIGN or android.security.keystore.KeyProperties.PURPOSE_VERIFY)
                    .setAlgorithmParameterSpec(java.security.spec.ECGenParameterSpec("secp256r1"))
                    .setDigests(android.security.keystore.KeyProperties.DIGEST_NONE, android.security.keystore.KeyProperties.DIGEST_SHA256).build())
                val pair = generator.generateKeyPair()
                val csr = DeviceIdentity.csr(pair, prefs.getString("device_id", "homehub")!!)
                if (!prefs.edit().putString("renew_alias", alias).putString("renew_csr", csr).commit()) {
                    ks.deleteEntry(alias); error("Unable to persist renewal")
                }
            }
            val stagedAlias = requireNotNull(alias)
            val trust = trustStore.load() ?: return@withContext
            val ca = cf.generateCertificate(trust.caCertPem.byteInputStream()) as X509Certificate
            fun issue(): String? {
                val body = JSONObject().put("csr_pem", prefs.getString("renew_csr", null)).toString()
                val response = client().newCall(Request.Builder().url(baseUrl() + "/v1/certs/renew")
                    .post(body.toRequestBody("application/json".toMediaType())).build()).execute().use {
                    if (it.code == 409 && cert.notAfter.time > System.currentTimeMillis()) {
                        // Server time is authoritative for the renewal window.
                        require(prefs.edit().putLong("renew_retry_at", System.currentTimeMillis() + 24L * 3600 * 1000).commit())
                        return null
                    }
                    if (!it.isSuccessful) throw ApiException(it.code)
                    JSONObject(requireNotNull(it.body).string())
                }
                val issued = response.getString("cert_pem")
                val next = cf.generateCertificate(issued.byteInputStream()) as X509Certificate
                next.verify(ca.publicKey); next.checkValidity()
                require(next.publicKey.encoded.contentEquals(ks.getCertificate(stagedAlias).publicKey.encoded))
                require(next.subjectX500Principal == cert.subjectX500Principal)
                require(prefs.edit().putString("renew_cert", issued).putLong("renew_expires", response.getLong("cert_expires_at"))
                    .remove("renew_retry_at").commit())
                return issued
            }
            val pin = PinnedCaTrustManager(ca)
            fun candidate(issued: String): OkHttpClient {
                val tls = createSslContext(ca, pin, issued, stagedAlias)
                return OkHttpClient.Builder().sslSocketFactory(tls.socketFactory, pin)
                    .hostnameVerifier { _, _ -> true }.followRedirects(false).followSslRedirects(false).build()
            }
            fun activate(nextClient: OkHttpClient) {
                nextClient.newCall(Request.Builder().url(baseUrl() + "/v1/info").get().build()).execute().use {
                    if (!it.isSuccessful) throw ApiException(it.code)
                    require(JSONObject(requireNotNull(it.body).string()).getString("hub_id") == trust.hubId)
                }
            }
            var issued = prefs.getString("renew_cert", null) ?: (issue() ?: return@withContext)
            var nextClient = candidate(issued)
            try { activate(nextClient) }
            catch (e: Exception) {
                if (e !is javax.net.ssl.SSLHandshakeException && !(e is ApiException && e.code in listOf(401, 403))) throw e
                // A stage can expire while this phone is away. Re-stage the same
                // persisted CSR through the still-current identity exactly once.
                // If activation already succeeded but its response was lost, the
                // old identity is rejected and the persisted new one is retained.
                nextClient.connectionPool.evictAll()
                issued = issue() ?: throw e
                nextClient = candidate(issued)
                activate(nextClient)
            }
            val oldAlias = prefs.getString("key_alias", null)
            require(prefs.edit().putString("key_alias", alias).putString("client_cert_pem", issued)
                .putLong("cert_expires_at", prefs.getLong("renew_expires", 0))
                .remove("renew_alias").remove("renew_csr").remove("renew_cert").remove("renew_expires").remove("renew_retry_at").commit())
            synchronized(this@HubClient) { cachedClient?.connectionPool?.evictAll(); cachedClient = nextClient }
            if (oldAlias != null && oldAlias != alias) runCatching { ks.deleteEntry(oldAlias) }
        } finally { renewal.unlock() }
    }

    fun resetClient() {
        synchronized(this) {
            cachedClient?.connectionPool?.evictAll()
            cachedClient = null
            scopes.value = savedScopes()
        }
    }

    fun baseUrl(): String {
        val trust = runBlocking(Dispatchers.IO) { trustStore.load() } ?: throw IllegalStateException("not paired")
        val url = okhttp3.HttpUrl.Builder().scheme("https").host("localhost").build().resolve("https://${trust.lastAddr}") ?: error("Invalid Home address")
        require(com.homehub.screen.LanSdp.privateAddress(url.host))
        return "https://${trust.lastAddr}"
    }

    class ApiException(val code: Int) : java.io.IOException("Home request failed ($code)")

    suspend fun info(): JSONObject = get("/v1/info")
    suspend fun request(method: String, path: String, body: Any? = null): String = withContext(Dispatchers.IO) {
        require(path.startsWith("/v1/") && !path.contains(".."))
        renewIfNeeded()
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
        require(path.startsWith("/v1/") && !path.contains("..")); renewIfNeeded()
        client().newCall(Request.Builder().url(baseUrl() + path).get().build()).execute().use {
            if (!it.isSuccessful) throw ApiException(it.code)
            it.body?.bytes() ?: byteArrayOf()
        }
    }
    /** Stream into an app-private partial file; callers export only after verification. */
    suspend fun download(fileId: String, target: File): JSONObject = withContext(Dispatchers.IO) {
        val meta = get("/v1/files/$fileId")
        require(fileId.matches(Regex("[A-Za-z0-9_-]+")))
        val expectedSize = meta.getLong("size")
        val expectedHash = meta.getString("hash")
        require(expectedSize >= 0 && expectedHash.matches(Regex("[a-f0-9]{64}")))
        val identity = File(target.path + ".identity")
        val marker = "$fileId:$expectedSize:$expectedHash"
        if (!identity.exists() || identity.readText() != marker || target.length() > expectedSize) {
            java.io.RandomAccessFile(target, "rw").use { it.setLength(0); it.fd.sync() }
            identity.outputStream().use { it.write(marker.toByteArray()); it.fd.sync() }
        }
        var offset = target.length()
        if (offset < expectedSize || expectedSize == 0L) {
            val request = Request.Builder().url(baseUrl() + "/v1/files/$fileId/content").get()
            if (offset > 0) request.header("Range", "bytes=$offset-")
            client().newCall(request.build()).execute().use { response ->
                if (!response.isSuccessful) throw ApiException(response.code)
                if (offset > 0 && response.code == 200) offset = 0
                if (response.code == 206) require(DownloadIntegrity.rangeStart(response.header("Content-Range"), expectedSize) == offset)
                else require(response.code == 200)
                java.io.RandomAccessFile(target, "rw").use { output ->
                    output.setLength(offset); output.seek(offset)
                    requireNotNull(response.body).byteStream().use { input ->
                        val buffer = ByteArray(256 * 1024)
                        while (true) {
                            kotlinx.coroutines.currentCoroutineContext().ensureActive()
                            val n = input.read(buffer)
                            if (n < 0) break
                            require(offset + n <= expectedSize)
                            output.write(buffer, 0, n); offset += n
                        }
                    }
                    output.fd.sync()
                }
            }
        }
        val hasher = Blake3.Hasher()
        target.inputStream().use { input ->
            val buffer = ByteArray(256 * 1024)
            while (true) {
                kotlinx.coroutines.currentCoroutineContext().ensureActive()
                val n = input.read(buffer); if (n < 0) break
                hasher.update(buffer, 0, n)
            }
        }
        if (target.length() != expectedSize || hasher.digestHex() != expectedHash) {
            target.delete(); identity.delete(); throw java.io.IOException("Download verification failed")
        }
        meta
    }
    suspend fun createTransfer(name: String, size: Long, mime: String?, kind: String,
        clientItemId: String, rootHash: String, takenAt: Long?, backupSourceId: String? = null, targetDeviceId: String? = null): JSONObject =
        post("/v1/transfers", JSONObject().apply {
            put("name", name); put("size", size); put("kind", kind)
            put("client_item_id", clientItemId); put("root_hash", rootHash)
            put("chunk_size", 4 * 1024 * 1024)
            mime?.let { put("mime", it) }; takenAt?.let { put("taken_at", it) }
            backupSourceId?.let { put("backup_source_id", it) }
            targetDeviceId?.let { put("target_device_id", it) }
        })
    suspend fun putChunk(transferId: String, idx: Long, bytes: ByteArray, hashHex: String): Int = withContext(Dispatchers.IO) {
        renewIfNeeded()
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
    suspend fun openRemoteInput(listener: okhttp3.WebSocketListener): okhttp3.WebSocket = withContext(Dispatchers.IO) {
        renewIfNeeded()
        client().newWebSocket(Request.Builder().url(baseUrl().replaceFirst("https://", "wss://") + "/v1/remote/input").build(), listener)
    }
}
