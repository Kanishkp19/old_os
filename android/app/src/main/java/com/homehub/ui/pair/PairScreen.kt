package com.homehub.ui.pair

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import com.homehub.R
import com.homehub.net.PairingClient
import com.homehub.net.QrPayload
import com.homehub.queue.HubTrustStore
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import java.util.concurrent.Executors
import javax.inject.Inject

/**
 * QR pairing (UI_UX §4.1, TRD §5): scan the code shown on the Hub dashboard.
 * The QR carries the one-time token AND the CA fingerprint; the client
 * verifies the fingerprint over TLS before the token is ever sent (T3).
 */
@HiltViewModel
class PairViewModel @Inject constructor(
    private val pairing: PairingClient,
    private val trustStore: HubTrustStore,
    private val hubClient: com.homehub.net.HubClient,
    @dagger.hilt.android.qualifiers.ApplicationContext private val context: android.content.Context,
) : ViewModel() {

    sealed class UiState {
        data object Scanning : UiState()
        data object Working : UiState()
        data class Done(val hubName: String) : UiState()
        data class Failed(val message: String) : UiState()
    }

    private val _state = MutableStateFlow<UiState>(UiState.Scanning)
    val state: StateFlow<UiState> = _state

    fun onQrScanned(raw: String) {
        if (_state.value != UiState.Scanning) return
        val payload = runCatching { QrPayload.parse(raw) }.getOrNull()
        if (payload == null) {
            _state.value = UiState.Failed(context.getString(R.string.pair_invalid))
            return
        }
        _state.value = UiState.Working
        viewModelScope.launch {
            try {
                val result = pairing.pair(payload)
                trustStore.savePairing(payload, result)
                hubClient.resetClient()
                _state.value = UiState.Done(result.hubName)
            } catch (e: Exception) {
                _state.value = UiState.Failed(com.homehub.ui.UserErrors.message(context, e))
            }
        }
    }

    fun reset() { _state.value = UiState.Scanning }
}

@Composable
fun PairScreen(onDone: () -> Unit, vm: PairViewModel = hiltViewModel()) {
    val state by vm.state.collectAsState()
    val context = LocalContext.current
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.CAMERA
            ) == PackageManager.PERMISSION_GRANTED
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission(),
        onResult = { granted ->
            hasCameraPermission = granted
        }
    )

    LaunchedEffect(Unit) {
        if (!hasCameraPermission) {
            permissionLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(stringResource(R.string.pair_title), style = MaterialTheme.typography.headlineSmall)
        Text(stringResource(R.string.pair_hint))

        when (val s = state) {
            is PairViewModel.UiState.Scanning -> {
                if (hasCameraPermission) {
                    QrScanner(onCode = vm::onQrScanned)
                } else {
                    Card(modifier = Modifier.fillMaxWidth().padding(8.dp)) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Text(stringResource(R.string.pair_camera_permission))
                            Button(onClick = { permissionLauncher.launch(Manifest.permission.CAMERA) }) {
                                Text(stringResource(R.string.pair_grant_camera))
                            }
                        }
                    }
                }

                var manualInput by remember { mutableStateOf("") }
                OutlinedTextField(
                    value = manualInput,
                    onValueChange = { manualInput = it },
                    label = { Text(stringResource(R.string.pair_paste)) },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
                if (manualInput.isNotBlank()) {
                    Button(
                        onClick = { vm.onQrScanned(manualInput.trim()) },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(stringResource(R.string.pair_connect_code))
                    }
                }
            }
            is PairViewModel.UiState.Working -> CircularProgressIndicator()
            is PairViewModel.UiState.Done -> {
                Text(stringResource(R.string.pair_success, s.hubName))
                Button(onClick = onDone) { Text(stringResource(R.string.action_done)) }
            }
            is PairViewModel.UiState.Failed -> {
                Text(stringResource(R.string.pair_failed, s.message), color = MaterialTheme.colorScheme.error)
                Button(onClick = vm::reset) { Text(stringResource(R.string.action_retry)) }
            }
        }
    }
}

/** CameraX preview + ML Kit barcode analysis feeding scanned QR strings up. */
@Composable
private fun QrScanner(onCode: (String) -> Unit) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val analyzer = remember { BarcodeScanning.getClient() }
    val executor = remember { Executors.newSingleThreadExecutor() }

    DisposableEffect(Unit) {
        onDispose {
            analyzer.close(); executor.shutdown()
            ProcessCameraProvider.getInstance(context).addListener({
                runCatching { ProcessCameraProvider.getInstance(context).get().unbindAll() }
            }, ContextCompat.getMainExecutor(context))
        }
    }

    AndroidView(
        modifier = Modifier.fillMaxWidth().height(320.dp),
        factory = { ctx ->
            val previewView = PreviewView(ctx)
            val future = ProcessCameraProvider.getInstance(ctx)
            future.addListener({
                try {
                    val provider = future.get()
                    val preview = Preview.Builder().build().also {
                        it.setSurfaceProvider(previewView.surfaceProvider)
                    }
                    val analysis = ImageAnalysis.Builder()
                        .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                        .build()
                        .also { img ->
                            img.setAnalyzer(executor) { proxy ->
                                @Suppress("UnsafeOptInUsageError")
                                val media = proxy.image
                                if (media != null) {
                                    analyzer.process(InputImage.fromMediaImage(media, proxy.imageInfo.rotationDegrees))
                                        .addOnSuccessListener { codes ->
                                            codes.firstOrNull { it.valueType == Barcode.TYPE_TEXT || it.valueType == Barcode.TYPE_URL }
                                                ?.rawValue?.let(onCode)
                                        }
                                        .addOnCompleteListener { proxy.close() }
                                } else proxy.close()
                            }
                        }
                    provider.unbindAll()
                    provider.bindToLifecycle(lifecycleOwner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                } catch (e: Exception) {
                    android.util.Log.e("QrScanner", "Camera initialization error", e)
                }
            }, ContextCompat.getMainExecutor(ctx))
            previewView
        },
    )
}
