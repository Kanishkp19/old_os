package com.homehub

import android.app.Application
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class HomeHubApp : Application(), Configuration.Provider {
    @Inject lateinit var backup: com.homehub.backup.BackupRepository

    override fun onCreate() { super.onCreate(); backup.schedule() }

    @Inject lateinit var workerFactory: HiltWorkerFactory

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setWorkerFactory(workerFactory)
            .build()
}
