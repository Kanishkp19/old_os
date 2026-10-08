package com.homehub.workers

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.ServiceInfo
import androidx.core.app.NotificationCompat
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.ForegroundInfo
import androidx.work.WorkerParameters
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@HiltWorker
class UploadWorker @AssistedInject constructor(
    @Assisted appContext: Context,
    @Assisted params: WorkerParameters,
    private val pipeline: UploadPipeline,
) : CoroutineWorker(appContext, params) {

    companion object {
        private const val CHANNEL_ID = "homehub-transfers"
    }

    override suspend fun getForegroundInfo() = TransferNotifications.foreground(applicationContext)

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        setForeground(getForegroundInfo())
        val pending = pipeline.processDue { item ->
            setForeground(TransferNotifications.foreground(applicationContext, item))
        }
        if (pending) Result.retry() else {
            TransferNotifications.completed(applicationContext, pipeline.hasOutstanding())
            Result.success()
        }
    }
}
