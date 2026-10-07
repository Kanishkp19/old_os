package com.homehub.queue

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import com.homehub.workers.UploadWorker
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.security.MessageDigest
import java.security.SecureRandom
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The send-later queue (FR-3.4). Sharing into Home Hub only writes a row and
 * schedules a worker — it never blocks the share sheet on network I/O.
 */
@Singleton
class QueueRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: QueueDb,
    private val trustStore: HubTrustStore,
    private val uploadPipeline: com.homehub.workers.UploadPipeline,
) {
    fun observeQueue() = db.queueDao().observeAll()
    fun observePendingCount() = db.queueDao().observePendingCount()

    /** Entry point for the share sheet (ACTION_SEND / ACTION_SEND_MULTIPLE). */
    fun enqueueFromShare(intent: Intent) {
        val uris: List<Uri> = when (intent.action) {
            Intent.ACTION_SEND -> listOfNotNull(
                @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM)
            )
            Intent.ACTION_SEND_MULTIPLE ->
                @Suppress("DEPRECATION") intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                    ?: emptyList()
            else -> emptyList()
        }
        if (uris.isEmpty()) return
        enqueueUris(uris, kind = "send")
    }

    /** Enqueue explicit URIs (share sheet, file picker, or photo backup scan). */
    @OptIn(DelicateCoroutinesApi::class)
    fun enqueueUris(uris: List<Uri>, kind: String) {
        // Fire-and-forget on the IO dispatcher; the share sheet must not wait.
        GlobalScope.launch(Dispatchers.IO) {
            val trust = trustStore.load() ?: return@launch // not paired: nothing to send to
            val now = System.currentTimeMillis()
            for (uri in uris) {
                // Keep read permission across reboots where the provider allows it.
                runCatching {
                    context.contentResolver.takePersistableUriPermission(
                        uri, Intent.FLAG_GRANT_READ_URI_PERMISSION
                    )
                }
                val meta = resolveMeta(uri)
                val clientItemId = clientItemIdFor(uri, meta)
                db.queueDao().upsert(
                    QueueItem(
                        id = newUlid(now),
                        hubId = trust.hubId,
                        sourceUri = uri.toString(),
                        clientItemId = clientItemId,
                        name = meta.name,
                        size = meta.size,
                        mime = meta.mime,
                        kind = kind,
                        rootHash = null,              // hashed lazily by the worker
                        sourceMtime = meta.mtime,
                        transferId = null,
                        state = QueueItem.QUEUED,
                        attempts = 0,
                        nextAttemptAt = null,
                        lastError = null,
                        createdAt = now,
                        updatedAt = now,
                    )
                )
            }
            scheduleUpload()
        }
    }

    /** Kick the upload pipeline. Idempotent — one named work chain. */
    fun scheduleUpload() {
        runCatching {
            val req = OneTimeWorkRequestBuilder<UploadWorker>()
                .setExpedited(androidx.work.OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
                .build()
            WorkManager.getInstance(context)
                .enqueueUniqueWork("homehub-upload", ExistingWorkPolicy.REPLACE, req)
        }
        @OptIn(DelicateCoroutinesApi::class)
        GlobalScope.launch(Dispatchers.IO) {
            uploadPipeline.processDue()
        }
    }

    fun enqueue(uris: List<Uri>) {
        enqueueUris(uris, kind = "send")
    }

    suspend fun retry(itemId: String) = withContext(Dispatchers.IO) {
        db.queueDao().get(itemId)?.let {
            db.queueDao().updateProgress(
                id = it.id, state = QueueItem.QUEUED, transferId = it.transferId,
                rootHash = it.rootHash, attempts = 0, nextAttemptAt = null,
                lastError = null, now = System.currentTimeMillis(),
            )
            scheduleUpload()
            uploadPipeline.processDue()
        }
    }

    suspend fun retryAll() = withContext(Dispatchers.IO) {
        val now = System.currentTimeMillis()
        db.queueDao().resetAllPending(now)
        scheduleUpload()
        uploadPipeline.processDue()
    }

    suspend fun remove(itemId: String) = db.queueDao().delete(itemId)

    private data class Meta(val name: String, val size: Long, val mime: String?, val mtime: Long?)

    private fun resolveMeta(uri: Uri): Meta {
        var name = "file"
        var size = -1L
        runCatching {
            context.contentResolver.query(uri, null, null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }
                        ?.let { name = c.getString(it) ?: name }
                    c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 }
                        ?.let { size = c.getLong(it) }
                }
            }
        }
        val mime = context.contentResolver.getType(uri)
        val mtime = runCatching {
            context.contentResolver.openFileDescriptor(uri, "r")?.use { it.statSize; null }
        }.getOrNull()
        return Meta(name = name, size = if (size >= 0) size else 0L, mime = mime, mtime = mtime)
    }

    /**
     * Stable dedupe key for resume + Hub-side dedupe (API_SPEC §12):
     * sha256(uri | size | mtime?) — regenerated identically for the same source.
     */
    private fun clientItemIdFor(uri: Uri, meta: Meta): String {
        val md = MessageDigest.getInstance("SHA-256")
        md.update(uri.toString().toByteArray())
        md.update(meta.size.toString().toByteArray())
        return md.digest().joinToString("") { "%02x".format(it) }.take(32)
    }

    private fun newUlid(now: Long): String {
        // Crockford-base32 ULID: 48-bit time + 80-bit random.
        val alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
        val rand = ByteArray(10).also { SecureRandom().nextBytes(it) }
        val sb = StringBuilder(26)
        var t = now
        for (i in 0 until 10) { sb.append(alphabet[(t shr (5 * (9 - i)) and 0x1F).toInt()]) }
        var bits = 0; var value = 0
        for (b in rand) {
            value = (value shl 8) or (b.toInt() and 0xFF); bits += 8
            while (bits >= 5) { bits -= 5; sb.append(alphabet[value shr bits and 0x1F]) }
        }
        return sb.toString()
    }
}
