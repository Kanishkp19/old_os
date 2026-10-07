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
) {
    companion object {
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

    @Query("UPDATE queue_items SET state = :state, transfer_id = COALESCE(:transferId, transfer_id), root_hash = COALESCE(:rootHash, root_hash), attempts = :attempts, next_attempt_at = :nextAttemptAt, last_error = :lastError, updated_at = :now WHERE id = :id")
    suspend fun updateProgress(
        id: String, state: String, transferId: String?, rootHash: String?,
        attempts: Int, nextAttemptAt: Long?, lastError: String?, now: Long,
    )

    @Query("SELECT COUNT(*) FROM queue_items WHERE state != 'done' AND state != 'failed_perm'")
    fun observePendingCount(): Flow<Int>

    @Query("UPDATE queue_items SET state = 'queued', attempts = 0, next_attempt_at = null, last_error = null, updated_at = :now WHERE state != 'done'")
    suspend fun resetAllPending(now: Long)

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

@Database(entities = [QueueItem::class, HubTrust::class], version = 1, exportSchema = true)
abstract class QueueDb : RoomDatabase() {
    abstract fun queueDao(): QueueDao
    abstract fun hubTrustDao(): HubTrustDao
}
