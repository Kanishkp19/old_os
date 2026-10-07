package com.homehub.ui.home

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.net.DiscoveredHub
import com.homehub.net.HubClient
import com.homehub.net.HubDiscovery
import com.homehub.queue.HubTrustStore
import com.homehub.queue.QueueItem
import com.homehub.queue.QueueRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch
import javax.inject.Inject

data class HomeUiState(
    val paired: Boolean = false,
    val hubName: String = "",
    val hubReachable: Boolean = false,
    val pendingCount: Int = 0,
    val hasClientCert: Boolean = false,
    val queueItems: List<QueueItem> = emptyList(),
)

@HiltViewModel
class HomeViewModel @Inject constructor(
    @ApplicationContext private val context: Context,
    private val trustStore: HubTrustStore,
    private val discovery: HubDiscovery,
    private val queue: QueueRepository,
    private val hub: HubClient,
) : ViewModel() {

    private val reachable = MutableStateFlow(false)

    val state: StateFlow<HomeUiState> = combine(
        trustStore.observe(),
        queue.observeQueue(),
        reachable,
    ) { trust, items, up ->
        val hasCertPem = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE)
            .contains("client_cert_pem")
        val canSign = try {
            val ks = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            val key = ks.getKey("homehub-device", null) as? java.security.PrivateKey
            if (key != null) {
                val sig = java.security.Signature.getInstance("NONEwithECDSA")
                sig.initSign(key)
                true
            } else {
                false
            }
        } catch (_: Throwable) {
            false
        }
        val hasCert = hasCertPem && canSign
        val pending = items.count { it.state != QueueItem.DONE && it.state != QueueItem.FAILED_PERM }
        HomeUiState(
            paired = trust != null,
            hubName = trust?.name ?: "",
            hubReachable = up,
            pendingCount = pending,
            hasClientCert = hasCert,
            queueItems = items.take(10),
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), HomeUiState())

    init {
        // Watch for the paired Hub on the LAN and refresh its last known address.
        viewModelScope.launch {
            discovery.browseHubs().collect { hubDiscovered: DiscoveredHub ->
                val trust = trustStore.load() ?: return@collect
                if (hubDiscovered.hubId == trust.hubId) {
                    reachable.value = true
                    trustStore.updateAddr(trust.hubId, "${hubDiscovered.host}:${hubDiscovered.port}")
                }
            }
        }
        // Direct reachability ping
        viewModelScope.launch(Dispatchers.IO) {
            try {
                hub.info()
                reachable.value = true
            } catch (_: Exception) {}
        }
    }

    fun retryUploads() {
        viewModelScope.launch {
            queue.retryAll()
        }
    }

    fun enqueue(uris: List<Uri>) {
        queue.enqueue(uris)
    }
}

@Composable
fun HomeScreen(onPair: () -> Unit, vm: HomeViewModel = hiltViewModel()) {
    val s by vm.state.collectAsState()
    val filePicker = rememberLauncherForActivityResult(
        ActivityResultContracts.GetMultipleContents()
    ) { uris ->
        if (uris.isNotEmpty()) vm.enqueue(uris)
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium)
        }

        if (!s.paired) {
            item {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(stringResource(R.string.home_not_paired_title), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.home_not_paired_body))
                        Button(onClick = onPair, modifier = Modifier.align(Alignment.End)) {
                            Text(stringResource(R.string.action_pair))
                        }
                    }
                }
            }
        } else if (!s.hasClientCert) {
            item {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
                ) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text(
                            "Pairing update required",
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onErrorContainer,
                        )
                        Text(
                            "To send files securely over mutual TLS, please scan the QR code on your computer once more.",
                            color = MaterialTheme.colorScheme.onErrorContainer,
                        )
                        Button(onClick = onPair, modifier = Modifier.align(Alignment.End)) {
                            Text("Scan QR Code to finish setup")
                        }
                    }
                }
            }
        } else {
            item {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(s.hubName, style = MaterialTheme.typography.titleMedium)
                            TextButton(onClick = onPair) {
                                Text("Re-pair")
                            }
                        }
                        Text(
                            stringResource(if (s.hubReachable) R.string.home_hub_nearby else R.string.home_hub_away),
                            color = if (s.hubReachable) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            item {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Row(
                        modifier = Modifier.padding(16.dp).fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Button(
                            onClick = { filePicker.launch("*/*") },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Choose file to send")
                        }
                        if (s.pendingCount > 0) {
                            OutlinedButton(onClick = { vm.retryUploads() }) {
                                Text("Send now (${s.pendingCount})")
                            }
                        }
                    }
                }
            }

            if (s.queueItems.isNotEmpty()) {
                item {
                    Text(
                        "Transfers",
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }

                items(s.queueItems, key = { it.id }) { item ->
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Column(modifier = Modifier.padding(12.dp)) {
                            Text(item.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1)
                            Spacer(Modifier.height(4.dp))
                            Text(
                                stateText(item),
                                style = MaterialTheme.typography.bodySmall,
                                color = when (item.state) {
                                    QueueItem.FAILED_PERM -> MaterialTheme.colorScheme.error
                                    QueueItem.FAILED_RETRY -> MaterialTheme.colorScheme.error
                                    QueueItem.DONE -> MaterialTheme.colorScheme.primary
                                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun stateText(item: QueueItem): String = when (item.state) {
    QueueItem.QUEUED -> "Waiting to send"
    QueueItem.CONNECTING -> "Connecting to Home…"
    QueueItem.UPLOADING -> "Sending…"
    QueueItem.VERIFYING -> "Checking it arrived safely…"
    QueueItem.DONE -> "Sent"
    QueueItem.FAILED_RETRY -> "Will retry soon" + (item.lastError?.let { ": $it" } ?: "")
    else -> "Couldn't send: " + (item.lastError ?: "")
}
