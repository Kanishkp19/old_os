package com.homehub.backup

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.homehub.workers.TransferNotifications
import com.homehub.workers.UploadPipeline
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@HiltWorker
class BackupWorker @AssistedInject constructor(@Assisted context: Context, @Assisted params: WorkerParameters,
    private val backup: BackupRepository, private val pipeline: UploadPipeline) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        if (!BackupRules.allowed(applicationContext)) return@withContext Result.success()
        setForeground(TransferNotifications.foreground(applicationContext))
        try {
            backup.scanAndQueue()
            backup.status.value = null
            val pending = pipeline.processDue { setForeground(TransferNotifications.foreground(applicationContext, it)) }
            if (pending) Result.retry() else { TransferNotifications.completed(applicationContext, pipeline.hasOutstanding()); Result.success() }
        } catch (e: CancellationException) { throw e }
        catch (e: SecurityException) { backup.status.value = applicationContext.getString(com.homehub.R.string.backup_permission_required); Result.failure() }
        catch (e: Exception) { backup.status.value = com.homehub.ui.UserErrors.message(applicationContext, e); Result.retry() }
    }
}
