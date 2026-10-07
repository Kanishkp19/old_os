package com.homehub.workers

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Foreground service for interactive photo-backup runs
 * (FR-2.1: bulk first backup can take hours; a WorkManager foreground alone
 * is capped, so long user-initiated backups pin a dataSync service).
 *
 * The actual upload work still runs in [UploadWorker]; this service only
 * keeps the process alive and shows progress.
 */
class BackupForegroundService : Service() {

    companion object {
        private const val CHANNEL_ID = "homehub-backup"
        private const val NOTIF_ID = 43
        const val ACTION_START = "com.homehub.action.BACKUP_START"
        const val ACTION_STOP = "com.homehub.action.BACKUP_STOP"

        fun start(context: Context) {
            val i = Intent(context, BackupForegroundService::class.java).setAction(ACTION_START)
            if (Build.VERSION.SDK_INT >= 26) context.startForegroundService(i) else context.startService(i)
        }

        fun stop(context: Context) =
            context.startService(Intent(context, BackupForegroundService::class.java).setAction(ACTION_STOP))
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startForegroundWithNotification()
            ACTION_STOP -> {
                ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
        }
        return START_NOT_STICKY
    }

    private fun startForegroundWithNotification() {
        val mgr = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        mgr.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Photo backup", NotificationManager.IMPORTANCE_LOW)
        )
        val notif: Notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Home Hub")
            .setContentText("Backing up your photos to Home…")
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIF_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(NOTIF_ID, notif)
        }
    }
}
