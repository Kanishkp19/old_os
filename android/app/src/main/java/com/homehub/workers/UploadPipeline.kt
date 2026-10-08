package com.homehub.workers

import android.content.Context
import android.net.Uri
import com.homehub.net.Blake3
import com.homehub.net.HubClient
import com.homehub.queue.QueueDao
import com.homehub.queue.QueueItem
import com.homehub.queue.SourceReader
import com.homehub.backup.BackupRules
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.sync.Mutex
import org.json.JSONObject
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.min
import kotlin.math.pow

/**
 * Upload pipeline (API_SPEC §12):
 * connecting → (lazy root hash) → create/resume transfer →
 * uploading (chunked PUT) → verifying → done
 */
@Singleton
class UploadPipeline @Inject constructor(
    @ApplicationContext private val context: Context,
    private val queueDao: QueueDao,
    private val hub: HubClient,
    private val trust: com.homehub.queue.HubTrustStore,
) {
    companion object {
        const val CHUNK = 4 * 1024 * 1024
        private const val MAX_ATTEMPTS = 8
    }

    private val mutex = Mutex()

    suspend fun processDue(onProgress: suspend (QueueItem) -> Unit = {}): Boolean {
        if (!mutex.tryLock()) return true
        try {
            queueDao.recoverInterrupted()
            val now = System.currentTimeMillis()
            val hubId = trust.load()?.hubId ?: return false
            val due = queueDao.due(now).filter { it.hubId == hubId }
            if (due.isEmpty()) return false

            for (item in due) {
                currentCoroutineContext().ensureActive()
                if (item.kind != "backup" || BackupRules.allowed(context)) {
                    try { processItem(item, onProgress) }
                    catch (e: CancellationException) { currentCoroutineContext().ensureActive() }
                }
            }
            return queueDao.due(Long.MAX_VALUE).any { it.hubId == hubId && (it.kind != "backup" || BackupRules.allowed(context)) }
        } finally {
            mutex.unlock()
        }
    }

    suspend fun hasOutstanding(): Boolean = trust.load()?.let { queueDao.outstandingCount(it.hubId) > 0 } ?: false

    private suspend fun processItem(item0: QueueItem, onProgress: suspend (QueueItem) -> Unit) {
        var item = item0
        fun now() = System.currentTimeMillis()
        suspend fun set(
            state: String, transferId: String? = item.transferId,
            rootHash: String? = item.rootHash, error: String? = null,
            attempts: Int = item.attempts, nextAttemptAt: Long? = null
        ) {
            queueDao.updateProgress(item.id, state, transferId, rootHash, attempts, nextAttemptAt, error, now())
            item = queueDao.get(item.id) ?: throw CancellationException("Transfer removed")
            if (item.state == QueueItem.CANCELLED) throw CancellationException("Transfer cancelled")
            onProgress(item)
        }

        try {
            set(QueueItem.CONNECTING)
            require(hub.info().getString("hub_id") == item.hubId) { "This file belongs to another Home" }

            val uri = Uri.parse(item.sourceUri)
            val meta = SourceReader.meta(context, uri)
            val (hash, actualSize) = SourceReader.hash(context, uri)
            if ((item.rootHash != null && item.rootHash != hash) ||
                (item.size >= 0 && item.size != actualSize) ||
                (item.sourceMtime != null && meta.modified != item.sourceMtime)) {
                item.transferId?.let { runCatching { hub.delete("/v1/transfers/$it") } }
                // Changing generations cannot resume chunks from the previous source.
                val refreshed = meta.copy(size = actualSize)
                item = item.copy(size = actualSize, sourceMtime = meta.modified,
                    clientItemId = SourceReader.identity(uri, refreshed), rootHash = hash,
                    transferId = null, state = QueueItem.CONNECTING, bytesSent = 0)
                queueDao.refreshSource(item.id, item.size, item.sourceMtime, item.clientItemId, item.rootHash, item.transferId, item.bytesSent)
                item = queueDao.get(item.id)?.takeIf { it.state != QueueItem.CANCELLED } ?: throw CancellationException("Transfer cancelled")
                if (item.kind == "backup") {
                    val source = requireNotNull(item.backupSourceId)
                    hub.post("/v1/backup/sources/$source/diff", org.json.JSONArray().put(JSONObject()
                        .put("client_item_id", item.clientItemId).put("size", actualSize)
                        .put("taken_at", item.takenAt).put("hash", hash)))
                }
            } else if (item.rootHash == null || item.size < 0) {
                item = item.copy(rootHash = hash, size = actualSize)
                queueDao.refreshSource(item.id, item.size, item.sourceMtime, item.clientItemId, item.rootHash, item.transferId, item.bytesSent)
                item = queueDao.get(item.id)?.takeIf { it.state != QueueItem.CANCELLED } ?: throw CancellationException("Transfer cancelled")
            }

            // Create or resume the transfer.
            var transferId = item.transferId
            if (transferId == null) {
                val created = hub.createTransfer(
                    name = item.name, size = item.size, mime = item.mime, kind = item.kind,
                    clientItemId = item.clientItemId, rootHash = item.rootHash!!,
                    takenAt = item.takenAt, backupSourceId = item.backupSourceId,
                )
                if (created.optBoolean("already_exists")) {
                    val fileId = created.getString("existing_file_id")
                    val stored = hub.get("/v1/files/$fileId")
                    require(stored.getString("hash") == item.rootHash && stored.getLong("size") == item.size)
                    queueDao.setResult(item.id, fileId)
                    set(QueueItem.DONE)
                    return
                }
                transferId = created.getString("transfer_id")
                set(QueueItem.UPLOADING, transferId = transferId)
            }

            val status = try { hub.transferStatus(transferId) }
            catch (e: HubClient.ApiException) {
                if (e.code == 404 || e.code == 410) {
                    set(QueueItem.QUEUED, transferId = null)
                    return
                }
                throw e
            }
            val have = parseRanges(status.getJSONObject("have"))
            val chunkCount = status.getLong("chunk_count")

            set(QueueItem.UPLOADING)
            uploadChunks(item, transferId, have, chunkCount, onProgress)

            set(QueueItem.VERIFYING)
            val current = SourceReader.hash(context, uri)
            if (current.first != item.rootHash || current.second != item.size)
                throw PermException(context.getString(com.homehub.R.string.error_source_changed))
            val completed = hub.complete(transferId, item.rootHash!!)
            require(completed.optBoolean("verified") && completed.getString("hash") == item.rootHash
                && completed.getLong("size") == item.size) { "Home has not verified this copy" }
            queueDao.setResult(item.id, completed.getString("file_id"))
            set(QueueItem.DONE)
        } catch (e: CancellationException) {
            if (queueDao.get(item.id)?.state != QueueItem.CANCELLED) {
                kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) { set(QueueItem.QUEUED) }
            }
            throw e
        } catch (e: java.io.FileNotFoundException) {
            set(QueueItem.FAILED_PERM, error = context.getString(com.homehub.R.string.error_missing))
        } catch (e: SecurityException) {
            set(QueueItem.FAILED_PERM, error = context.getString(com.homehub.R.string.error_access))
        } catch (e: PermException) {
            set(QueueItem.FAILED_PERM, error = e.message)
        } catch (e: Exception) {
            val attempts = item.attempts + 1
            if (attempts >= MAX_ATTEMPTS) {
                set(QueueItem.FAILED_PERM, attempts = attempts, error = com.homehub.ui.UserErrors.message(context, e))
            } else {
                val backoffMs = min(2.0.pow(attempts).toLong() * 60_000L, 6L * 60 * 60_000L)
                set(QueueItem.FAILED_RETRY, attempts = attempts,
                    nextAttemptAt = now() + backoffMs, error = com.homehub.ui.UserErrors.message(context, e))
            }
        }
    }

    private suspend fun uploadChunks(item: QueueItem, transferId: String, have: List<LongRange>, chunkCount: Long,
        onProgress: suspend (QueueItem) -> Unit) {
        require(chunkCount == (item.size + CHUNK - 1) / CHUNK) { "Unexpected transfer size" }
        context.contentResolver.openInputStream(Uri.parse(item.sourceUri)).use { input ->
            requireNotNull(input) { "This file is no longer available" }
            val buffer = ByteArray(CHUNK)
            for (idx in 0 until chunkCount) {
                currentCoroutineContext().ensureActive()
                if (queueDao.get(item.id)?.state == QueueItem.CANCELLED || queueDao.get(item.id) == null) throw CancellationException("Transfer removed")
                if (item.kind == "backup" && !BackupRules.allowed(context)) throw java.io.IOException("Waiting for your backup rules")
                val expected = min(CHUNK.toLong(), item.size - idx * CHUNK).toInt()
                var filled = 0
                while (filled < expected) {
                    val n = input.read(buffer, filled, expected - filled)
                    if (n < 0) throw PermException(context.getString(com.homehub.R.string.error_source_changed))
                    filled += n
                }
                if (!isCovered(idx, have)) {
                    val chunk = buffer.copyOf(expected)
                    when (val code = hub.putChunk(transferId, idx, chunk, Blake3.hashHex(chunk))) {
                        200, 201, 204 -> Unit
                        401, 403 -> throw PermException(context.getString(com.homehub.R.string.error_permission))
                        else -> throw HubClient.ApiException(code)
                    }
                }
                val bytes = min(item.size, (idx + 1) * CHUNK)
                queueDao.setBytes(item.id, bytes, System.currentTimeMillis())
                onProgress(item.copy(bytesSent = bytes, state = QueueItem.UPLOADING))
            }
            if (input.read() != -1) throw PermException(context.getString(com.homehub.R.string.error_source_changed))
        }
    }

    private fun isCovered(idx: Long, ranges: List<LongRange>): Boolean =
        ranges.any { idx in it }

    private fun parseRanges(have: JSONObject): List<LongRange> {
        val arr = have.getJSONArray("ranges")
        return (0 until arr.length()).map { i ->
            val r = arr.getJSONArray(i)
            r.getLong(0)..r.getLong(1)
        }
    }

    private class PermException(msg: String) : Exception(msg)
}
