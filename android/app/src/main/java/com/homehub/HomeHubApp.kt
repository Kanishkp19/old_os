package com.homehub

import android.app.Application
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class HomeHubApp : Application(), Configuration.Provider {
    override fun attachBaseContext(base: android.content.Context) { super.attachBaseContext(com.homehub.ui.LanguagePreference.wrap(base)) }

    @Inject lateinit var backup: com.homehub.backup.BackupRepository

    @Inject lateinit var connectivity: com.homehub.net.HubConnectivity

    override fun onCreate() { super.onCreate(); backup.schedule(); connectivity.start() }

    @Inject lateinit var workerFactory: HiltWorkerFactory

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setWorkerFactory(workerFactory)
            .build()
}
