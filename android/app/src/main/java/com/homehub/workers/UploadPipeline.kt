package com.homehub.workers

import android.content.Context
import android.net.Uri
import com.homehub.net.Blake3
import com.homehub.net.HubClient
import com.homehub.queue.QueueDao
import com.homehub.queue.QueueItem
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
) {
    companion object {
        const val CHUNK = 4 * 1024 * 1024
        private const val MAX_ATTEMPTS = 8
    }

    private val mutex = Mutex()

    suspend fun processDue() {
        if (!mutex.tryLock()) return
        try {
            val now = System.currentTimeMillis()
            val due = queueDao.due(now)
            if (due.isEmpty()) return

            for (item in due) {
                processItem(item)
            }
        } finally {
            mutex.unlock()
        }
    }

    private suspend fun processItem(item0: QueueItem) {
        var item = item0
        fun now() = System.currentTimeMillis()
        suspend fun set(
            state: String, transferId: String? = item.transferId,
            rootHash: String? = item.rootHash, error: String? = null,
            attempts: Int = item.attempts, nextAttemptAt: Long? = null
        ) {
            queueDao.updateProgress(item.id, state, transferId, rootHash, attempts, nextAttemptAt, error, now())
            item = queueDao.get(item.id) ?: item
        }

        try {
            set(QueueItem.CONNECTING)
            hub.info() // reachability + cert validity check

            // Lazy whole-file root hash (computed once, persisted).
            if (item.rootHash == null) {
                set(QueueItem.CONNECTING, rootHash = hashWholeFile(Uri.parse(item.sourceUri)))
            }

            // Create or resume the transfer.
            var transferId = item.transferId
            if (transferId == null) {
                val created = hub.createTransfer(
                    name = item.name, size = item.size, mime = item.mime, kind = item.kind,
                    clientItemId = item.clientItemId, rootHash = item.rootHash!!,
                    takenAt = null,
                )
                if (created.optBoolean("already_exists")) {
                    set(QueueItem.DONE)
                    return
                }
                transferId = created.getString("transfer_id")
                set(QueueItem.UPLOADING, transferId = transferId)
            }

            val status = runCatching { hub.transferStatus(transferId) }.getOrNull()
                ?: run {
                    set(QueueItem.CONNECTING, transferId = null)
                    return processItem(queueDao.get(item.id)!!)
                }
            val have = parseRanges(status.getJSONObject("have"))
            val chunkCount = status.getLong("chunk_count")

            set(QueueItem.UPLOADING)
            uploadChunks(Uri.parse(item.sourceUri), transferId, have, chunkCount)

            set(QueueItem.VERIFYING)
            hub.complete(transferId, item.rootHash!!)
            set(QueueItem.DONE)
        } catch (e: PermException) {
            android.util.Log.e("UploadPipeline", "Permanent failure for ${item.name}: ${e.message}", e)
            set(QueueItem.FAILED_PERM, error = e.message)
        } catch (e: Exception) {
            android.util.Log.e("UploadPipeline", "Retryable error for ${item.name}: ${e.message}", e)
            val attempts = item.attempts + 1
            if (attempts >= MAX_ATTEMPTS) {
                set(QueueItem.FAILED_PERM, attempts = attempts, error = "gave up: ${e.message}")
            } else {
                val backoffMs = min(2.0.pow(attempts).toLong() * 60_000L, 6L * 60 * 60_000L)
                set(QueueItem.FAILED_RETRY, attempts = attempts,
                    nextAttemptAt = now() + backoffMs, error = e.message)
            }
        }
    }

    private fun hashWholeFile(uri: Uri): String {
        val h = Blake3.Hasher()
        context.contentResolver.openInputStream(uri).use { input ->
            requireNotNull(input) { "cannot open $uri" }
            val buf = ByteArray(256 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n <= 0) break
                h.update(buf, 0, n)
            }
        }
        return h.digestHex()
    }

    private suspend fun uploadChunks(uri: Uri, transferId: String, have: List<LongRange>, chunkCount: Long) {
        context.contentResolver.openInputStream(uri).use { input ->
            requireNotNull(input) { "cannot open $uri" }
            val buf = ByteArray(CHUNK)
            var idx = 0L
            while (idx < chunkCount) {
                var filled = 0
                while (filled < buf.size) {
                    val n = input.read(buf, filled, buf.size - filled)
                    if (n <= 0) break
                    filled += n
                }
                if (filled == 0 && idx < chunkCount) throw PermException("source shrank during upload")
                if (!isCovered(idx, have)) {
                    val chunk = if (filled == buf.size) buf else buf.copyOf(filled)
                    val code = hub.putChunk(transferId, idx, chunk, Blake3.hashHex(chunk))
                    when (code) {
                        200, 201, 204 -> Unit
                        401, 403 -> throw PermException("device not authorized ($code)")
                        404 -> throw PermException("transfer gone ($code)")
                        else -> error("chunk $idx -> $code")
                    }
                }
                idx++
            }
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
