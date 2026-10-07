package com.homehub.net

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import javax.inject.Inject
import javax.inject.Singleton

data class DiscoveredHub(
    val host: String,
    val port: Int,
    val hubId: String?,
    val name: String?,
    val caFingerprint: String?,
)

/**
 * mDNS discovery via NsdManager (API_SPEC §2). No aggressive polling:
 * this is a browse listener plus ConnectivityManager triggers (AGENTS.md §6).
 */
@Singleton
class HubDiscovery @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    fun browseHubs(): Flow<DiscoveredHub> = callbackFlow {
        val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
        // Multicast lock is required for mDNS on many devices.
        val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val lock = wifi.createMulticastLock("homehub").apply { setReferenceCounted(true); acquire() }

        val listener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(regType: String) {}
            override fun onServiceFound(service: NsdServiceInfo) {
                nsd.resolveService(service, object : NsdManager.ResolveListener {
                    override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {}
                    override fun onServiceResolved(info: NsdServiceInfo) {
                        val txt = info.attributes
                        val hub = DiscoveredHub(
                            host = info.host.hostAddress ?: return,
                            port = info.port,
                            hubId = txt["id"]?.let { String(it) },
                            name = txt["name"]?.let { String(it) },
                            caFingerprint = txt["fp"]?.let { String(it) },
                        )
                        trySend(hub)
                    }
                })
            }
            override fun onServiceLost(service: NsdServiceInfo) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {}
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
        }
        nsd.discoverServices("_homehub._tcp", NsdManager.PROTOCOL_DNS_SD, listener)
        awaitClose {
            runCatching { nsd.stopServiceDiscovery(listener) }
            runCatching { lock.release() }
        }
    }
}
