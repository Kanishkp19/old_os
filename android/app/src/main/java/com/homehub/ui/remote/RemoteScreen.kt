package com.homehub.ui.remote

import android.view.MotionEvent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInteropFilter
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.net.HubClient
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import javax.inject.Inject

/**
 * Phone-as-trackpad (FR-4.1). Pointer deltas are coalesced and sent as
 * {"t":"mv",dx,dy} frames — the Hub applies the real cursor movement through
 * the Session-0 helper (TRD §8). 60 s of inactivity ends the session
 * server-side, so the UI reconnects on demand.
 */
@HiltViewModel
class RemoteViewModel @Inject constructor(
    private val hub: HubClient,
) : ViewModel() {

    private var ws: WebSocket? = null
    private var pendingDx = 0
    private var pendingDy = 0
    private var flushing = false

    val connected = kotlinx.coroutines.flow.MutableStateFlow(false)

    fun connect() {
        if (ws != null) return
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                hub.openRemoteInput(object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: okhttp3.Response) {
                        connected.value = true
                    }
                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                        connected.value = false; ws = null
                    }
                    override fun onFailure(webSocket: WebSocket, t: Throwable, response: okhttp3.Response?) {
                        connected.value = false; ws = null
                    }
                })
            }.onSuccess { ws = it }
        }
    }

    fun disconnect() { ws?.close(1000, "done"); ws = null; connected.value = false }

    fun onMove(dx: Float, dy: Float) {
        pendingDx += dx.toInt(); pendingDy += dy.toInt()
        if (!flushing) {
            flushing = true
            viewModelScope.launch(Dispatchers.IO) {
                kotlinx.coroutines.delay(16) // ~60 msg/s, well under the 500/s limit
                val msg = JSONObject()
                    .put("t", "mv").put("dx", pendingDx).put("dy", pendingDy).toString()
                pendingDx = 0; pendingDy = 0; flushing = false
                ws?.send(msg)
            }
        }
    }

    fun click(button: String) =
        ws?.send(JSONObject().put("t", "click").put("b", button).toString())

    fun scroll(dy: Int) =
        ws?.send(JSONObject().put("t", "scroll").put("dy", dy).toString())

    fun sendText(s: String) =
        ws?.send(JSONObject().put("t", "text").put("s", s).toString())

    override fun onCleared() = disconnect()
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun RemoteScreen(vm: RemoteViewModel = hiltViewModel()) {
    val connected by vm.connected.collectAsState()
    var lastX by remember { mutableStateOf(0f) }
    var lastY by remember { mutableStateOf(0f) }
    var typedText by remember { mutableStateOf("") }

    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.remote_title), style = MaterialTheme.typography.headlineSmall)

        if (!connected) {
            Button(onClick = vm::connect, modifier = Modifier.align(Alignment.CenterHorizontally)) {
                Text(stringResource(R.string.remote_connect))
            }
        } else {
            // Trackpad surface
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .pointerInteropFilter { ev ->
                        when (ev.action) {
                            MotionEvent.ACTION_DOWN -> { lastX = ev.x; lastY = ev.y }
                            MotionEvent.ACTION_MOVE -> vm.onMove(ev.x - lastX, ev.y - lastY).also {
                                lastX = ev.x; lastY = ev.y
                            }
                        }
                        true
                    },
                tonalElevation = 2.dp,
                shape = MaterialTheme.shapes.large,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Text(stringResource(R.string.remote_trackpad_hint),
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { vm.click("left") }, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.remote_left_click))
                }
                OutlinedButton(onClick = { vm.click("right") }, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.remote_right_click))
                }
            }

            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = typedText, onValueChange = { typedText = it },
                    modifier = Modifier.weight(1f),
                    placeholder = { Text(stringResource(R.string.remote_type_hint)) },
                    singleLine = true,
                )
                Button(onClick = { vm.sendText(typedText); typedText = "" }) {
                    Text(stringResource(R.string.action_send))
                }
            }

            TextButton(onClick = vm::disconnect, modifier = Modifier.align(Alignment.CenterHorizontally)) {
                Text(stringResource(R.string.remote_disconnect))
            }
        }
    }
}
