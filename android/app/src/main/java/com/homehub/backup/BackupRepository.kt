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
        if (!prefs.getBoolean("enabled", false)) return false
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
    val lastRun: Long = 0, val pendingCleanup: Boolean = false)
data class LocalMedia(val uri: Uri, val clientId: String, val size: Long, val takenAt: Long?, val hash: String)

@Singleton
class BackupRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val hub: HubClient,
    private val queue: QueueRepository,
) {
    private val prefs = BackupRules.prefs(context)
    val settings = MutableStateFlow(readSettings())
    val status = MutableStateFlow<String?>(null)
    private fun readSettings() = BackupSettings(prefs.contains("source_id"), prefs.getBoolean("enabled", false),
        prefs.getBoolean("wifi_only", true), prefs.getBoolean("charging_only", false), prefs.getLong("interval_hours", 6),
        prefs.getLong("last_run", 0), prefs.contains("cleanup"))
    private fun refresh() { settings.value = readSettings() }

    /** Called only after explicit approval and media permission. */
    suspend fun approve() = withContext(Dispatchers.IO) {
        val source = hub.post("/v1/backup/sources", JSONObject().put("kind", "camera_roll").put("label", android.os.Build.MODEL))
        require(prefs.edit().putString("source_id", source.getString("id")).putBoolean("enabled", true).commit())
        updateRules(true, settings.value.wifiOnly, settings.value.chargingOnly, settings.value.intervalHours)
    }
    suspend fun updateRules(enabled: Boolean, wifiOnly: Boolean, chargingOnly: Boolean, intervalHours: Long) = withContext(Dispatchers.IO) {
        require(intervalHours in listOf(1L, 6L, 12L, 24L))
        val sourceId = prefs.getString("source_id", null)
        if (sourceId != null) hub.patch("/v1/backup/sources/$sourceId", JSONObject().put("enabled", enabled)
            .put("wifi_only", wifiOnly).put("charging_only", chargingOnly))
        require(prefs.edit().putBoolean("enabled", enabled && sourceId != null).putBoolean("wifi_only", wifiOnly)
            .putBoolean("charging_only", chargingOnly).putLong("interval_hours", intervalHours).commit())
        schedule(); refresh()
    }
    fun schedule() {
        val manager = WorkManager.getInstance(context)
        if (!prefs.getBoolean("enabled", false)) { manager.cancelUniqueWork("homehub-photo-backup"); return }
        val constraints = Constraints.Builder().setRequiredNetworkType(
            if (prefs.getBoolean("wifi_only", true)) NetworkType.UNMETERED else NetworkType.CONNECTED)
            .setRequiresCharging(prefs.getBoolean("charging_only", false)).build()
        val request = PeriodicWorkRequestBuilder<BackupWorker>(prefs.getLong("interval_hours", 6), TimeUnit.HOURS)
            .setConstraints(constraints).build()
        manager.enqueueUniquePeriodicWork("homehub-photo-backup", ExistingPeriodicWorkPolicy.UPDATE, request)
    }
    fun runNow() {
        val request = OneTimeWorkRequestBuilder<BackupWorker>().setConstraints(Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED).build()).build()
        WorkManager.getInstance(context).enqueueUniqueWork("homehub-photo-backup-now", ExistingWorkPolicy.KEEP, request)
    }
    private suspend fun media(): List<LocalMedia> {
        val items = mutableListOf<LocalMedia>()
        val collection = MediaStore.Files.getContentUri("external")
        val columns = arrayOf(MediaStore.Files.FileColumns._ID, MediaStore.Files.FileColumns.MEDIA_TYPE)
        val selection = "${MediaStore.Files.FileColumns.MEDIA_TYPE} IN (?,?)"
        context.contentResolver.query(collection, columns, selection, arrayOf(
            MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE.toString(), MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO.toString()),
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
        val source = prefs.getString("source_id", null) ?: return@withContext
        if (!BackupRules.allowed(context)) return@withContext
        for (batch in media().chunked(100)) {
            if (!BackupRules.allowed(context)) return@withContext
            val payload = JSONArray()
            batch.forEach { payload.put(JSONObject().put("client_item_id", it.clientId).put("size", it.size)
                .put("taken_at", it.takenAt).put("hash", it.hash)) }
            val needed = hub.post("/v1/backup/sources/$source/diff", payload).getJSONArray("needed")
            val ids = (0 until needed.length()).map { needed.getString(it) }.toSet()
            queue.enqueuePersisted(batch.filter { it.clientId in ids }.map { it.uri }, "backup", source)
        }
        prefs.edit().putLong("last_run", System.currentTimeMillis()).apply(); refresh()
    }
    /** Pins durable, freshly rehashed Hub copies across the OS confirmation. */
    suspend fun prepareCleanup(): List<Uri> = withContext(Dispatchers.IO) {
        require(Build.VERSION.SDK_INT >= 30) { "Remove verified photos through your system gallery on this Android version" }
        require(!prefs.contains("cleanup")) { "Finish or release the previous storage review first" }
        val source = prefs.getString("source_id", null) ?: error("Approve photo backup first")
        val local = media().take(500)
        val lease = hub.post("/v1/backup/sources/$source/cleanup-lease", JSONObject()
            .put("client_item_ids", JSONArray(local.map { it.clientId })))
        val rows = lease.getJSONArray("items")
        val mappings = JSONArray()
        for (i in 0 until rows.length()) {
            val row = rows.getJSONObject(i)
            val item = local.firstOrNull { it.clientId == row.getString("client_item_id") } ?: continue
            if (item.hash != row.getString("hash") || item.size != row.getLong("size")) continue
            mappings.put(JSONObject(row.toString()).put("uri", item.uri.toString()))
        }
        val persisted = JSONObject().put("lease_id", lease.getString("lease_id")).put("items", mappings)
        require(prefs.edit().putString("cleanup", persisted.toString()).commit())
        refresh()
        try {
            // Recheck immediately before showing the system dialog. No provider
            // mutation, no direct delete(), and no deletion on worker completion.
            for (i in 0 until mappings.length()) {
                val row = mappings.getJSONObject(i)
                val pair = SourceReader.hash(context, Uri.parse(row.getString("uri")))
                require(pair.first == row.getString("hash") && pair.second == row.getLong("size")) { "A photo changed; review again" }
            }
            (0 until mappings.length()).map { Uri.parse(mappings.getJSONObject(it).getString("uri")) }
        } catch (e: Exception) { finishCleanup(false); throw e }
    }
    /** On restart retain every pin until user explicitly resolves the old review. */
    suspend fun finishCleanup(confirmed: Boolean) = withContext(Dispatchers.IO) {
        val raw = prefs.getString("cleanup", null) ?: return@withContext
        val cleanup = JSONObject(raw); val mappings = cleanup.getJSONArray("items")
        val freed = JSONArray()
        if (confirmed) for (i in 0 until mappings.length()) {
            val row = mappings.getJSONObject(i)
            val missing = runCatching { context.contentResolver.query(Uri.parse(row.getString("uri")),
                arrayOf(MediaStore.MediaColumns._ID), null, null, null)?.use { !it.moveToFirst() } == true }.getOrDefault(false)
            if (missing) freed.put(row.getString("client_item_id"))
        }
        hub.post("/v1/backup/cleanup-leases/${cleanup.getString("lease_id")}/complete", JSONObject().put("freed_client_item_ids", freed))
        require(prefs.edit().remove("cleanup").commit()); refresh()
    }
}
