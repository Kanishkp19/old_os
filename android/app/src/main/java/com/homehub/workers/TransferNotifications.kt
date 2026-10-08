package com.homehub.workers

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.ServiceInfo
import androidx.core.app.NotificationCompat
import androidx.work.ForegroundInfo
import com.homehub.R
import com.homehub.queue.QueueItem

object TransferNotifications {
    private const val CHANNEL = "homehub-transfers"
    fun foreground(context: Context, item: QueueItem? = null): ForegroundInfo {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, context.getString(R.string.transfers_title), NotificationManager.IMPORTANCE_LOW))
        val text = if (item == null) context.getString(R.string.state_connecting)
            else context.getString(if (item.state == QueueItem.VERIFYING) R.string.state_verifying else R.string.state_uploading)
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_upload).setContentTitle(context.getString(R.string.app_name))
            .setContentText(text).setOngoing(true)
            .setContentIntent(android.app.PendingIntent.getActivity(context, 42,
                android.content.Intent(context, com.homehub.MainActivity::class.java).putExtra("route", "transfers"),
                android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT))
            .setProgress(100, item?.let { if (it.size > 0) (100 * it.bytesSent / it.size).toInt() else 0 } ?: 0, item == null)
            .build()
        return ForegroundInfo(42, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
    }
    fun completed(context: Context, hasFailures: Boolean = false) {
        runCatching { context.getSystemService(NotificationManager::class.java).notify(44,
            NotificationCompat.Builder(context, CHANNEL).setSmallIcon(android.R.drawable.stat_sys_upload_done)
                .setContentTitle(context.getString(R.string.app_name)).setContentText(context.getString(if (hasFailures) R.string.transfer_attention else R.string.backup_completed))
                .setContentIntent(android.app.PendingIntent.getActivity(context, 44, android.content.Intent(context, com.homehub.MainActivity::class.java).putExtra("route", "transfers"), android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT)).build()) }
    }
}
