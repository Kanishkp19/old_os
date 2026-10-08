package com.homehub.screen

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.homehub.R
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

/** Starts only after the system MediaProjection consent activity returns OK. */
@AndroidEntryPoint
class ScreenCastService : Service() {
    @Inject lateinit var session: ScreenSession
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "stop") { session.stop(); stopSelf(); return START_NOT_STICKY }
        @Suppress("DEPRECATION") val permission = intent?.getParcelableExtra<Intent>("projection")
        if (permission == null) { stopSelf(); return START_NOT_STICKY }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel("homehub-screen", getString(R.string.screen_cast), NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 99, Intent(this, ScreenCastService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val notification = NotificationCompat.Builder(this, "homehub-screen")
            .setSmallIcon(android.R.drawable.ic_menu_view).setContentTitle(getString(R.string.screen_cast))
            .setContentText(getString(R.string.screen_cast_active)).setOngoing(true)
            .addAction(android.R.drawable.ic_media_pause, getString(R.string.remote_disconnect), stop).build()
        ServiceCompat.startForeground(this, 99, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        session.startCast(permission)
        return START_NOT_STICKY
    }
    override fun onDestroy() { session.stop(); super.onDestroy() }
}
