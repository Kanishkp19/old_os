package com.homehub.net

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import com.homehub.queue.HubTrustStore
import com.homehub.queue.QueueRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class HubConnectivity @Inject constructor(@ApplicationContext private val context: Context,
    private val discovery: HubDiscovery, private val trust: HubTrustStore,
    private val hub: HubClient, private val queue: QueueRepository, private val backup: com.homehub.backup.BackupRepository) {
    val reachable = MutableStateFlow(false)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var started = false
    private var refreshJob: Job? = null
    @Synchronized fun start() {
        if (started) return
        started = true
        context.getSystemService(ConnectivityManager::class.java).registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) { refresh() }
            override fun onLost(network: Network) { reachable.value = false }
        })
        scope.launch {
            discovery.browseHubs().collect { found ->
                val current = trust.load() ?: return@collect
                if (found.hubId == current.hubId && com.homehub.screen.LanSdp.privateAddress(found.host)) {
                    val host = if (found.host.contains(':')) "[${found.host}]" else found.host
                    trust.updateAddr(current.hubId, "$host:${found.port}")
                    refresh()
                }
            }
        }
        refresh()
    }
    @Synchronized fun refresh() {
        if (refreshJob?.isActive == true) return
        refreshJob = scope.launch {
            try {
                val current = trust.load() ?: return@launch
                val info = hub.info()
                reachable.value = info.getString("hub_id") == current.hubId
                if (reachable.value) { hub.refreshScopes(); backup.applyGlobalSchedule(info.optLong("backup_interval_minutes", 15)); queue.scheduleUpload() }
            } catch (e: CancellationException) { throw e }
            catch (_: Exception) { reachable.value = false }
        }
    }
}
