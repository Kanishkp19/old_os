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

    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.IO)
    val enqueueError = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)

    /** Application-owned scope persists the queue before WorkManager starts. */
    fun enqueueUris(uris: List<Uri>, kind: String) {
        scope.launch {
            try { enqueuePersisted(uris, kind) }
            catch (e: Exception) { enqueueError.value = "Unable to queue the selected file" }
        }
    }
    suspend fun enqueuePersisted(uris: List<Uri>, kind: String, backupSourceId: String? = null) = withContext(Dispatchers.IO) {
        val trust = trustStore.load() ?: error("Connect to your Home first")
        for (uri0 in uris) {
            var uri = uri0
            // ACTION_SEND grants are temporary. Preserve a private snapshot when
            // the provider cannot grant durable access; backup uses MediaStore.
            if (kind == "send" && uri.scheme == "content") {
                val durable = runCatching { context.contentResolver.takePersistableUriPermission(uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION) }.isSuccess
                if (!durable) {
                    val sourceMeta = SourceReader.meta(context, uri)
                    val folder = java.io.File(context.filesDir, "queue-sources").apply { mkdirs() }
                    val snapshot = java.io.File(folder, java.util.UUID.randomUUID().toString())
                    try {
                        context.contentResolver.openInputStream(uri)?.use { input -> snapshot.outputStream().use { output ->
                            input.copyTo(output); output.fd.sync()
                        } } ?: error("This file is no longer available")
                        uri = Uri.fromFile(snapshot)
                        enqueueOne(trust.hubId, uri, sourceMeta.copy(size = snapshot.length(), modified = snapshot.lastModified()), kind, backupSourceId)
                        continue
                    } catch (e: Exception) { snapshot.delete(); throw e }
                }
            }
            enqueueOne(trust.hubId, uri, SourceReader.meta(context, uri), kind, backupSourceId)
        }
        scheduleUpload()
    }
    private suspend fun enqueueOne(hubId: String, uri: Uri, meta: SourceMeta, kind: String, backupSourceId: String?) {
        val now = System.currentTimeMillis()
        val clientId = SourceReader.identity(uri, meta)
        val existing = db.queueDao().findSource(hubId, clientId, kind)
        if (existing != null) {
            if (kind == "backup" && existing.state == QueueItem.DONE) db.queueDao().upsert(existing.copy(
                state = QueueItem.QUEUED, transferId = null, resultFileId = null, bytesSent = 0, updatedAt = now))
            return
        }
        db.queueDao().upsert(QueueItem(newUlid(now), hubId, uri.toString(), clientId,
            meta.name, meta.size, meta.mime, kind, null, meta.modified, null,
            QueueItem.QUEUED, createdAt = now, updatedAt = now,
            backupSourceId = backupSourceId, takenAt = meta.takenAt))
    }
    /** One named append chain avoids cancelling an active upload. */
    fun scheduleUpload() {
        val req = OneTimeWorkRequestBuilder<UploadWorker>()
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .setBackoffCriteria(androidx.work.BackoffPolicy.EXPONENTIAL, 1, java.util.concurrent.TimeUnit.MINUTES)
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork("homehub-upload", ExistingWorkPolicy.APPEND_OR_REPLACE, req)
    }
    fun enqueue(uris: List<Uri>) = enqueueUris(uris, "send")

    suspend fun retry(itemId: String) = withContext(Dispatchers.IO) {
        db.queueDao().get(itemId)?.let {
            db.queueDao().updateProgress(
                id = it.id, state = QueueItem.QUEUED, transferId = it.transferId,
                rootHash = it.rootHash, attempts = 0, nextAttemptAt = null,
                lastError = null, now = System.currentTimeMillis(),
            )
            scheduleUpload()

        }
    }

    suspend fun retryAll() = withContext(Dispatchers.IO) {
        val now = System.currentTimeMillis()
        db.queueDao().resetAllPending(now)
        scheduleUpload()

    }

    suspend fun remove(itemId: String) = db.queueDao().delete(itemId)

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
