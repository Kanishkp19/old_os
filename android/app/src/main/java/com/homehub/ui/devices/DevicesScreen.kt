package com.homehub.ui.devices

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.net.HubClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject
import javax.inject.Inject

data class DeviceRow(
    val id: String, val name: String, val platform: String,
    val status: String, val isSelf: Boolean,
)

/**
 * Device management (FR-5.3, SECURITY §6): list paired devices; revoking a
 * device flips it server-side and takes effect on its NEXT TLS handshake —
 * the session it is on right now ends with the connection.
 */
@HiltViewModel
class DevicesViewModel @Inject constructor(
    private val hub: HubClient,
    @dagger.hilt.android.qualifiers.ApplicationContext private val context: android.content.Context,
) : ViewModel() {

    private val _devices = MutableStateFlow<List<DeviceRow>>(emptyList())
    val devices: StateFlow<List<DeviceRow>> = _devices

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error

    fun refresh() = viewModelScope.launch {
        try {
            val arr = hub.devices()
            _devices.value = (0 until arr.length()).map { arr.getJSONObject(it) }.map { d: JSONObject ->
                DeviceRow(
                    id = d.getString("id"),
                    name = d.getString("name"),
                    platform = d.optString("platform", "?"),
                    status = d.getString("status"),
                    isSelf = d.optBoolean("is_self", false) || d.getString("id") == context.getSharedPreferences("homehub_auth", android.content.Context.MODE_PRIVATE).getString("device_id", null),
                )
            }
            _error.value = null
        } catch (e: Exception) {
            _error.value = e.message
        }
    }

    fun revoke(id: String) = viewModelScope.launch {
        try { hub.revokeDevice(id); refresh() } catch (e: Exception) { _error.value = e.message }
    }
}

@Composable
fun DevicesScreen(vm: DevicesViewModel = hiltViewModel()) {
    val devices by vm.devices.collectAsState()
    val error by vm.error.collectAsState()
    var confirmRevoke by remember { mutableStateOf<DeviceRow?>(null) }

    LaunchedEffect(Unit) { vm.refresh() }

    Column(Modifier.fillMaxSize().padding(16.dp)) {
        Text(stringResource(R.string.devices_title), style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(12.dp))
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(devices, key = { it.id }) { d ->
                Card(Modifier.fillMaxWidth()) {
                    Row(Modifier.padding(12.dp).fillMaxWidth()) {
                        Column(Modifier.weight(1f)) {
                            Text(d.name, style = MaterialTheme.typography.bodyLarge)
                            Text(
                                "${d.platform} · ${d.status}" + if (d.isSelf) " · ${stringResource(R.string.devices_this_one)}" else "",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (!d.isSelf && d.status == "active") {
                            TextButton(onClick = { confirmRevoke = d }) {
                                Text(stringResource(R.string.action_revoke), color = MaterialTheme.colorScheme.error)
                            }
                        }
                    }
                }
            }
        }
    }

    confirmRevoke?.let { d ->
        AlertDialog(
            onDismissRequest = { confirmRevoke = null },
            title = { Text(stringResource(R.string.revoke_title)) },
            text = { Text(stringResource(R.string.revoke_body, d.name)) },
            confirmButton = {
                TextButton(onClick = { vm.revoke(d.id); confirmRevoke = null }) {
                    Text(stringResource(R.string.action_revoke), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRevoke = null }) { Text(stringResource(R.string.action_cancel)) }
            },
        )
    }
}
