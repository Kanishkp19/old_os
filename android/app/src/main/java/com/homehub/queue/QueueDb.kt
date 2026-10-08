package com.homehub.queue

import androidx.room.*
import kotlinx.coroutines.flow.Flow

/**
 * Client-side upload queue (BACKEND_SCHEMA §9). The queue is the source of
 * truth for "send later" (FR-3.4): items survive reboot, upload when the Hub
 * is reachable, and never delete the local source until the Hub confirms.
 *
 * Device private keys live in AndroidKeyStore, NEVER in this DB (SECURITY §3).
 */
@Entity(tableName = "queue_items")
data class QueueItem(
    @PrimaryKey val id: String,
    @ColumnInfo(name = "hub_id") val hubId: String,
    @ColumnInfo(name = "source_uri") val sourceUri: String, // content:// or file URL
    @ColumnInfo(name = "client_item_id") val clientItemId: String,
    val name: String,
    val size: Long,
    val mime: String?,
    val kind: String,                        // send | backup
    @ColumnInfo(name = "root_hash") val rootHash: String?,   // computed lazily
    @ColumnInfo(name = "source_mtime") val sourceMtime: Long?,
    @ColumnInfo(name = "transfer_id") val transferId: String?, // from Hub once created
    val state: String,                       // queued|connecting|uploading|verifying|done|failed_retry|failed_perm
    val attempts: Int = 0,
    @ColumnInfo(name = "next_attempt_at") val nextAttemptAt: Long?,
    @ColumnInfo(name = "last_error") val lastError: String?,
    @ColumnInfo(name = "created_at") val createdAt: Long,
    @ColumnInfo(name = "updated_at") val updatedAt: Long,
    @ColumnInfo(name = "backup_source_id") val backupSourceId: String? = null,
    @ColumnInfo(name = "taken_at") val takenAt: Long? = null,
    @ColumnInfo(name = "result_file_id") val resultFileId: String? = null,
    @ColumnInfo(name = "bytes_sent", defaultValue = "0") val bytesSent: Long = 0,
) {
    companion object {
        const val CANCELLED = "cancelled"
        const val QUEUED = "queued"
        const val CONNECTING = "connecting"
        const val UPLOADING = "uploading"
        const val VERIFYING = "verifying"
        const val DONE = "done"
        const val FAILED_RETRY = "failed_retry"
        const val FAILED_PERM = "failed_perm"
    }
}

@Entity(tableName = "hub_trust")
data class HubTrust(
    @PrimaryKey @ColumnInfo(name = "hub_id") val hubId: String,
    val name: String?,
    @ColumnInfo(name = "ca_fingerprint") val caFingerprint: String,
    @ColumnInfo(name = "ca_cert_pem") val caCertPem: String,
    @ColumnInfo(name = "last_addr") val lastAddr: String?,
    @ColumnInfo(name = "paired_at") val pairedAt: Long,
)

@Dao
interface QueueDao {
    @Query("SELECT * FROM queue_items ORDER BY created_at DESC")
    fun observeAll(): Flow<List<QueueItem>>

    @Query("SELECT * FROM queue_items WHERE state IN ('queued','failed_retry') AND (next_attempt_at IS NULL OR next_attempt_at <= :now) ORDER BY created_at ASC")
    suspend fun due(now: Long): List<QueueItem>

    @Query("SELECT * FROM queue_items WHERE id = :id")
    suspend fun get(id: String): QueueItem?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(item: QueueItem)

    @Query("UPDATE queue_items SET state = :state, transfer_id = :transferId, root_hash = :rootHash, attempts = :attempts, next_attempt_at = :nextAttemptAt, last_error = :lastError, updated_at = :now WHERE id = :id AND state != 'cancelled'")
    suspend fun updateProgress(
        id: String, state: String, transferId: String?, rootHash: String?,
        attempts: Int, nextAttemptAt: Long?, lastError: String?, now: Long,
    )

    @Query("UPDATE queue_items SET size=:size, source_mtime=:modified, client_item_id=:clientId, root_hash=:hash, transfer_id=:transferId, bytes_sent=:bytes WHERE id=:id AND state != 'cancelled'")
    suspend fun refreshSource(id: String, size: Long, modified: Long?, clientId: String, hash: String?, transferId: String?, bytes: Long)

    @Query("SELECT * FROM queue_items WHERE hub_id = :hubId AND client_item_id = :clientItemId AND kind = :kind ORDER BY created_at DESC LIMIT 1")
    suspend fun findSource(hubId: String, clientItemId: String, kind: String): QueueItem?

    @Query("UPDATE queue_items SET bytes_sent = :bytes, updated_at = :now WHERE id = :id")
    suspend fun setBytes(id: String, bytes: Long, now: Long)

    @Query("UPDATE queue_items SET result_file_id = :fileId WHERE id = :id")
    suspend fun setResult(id: String, fileId: String)

    @Query("UPDATE queue_items SET state='queued' WHERE state IN ('connecting','uploading','verifying')")
    suspend fun recoverInterrupted()

    @Query("SELECT COUNT(*) FROM queue_items WHERE state NOT IN ('done','failed_perm','cancelled')")
    fun observePendingCount(): Flow<Int>

    @Query("UPDATE queue_items SET state = 'queued', attempts = 0, next_attempt_at = null, last_error = null, updated_at = :now WHERE state NOT IN ('done','cancelled')")
    suspend fun resetAllPending(now: Long)

    @Query("SELECT COUNT(*) FROM queue_items WHERE hub_id=:hubId AND state NOT IN ('done','cancelled')")
    suspend fun outstandingCount(hubId: String): Int

    @Query("UPDATE queue_items SET state='cancelled', updated_at=:now WHERE id=:id AND state != 'done'")
    suspend fun cancel(id: String, now: Long)

    @Query("DELETE FROM queue_items WHERE id = :id")
    suspend fun delete(id: String)
}

@Dao
interface HubTrustDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(trust: HubTrust)

    @Query("SELECT * FROM hub_trust ORDER BY paired_at DESC LIMIT 1")
    suspend fun current(): HubTrust?

    @Query("SELECT * FROM hub_trust ORDER BY paired_at DESC LIMIT 1")
    fun observeCurrent(): Flow<HubTrust?>

    @Query("UPDATE hub_trust SET last_addr = :addr WHERE hub_id = :hubId")
    suspend fun updateAddr(hubId: String, addr: String)
}

@Database(entities = [QueueItem::class, HubTrust::class], version = 2, exportSchema = true)
abstract class QueueDb : RoomDatabase() {
    companion object {
        val MIGRATION_1_2 = object : androidx.room.migration.Migration(1, 2) {
            override fun migrate(db: androidx.sqlite.db.SupportSQLiteDatabase) {
                db.execSQL("ALTER TABLE queue_items ADD COLUMN backup_source_id TEXT")
                db.execSQL("ALTER TABLE queue_items ADD COLUMN taken_at INTEGER")
                db.execSQL("ALTER TABLE queue_items ADD COLUMN result_file_id TEXT")
                db.execSQL("ALTER TABLE queue_items ADD COLUMN bytes_sent INTEGER NOT NULL DEFAULT 0")
            }
        }
    }
    abstract fun queueDao(): QueueDao
    abstract fun hubTrustDao(): HubTrustDao
}
