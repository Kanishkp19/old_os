package com.homehub.di

import android.content.Context
import androidx.room.Room
import com.homehub.queue.QueueDb
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AppModule {

    @Provides
    @Singleton
    fun provideQueueDb(@ApplicationContext context: Context): QueueDb =
        Room.databaseBuilder(context, QueueDb::class.java, "homehub.db")
            .addMigrations(QueueDb.MIGRATION_1_2)
            .build()

    @Provides fun provideQueueDao(db: QueueDb) = db.queueDao()
    @Provides fun provideHubTrustDao(db: QueueDb) = db.hubTrustDao()
}
