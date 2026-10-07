package com.homehub.queue

import com.homehub.net.PairingClient
import com.homehub.net.QrPayload
import javax.inject.Inject
import javax.inject.Singleton

/** Paired-hub trust material, minus the private key (which stays in AndroidKeyStore). */
data class Trust(
    val hubId: String,
    val name: String,
    val caFingerprint: String,
    val caCertPem: String,
    val lastAddr: String,
)

/**
 * Read/write access to the `hub_trust` table (BACKEND_SCHEMA §9).
 * Written once at pairing; `lastAddr` refreshed on every successful discovery.
 */
@Singleton
class HubTrustStore @Inject constructor(
    private val db: QueueDb,
) {
    suspend fun load(): Trust? = db.hubTrustDao().current()?.let {
        Trust(
            hubId = it.hubId,
            name = it.name ?: "Home Hub",
            caFingerprint = it.caFingerprint,
            caCertPem = it.caCertPem,
            lastAddr = it.lastAddr ?: return@let null,
        )
    }

    fun observe() = db.hubTrustDao().observeCurrent()

    suspend fun savePairing(payload: QrPayload, result: PairingClient.PairResult) {
        val rawAddr = payload.addrs.firstOrNull() ?: ""
        val host = rawAddr.substringBefore(':')
        val addr = if (host.isNotEmpty()) "$host:47800" else rawAddr
        db.hubTrustDao().upsert(
            HubTrust(
                hubId = payload.hubId,
                name = result.hubName,
                caFingerprint = payload.caFingerprint,
                caCertPem = result.caCertPem,
                lastAddr = addr,
                pairedAt = System.currentTimeMillis(),
            )
        )
    }

    suspend fun updateAddr(hubId: String, addr: String) {
        val host = addr.substringBefore(':')
        val apiAddr = if (host.isNotEmpty()) "$host:47800" else addr
        db.hubTrustDao().updateAddr(hubId, apiAddr)
    }
}
