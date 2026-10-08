package com.homehub.queue

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class QueueMigrationTest {
    @Test fun upgradePreservesSourcesAndCancellationSurvivesWorkerRecovery() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val name = "migration-${java.util.UUID.randomUUID()}.db"
        val old = context.openOrCreateDatabase(name, 0, null)
        old.execSQL("CREATE TABLE queue_items (id TEXT NOT NULL PRIMARY KEY,hub_id TEXT NOT NULL,source_uri TEXT NOT NULL,client_item_id TEXT NOT NULL,name TEXT NOT NULL,size INTEGER NOT NULL,mime TEXT,kind TEXT NOT NULL,root_hash TEXT,source_mtime INTEGER,transfer_id TEXT,state TEXT NOT NULL,attempts INTEGER NOT NULL,next_attempt_at INTEGER,last_error TEXT,created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL)")
        old.execSQL("CREATE TABLE hub_trust (hub_id TEXT NOT NULL PRIMARY KEY,name TEXT,ca_fingerprint TEXT NOT NULL,ca_cert_pem TEXT NOT NULL,last_addr TEXT,paired_at INTEGER NOT NULL)")
        old.execSQL("INSERT INTO queue_items VALUES('kept','home','content://source/1','source','photo.jpg',20,NULL,'send',NULL,NULL,'session','uploading',0,NULL,NULL,1,1)")
        old.version = 1; old.close()
        val db = Room.databaseBuilder(context, QueueDb::class.java, name).addMigrations(QueueDb.MIGRATION_1_2).build()
        try { withContext(Dispatchers.IO) {
            val item = requireNotNull(db.queueDao().get("kept"))
            assertEquals("content://source/1", item.sourceUri); assertEquals(0L, item.bytesSent)
            db.queueDao().cancel(item.id, 2)
            db.queueDao().recoverInterrupted()
            db.queueDao().updateProgress(item.id, QueueItem.DONE, item.transferId, null, 0, null, null, 3)
            assertEquals(QueueItem.CANCELLED, db.queueDao().get(item.id)?.state)
        } } finally { db.close(); context.deleteDatabase(name) }
    }
}
