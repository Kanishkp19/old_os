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

    override suspend fun getForegroundInfo(): ForegroundInfo {
        val mgr = applicationContext.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        mgr.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Transfers", NotificationManager.IMPORTANCE_LOW)
        )
        val notif = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Home Hub")
            .setContentText("Preparing transfers…")
            .setOngoing(true)
            .build()
        return ForegroundInfo(42, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
    }

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        setForeground(getForegroundInfo())
        val pending = pipeline.processDue { item ->
            setForeground(TransferNotifications.foreground(applicationContext, item))
        }
        if (pending) Result.retry() else {
            TransferNotifications.completed(applicationContext)
            Result.success()
        }
    }
}
