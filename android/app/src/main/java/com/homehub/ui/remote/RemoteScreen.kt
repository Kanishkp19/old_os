package com.homehub.ui.remote

import android.view.MotionEvent
import android.app.Activity
import android.content.Intent
import android.content.Context
import android.media.projection.MediaProjectionManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import com.homehub.screen.ScreenSession
import com.homehub.screen.ScreenCastService
import dagger.hilt.android.qualifiers.ApplicationContext
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
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
    val screen: ScreenSession,
    @ApplicationContext private val context: Context,
) : ViewModel() {

    val scopes get() = hub.scopes
    val allowed get() = hub.hasScope("remote")
    private var ws: WebSocket? = null
    private var pendingDx = 0
    private var pendingDy = 0
    private var flushing = false

    val error = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    private var connecting = false
    val connected = kotlinx.coroutines.flow.MutableStateFlow(false)

    fun connect() {
        if (ws != null || connecting) return
        connecting = true
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                hub.openRemoteInput(object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: okhttp3.Response) {
                        connected.value = true; connecting = false
                    }
                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                        connected.value = false; connecting = false; ws = null
                    }
                    override fun onFailure(webSocket: WebSocket, t: Throwable, response: okhttp3.Response?) {
                        connected.value = false; connecting = false; ws = null
                    }
                })
            }.onSuccess { ws = it; cacheWake() }.onFailure { connecting = false; error.value = com.homehub.ui.UserErrors.message(context, it) }
        }
    }

    fun disconnect() { ws?.close(1000, "done"); ws = null; connecting = false; connected.value = false }

    fun onMove(dx: Float, dy: Float) {
        pendingDx += dx.toInt(); pendingDy += dy.toInt()
        if (!flushing) {
            flushing = true
            viewModelScope.launch {
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

    fun power(action: String) = viewModelScope.launch {
        try { hub.post("/v1/remote/power", JSONObject().put("action", action).put("confirm", true)); error.value = null }
        catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    fun media(key: String) = viewModelScope.launch {
        try { hub.post("/v1/remote/media", JSONObject().put("key", key)); error.value = null }
        catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    private fun cacheWake() = viewModelScope.launch {
        runCatching { hub.get("/v1/remote/wake-info") }.onSuccess {
            context.getSharedPreferences("homehub_wake", Context.MODE_PRIVATE).edit().putString("info", it.toString()).apply()
        }
    }
    fun wake() = viewModelScope.launch(Dispatchers.IO) {
        try {
            val raw = context.getSharedPreferences("homehub_wake", Context.MODE_PRIVATE).getString("info", null)
                ?: hub.get("/v1/remote/wake-info").toString()
            val macs = JSONObject(raw).getJSONArray("macs")
            require(macs.length() > 0) { context.getString(R.string.wake_unavailable) }
            java.net.DatagramSocket().use { socket ->
                socket.broadcast = true
                for (i in 0 until macs.length()) {
                    val bytes = macs.getJSONObject(i).getString("mac").split(':', '-').map {
                        require(it.length == 2); it.toInt(16).toByte()
                    }.toByteArray()
                    require(bytes.size == 6)
                    val packet = ByteArray(102) { 0xff.toByte() }
                    repeat(16) { bytes.copyInto(packet, 6 + it * 6) }
                    socket.send(java.net.DatagramPacket(packet, packet.size, java.net.InetAddress.getByName("255.255.255.255"), 9))
                }
            }
            error.value = context.getString(R.string.wake_attempted)
        } catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    override fun onCleared() { disconnect(); if (screen.status.value != "casting") screen.stop() }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun RemoteScreen(vm: RemoteViewModel = hiltViewModel()) {
    val grantedScopes = vm.scopes.collectAsState().value
    val connected by vm.connected.collectAsState()
    val error by vm.error.collectAsState()
    val screenState by vm.screen.status.collectAsState()
    val screenError by vm.screen.error.collectAsState()
    val remoteVideo by vm.screen.remoteVideo.collectAsState()
    val context = LocalContext.current
    var powerAction by remember { mutableStateOf<String?>(null) }
    var screenConsent by remember { mutableStateOf(false) }
    val projection = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK && result.data != null) {
            androidx.core.content.ContextCompat.startForegroundService(context,
                Intent(context, ScreenCastService::class.java).putExtra("projection", result.data))
        }
    }
    DisposableEffect(Unit) { onDispose { vm.disconnect(); if (vm.screen.status.value != "casting") vm.screen.stop() } }
    var lastX by remember { mutableStateOf(0f) }
    var lastY by remember { mutableStateOf(0f) }
    var typedText by remember { mutableStateOf("") }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.remote_title), style = MaterialTheme.typography.headlineSmall)
        if (!vm.allowed) { Text(stringResource(R.string.error_permission)); return@Column }
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        screenError?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            TextButton(onClick = vm::wake) { Text(stringResource(R.string.remote_wake)) }
            TextButton(onClick = { powerAction = "sleep" }) { Text(stringResource(R.string.remote_sleep)) }
            TextButton(onClick = { powerAction = "restart" }) { Text(stringResource(R.string.remote_restart)) }
            TextButton(onClick = { powerAction = "shutdown" }) { Text(stringResource(R.string.remote_shutdown)) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            TextButton(onClick = { vm.media("play_pause") }) { Text(stringResource(R.string.media_play_pause)) }
            TextButton(onClick = { vm.media("prev") }) { Text(stringResource(R.string.media_prev)) }
            TextButton(onClick = { vm.media("next") }) { Text(stringResource(R.string.media_next)) }
            TextButton(onClick = { vm.media("mute") }) { Text(stringResource(R.string.media_mute)) }
        }
        Row {
            TextButton(onClick = { vm.media("vol_down") }) { Text(stringResource(R.string.media_vol_down)) }
            TextButton(onClick = { vm.media("vol_up") }) { Text(stringResource(R.string.media_vol_up)) }
        }
        if (screenState == "idle") Row {
            TextButton(onClick = vm.screen::startView) { Text(stringResource(R.string.screen_view)) }
            TextButton(onClick = { screenConsent = true }) { Text(stringResource(R.string.screen_cast)) }
        } else TextButton(onClick = vm.screen::stop) { Text(stringResource(R.string.screen_stop)) }
        remoteVideo?.let { track ->
            val eglContext = vm.screen.eglContext()
            if (eglContext != null) {
                val renderer = remember(track) { org.webrtc.SurfaceViewRenderer(context).apply { init(eglContext, null) } }
                DisposableEffect(track, renderer) {
                    track.addSink(renderer)
                    onDispose { track.removeSink(renderer); renderer.release() }
                }
                AndroidView(factory = { renderer }, modifier = Modifier.fillMaxWidth().height(220.dp))
            }
        }


        if (!connected) {
            Button(onClick = vm::connect, modifier = Modifier.align(Alignment.CenterHorizontally)) {
                Text(stringResource(R.string.remote_connect))
            }
        } else {
            // Trackpad surface
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(220.dp)
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
    powerAction?.let { action -> AlertDialog(onDismissRequest = { powerAction = null },
        title = { Text(stringResource(R.string.remote_power_confirm)) }, text = { Text(stringResource(R.string.remote_power_body)) },
        confirmButton = { TextButton(onClick = { vm.power(action); powerAction = null }) { Text(stringResource(R.string.action_done)) } },
        dismissButton = { TextButton(onClick = { powerAction = null }) { Text(stringResource(R.string.action_cancel)) } }) }
    if (screenConsent) AlertDialog(onDismissRequest = { screenConsent = false }, title = { Text(stringResource(R.string.screen_cast)) },
        text = { Text(stringResource(R.string.screen_consent)) }, confirmButton = { TextButton(onClick = {
            screenConsent = false
            val manager = context.getSystemService(MediaProjectionManager::class.java)
            projection.launch(manager.createScreenCaptureIntent())
        }) { Text(stringResource(R.string.cleanup_continue)) } }, dismissButton = { TextButton(onClick = { screenConsent = false }) { Text(stringResource(R.string.action_cancel)) } })

}
