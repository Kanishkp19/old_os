package com.homehub.backup

import android.content.ContentUris
import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.BatteryManager
import android.os.Build
import android.provider.MediaStore
import androidx.work.*
import com.homehub.net.HubClient
import com.homehub.queue.QueueRepository
import com.homehub.queue.SourceReader
import com.homehub.workers.UploadPipeline
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

object BackupRules {
    fun prefs(context: Context) = context.getSharedPreferences("homehub_backup", Context.MODE_PRIVATE)
    fun allowed(context: Context): Boolean {
        val prefs = prefs(context)
        if (!prefs.getBoolean("enabled", false) || prefs.getString("source_hub_id", null) !=
            context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE).getString("hub_id", null)) return false
        if (prefs.getBoolean("wifi_only", true)) {
            val manager = context.getSystemService(ConnectivityManager::class.java)
            val caps = manager.getNetworkCapabilities(manager.activeNetwork) ?: return false
            if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) return false
        }
        if (prefs.getBoolean("charging_only", false) && !context.getSystemService(BatteryManager::class.java).isCharging) return false
        return true
    }
}
data class BackupSettings(val approved: Boolean = false, val enabled: Boolean = false,
    val wifiOnly: Boolean = true, val chargingOnly: Boolean = false, val intervalHours: Long = 6,
    val lastRun: Long = 0, val pendingCleanup: Boolean = false, val effectiveMinutes: Long = 360)
object MediaAccess {
    fun permissions(): Array<String> = if (Build.VERSION.SDK_INT >= 34) arrayOf(
        android.Manifest.permission.READ_MEDIA_IMAGES, android.Manifest.permission.READ_MEDIA_VIDEO,
        android.Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)
        else if (Build.VERSION.SDK_INT >= 33) arrayOf(android.Manifest.permission.READ_MEDIA_IMAGES, android.Manifest.permission.READ_MEDIA_VIDEO)
        else arrayOf(android.Manifest.permission.READ_EXTERNAL_STORAGE)
    fun granted(context: Context): Boolean = permissions().any {
        androidx.core.content.ContextCompat.checkSelfPermission(context, it) == android.content.pm.PackageManager.PERMISSION_GRANTED
    }
    fun partial(context: Context) = Build.VERSION.SDK_INT >= 34 &&
        androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED) == android.content.pm.PackageManager.PERMISSION_GRANTED &&
        androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.READ_MEDIA_IMAGES) != android.content.pm.PackageManager.PERMISSION_GRANTED
}
data class LocalMedia(val uri: Uri, val clientId: String, val size: Long, val takenAt: Long?, val hash: String)

@Singleton
class BackupRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val hub: HubClient,
    private val queue: QueueRepository,
    private val trust: com.homehub.queue.HubTrustStore,
) {
    private val prefs = BackupRules.prefs(context)
    val settings = MutableStateFlow(readSettings())
    val status = MutableStateFlow<String?>(null)
    val remoteReviews = MutableStateFlow<List<JSONObject>>(emptyList())
    private fun ownSource() = prefs.contains("source_id") && prefs.getString("source_hub_id", null) ==
        context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE).getString("hub_id", null)
    private fun readSettings() = BackupSettings(ownSource(), ownSource() && prefs.getBoolean("enabled", false),
        prefs.getBoolean("wifi_only", true), prefs.getBoolean("charging_only", false), prefs.getLong("interval_hours", 6),
        prefs.getLong("last_run", 0), prefs.contains("cleanup") || prefs.contains("cleanup_review_id"), effectiveMinutes())
    fun refresh() { settings.value = readSettings() }

    /** Called only after explicit approval and media permission. */
    suspend fun approve() = withContext(Dispatchers.IO) {
        if (!MediaAccess.granted(context)) throw SecurityException(context.getString(com.homehub.R.string.backup_permission_required))
        val hubId = requireNotNull(trust.load()).hubId
        val source = hub.post("/v1/backup/sources", JSONObject().put("kind", "camera_roll").put("label", android.os.Build.MODEL))
        require(prefs.edit().putString("source_id", source.getString("id")).putString("source_hub_id", hubId)
            .putBoolean("enabled", true).remove("scan_since_sec").remove("last_full_scan").commit())
        updateRules(true, settings.value.wifiOnly, settings.value.chargingOnly, settings.value.intervalHours)
    }
    suspend fun updateRules(enabled: Boolean, wifiOnly: Boolean, chargingOnly: Boolean, intervalHours: Long) = withContext(Dispatchers.IO) {
        require(intervalHours in listOf(1L, 6L, 12L, 24L))
        val sourceId = if (ownSource()) prefs.getString("source_id", null) else null
        if (sourceId != null) hub.patch("/v1/backup/sources/$sourceId", JSONObject().put("enabled", enabled)
            .put("wifi_only", wifiOnly).put("charging_only", chargingOnly))
        require(prefs.edit().putBoolean("enabled", enabled && sourceId != null).putBoolean("wifi_only", wifiOnly)
            .putBoolean("charging_only", chargingOnly).putLong("interval_hours", intervalHours).commit())
        schedule(); refresh()
    }
    private fun effectiveMinutes() = maxOf(prefs.getLong("interval_hours", 6) * 60,
        prefs.getLong("home_interval_minutes", 15)).coerceAtLeast(15)
    suspend fun applyGlobalSchedule(minutes: Long) = withContext(Dispatchers.IO) {
        if (minutes !in 15..10080 || prefs.getLong("home_interval_minutes", 15) == minutes) return@withContext
        require(prefs.edit().putLong("home_interval_minutes", minutes).commit())
        schedule(); refresh()
    }
    fun schedule() {
        val manager = WorkManager.getInstance(context)
        if (!prefs.getBoolean("enabled", false)) { manager.cancelUniqueWork("homehub-photo-backup"); return }
        val constraints = Constraints.Builder().setRequiredNetworkType(
            if (prefs.getBoolean("wifi_only", true)) NetworkType.UNMETERED else NetworkType.CONNECTED)
            .setRequiresCharging(prefs.getBoolean("charging_only", false)).build()
        val request = PeriodicWorkRequestBuilder<BackupWorker>(effectiveMinutes(), TimeUnit.MINUTES)
            .setConstraints(constraints).build()
        manager.enqueueUniquePeriodicWork("homehub-photo-backup", ExistingPeriodicWorkPolicy.UPDATE, request)
    }
    fun runNow() {
        val request = OneTimeWorkRequestBuilder<BackupWorker>().setConstraints(Constraints.Builder()
            .setRequiredNetworkType(if (prefs.getBoolean("wifi_only", true)) NetworkType.UNMETERED else NetworkType.CONNECTED)
            .setRequiresCharging(prefs.getBoolean("charging_only", false)).build()).build()
        WorkManager.getInstance(context).enqueueUniqueWork("homehub-photo-backup-now", ExistingWorkPolicy.KEEP, request)
    }
    private suspend fun media(sinceSeconds: Long = 0): List<LocalMedia> {
        val items = mutableListOf<LocalMedia>()
        val collection = MediaStore.Files.getContentUri("external")
        val columns = arrayOf(MediaStore.Files.FileColumns._ID, MediaStore.Files.FileColumns.MEDIA_TYPE)
        val selection = "${MediaStore.Files.FileColumns.MEDIA_TYPE} IN (?,?)" +
            if (sinceSeconds > 0) " AND (${MediaStore.MediaColumns.DATE_MODIFIED}>=? OR ${MediaStore.MediaColumns.DATE_ADDED}>=?)" else ""
        val args = mutableListOf(MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE.toString(),
            MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO.toString())
        if (sinceSeconds > 0) {
            val overlap = (sinceSeconds - 1).toString()
            args += overlap; args += overlap
        }
        context.contentResolver.query(collection, columns, selection, args.toTypedArray(),
            "${MediaStore.Files.FileColumns._ID} ASC")?.use { cursor ->
            while (cursor.moveToNext()) {
                val id = cursor.getLong(0)
                val base = if (cursor.getInt(1) == MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE)
                    MediaStore.Images.Media.EXTERNAL_CONTENT_URI else MediaStore.Video.Media.EXTERNAL_CONTENT_URI
                val uri = ContentUris.withAppendedId(base, id)
                val meta = SourceReader.meta(context, uri)
                val (hash, size) = SourceReader.hash(context, uri)
                require(meta.size < 0 || meta.size == size) { "A photo changed during review; try again" }
                items += LocalMedia(uri, SourceReader.identity(uri, meta.copy(size = size)), size, meta.takenAt, hash)
            }
        } ?: error("Allow photo and video access to back up your library")
        return items
    }
    suspend fun scanAndQueue() = withContext(Dispatchers.IO) {
        if (!ownSource()) return@withContext
        if (!MediaAccess.granted(context)) throw SecurityException(context.getString(com.homehub.R.string.backup_permission_required))
        val source = prefs.getString("source_id", null) ?: return@withContext
        if (!BackupRules.allowed(context)) return@withContext
        val scanStarted = System.currentTimeMillis()
        // Full sweeps catch older copies lost on the Hub and newly granted
        // photos under Android's partial-library permission.
        val full = MediaAccess.partial(context) || scanStarted - prefs.getLong("last_full_scan", 0) >= 7L * 24 * 3600 * 1000
        val since = if (full) 0 else prefs.getLong("scan_since_sec", 0)
        for (batch in media(since).chunked(100)) {
            if (!BackupRules.allowed(context)) return@withContext
            val payload = JSONArray()
            batch.forEach { payload.put(JSONObject().put("client_item_id", it.clientId).put("size", it.size)
                .put("taken_at", it.takenAt).put("hash", it.hash)) }
            val needed = hub.post("/v1/backup/sources/$source/diff", payload).getJSONArray("needed")
            val ids = (0 until needed.length()).map { needed.getString(it) }.toSet()
            queue.enqueuePersisted(batch.filter { it.clientId in ids }.map { it.uri }, "backup", source)
        }
        val saved = prefs.edit().putLong("last_run", scanStarted).putLong("scan_since_sec", scanStarted / 1000)
        if (full) saved.putLong("last_full_scan", scanStarted)
        require(saved.commit()) { "Unable to save photo scan progress" }
        refresh()
    }
    /** Pins durable, freshly rehashed Hub copies across the OS confirmation. */
    suspend fun prepareCleanup(): List<Uri> = withContext(Dispatchers.IO) {
        require(Build.VERSION.SDK_INT >= 30) { "Remove verified photos through your system gallery on this Android version" }
        require(!prefs.contains("cleanup")) { "Finish or release the previous storage review first" }
        require(ownSource())
        val source = prefs.getString("source_id", null) ?: error("Approve photo backup first")
        val savedRequest = prefs.getString("cleanup_request", null)
        val local = if (savedRequest != null) JSONArray(savedRequest).let { items ->
            (0 until items.length()).map { items.getJSONObject(it) }.map {
                LocalMedia(Uri.parse(it.getString("uri")), it.getString("client_id"), it.getLong("size"), null, it.getString("hash"))
            }
        } else media().take(500)
        if (local.isEmpty()) return@withContext emptyList<Uri>()
        val review = prefs.getString("cleanup_review_id", null) ?: java.util.UUID.randomUUID().toString()
        val snapshot = JSONArray(local.map { JSONObject().put("uri", it.uri.toString()).put("client_id", it.clientId)
            .put("size", it.size).put("hash", it.hash) })
        require(prefs.edit().putString("cleanup_review_id", review).putString("cleanup_request", snapshot.toString()).commit())
        refresh()
        val lease = hub.post("/v1/backup/sources/$source/cleanup-lease", JSONObject()
            .put("client_item_ids", JSONArray(local.map { it.clientId })).put("client_review_id", review))
        val rows = lease.getJSONArray("items")
        val mappings = JSONArray()
        for (i in 0 until rows.length()) {
            val row = rows.getJSONObject(i)
            val item = local.firstOrNull { it.clientId == row.getString("client_item_id") } ?: continue
            if (item.hash != row.getString("hash") || item.size != row.getLong("size")) continue
            mappings.put(JSONObject(row.toString()).put("uri", item.uri.toString()))
        }
        val persisted = JSONObject().put("lease_id", lease.getString("lease_id")).put("hub_id", requireNotNull(trust.load()).hubId).put("items", mappings)
        require(prefs.edit().putString("cleanup", persisted.toString()).remove("cleanup_review_id").remove("cleanup_request").commit())
        refresh()
        try {
            // Recheck immediately before showing the system dialog. No provider
            // mutation, no direct delete(), and no deletion on worker completion.
            validateCleanup()
        } catch (e: Exception) { finishCleanup(false); throw e }
    }
    /** Re-hash after the user review, immediately before launching Android consent. */
    suspend fun validateCleanup(): List<Uri> = withContext(Dispatchers.IO) {
        val cleanup = JSONObject(requireNotNull(prefs.getString("cleanup", null)))
        require(cleanup.getString("hub_id") == requireNotNull(trust.load()).hubId)
        val mappings = cleanup.getJSONArray("items")
        for (i in 0 until mappings.length()) {
            val row = mappings.getJSONObject(i)
            val pair = SourceReader.hash(context, Uri.parse(row.getString("uri")))
            require(pair.first == row.getString("hash") && pair.second == row.getLong("size"))
        }
        (0 until mappings.length()).map { Uri.parse(mappings.getJSONObject(it).getString("uri")) }
    }
    suspend fun recoverReview() {
        if (!prefs.contains("cleanup") && prefs.contains("cleanup_review_id")) {
            try { prepareCleanup() }
            catch (e: kotlinx.coroutines.CancellationException) { throw e }
            catch (e: Exception) { if (!prefs.contains("cleanup")) throw e }
        }
        finishCleanup(true)
    }
    suspend fun refreshRemoteReviews() {
        if (!ownSource()) return
        val source = requireNotNull(prefs.getString("source_id", null))
        val items = hub.get("/v1/backup/sources/$source/cleanup").getJSONArray("items")
        remoteReviews.value = (0 until items.length()).map(items::getJSONObject)
    }
    /** Only an explicit recovery action releases server reviews without a local dialog receipt. */
    suspend fun releaseOrphanReviews() {
        refreshRemoteReviews()
        val local = prefs.getString("cleanup", null)?.let { JSONObject(it).getString("lease_id") }
        for (review in remoteReviews.value) {
            if (review.getString("lease_id") == local) continue
            hub.post("/v1/backup/cleanup-leases/${review.getString("lease_id")}/complete", JSONObject().put("freed_client_item_ids", JSONArray()))
        }
        refreshRemoteReviews()
    }
    /** On restart retain every pin until user explicitly resolves the old review. */
    suspend fun finishCleanup(confirmed: Boolean) = withContext(Dispatchers.IO) {
        val raw = prefs.getString("cleanup", null) ?: return@withContext
        val cleanup = JSONObject(raw)
        require(cleanup.getString("hub_id") == requireNotNull(trust.load()).hubId)
        val mappings = cleanup.getJSONArray("items")
        val freed = JSONArray()
        if (confirmed && !MediaAccess.partial(context) && MediaAccess.granted(context)) for (i in 0 until mappings.length()) {
            val row = mappings.getJSONObject(i)
            val missing = runCatching { context.contentResolver.query(Uri.parse(row.getString("uri")),
                arrayOf(MediaStore.MediaColumns._ID), null, null, null)?.use { !it.moveToFirst() } == true }.getOrDefault(false)
            if (missing) freed.put(row.getString("client_item_id"))
        }
        hub.post("/v1/backup/cleanup-leases/${cleanup.getString("lease_id")}/complete", JSONObject().put("freed_client_item_ids", freed))
        require(prefs.edit().remove("cleanup").remove("cleanup_review_id").remove("cleanup_request").commit()); refresh()
    }
}
