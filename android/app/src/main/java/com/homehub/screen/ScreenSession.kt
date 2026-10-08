package com.homehub.screen

import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjection
import com.homehub.net.HubClient
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.json.JSONObject
import org.webrtc.*
import java.util.concurrent.atomic.AtomicBoolean
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Native WebRTC, no STUN/TURN, microphone, camera or cloud signaling. */
@Singleton
class ScreenSession @Inject constructor(@ApplicationContext private val context: Context, private val hub: HubClient) {
    val status = MutableStateFlow("idle")
    val error = MutableStateFlow<String?>(null)
    val remoteVideo = MutableStateFlow<VideoTrack?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var job: Job? = null
    private var peer: PeerConnection? = null
    private var factory: PeerConnectionFactory? = null
    private var source: VideoSource? = null
    private var localTrack: VideoTrack? = null
    private var capturer: ScreenCapturerAndroid? = null
    private var texture: SurfaceTextureHelper? = null
    private var egl: EglBase? = null
    private var sessionId: String? = null
    private val initialized = AtomicBoolean(false)
    fun eglContext(): EglBase.Context? = egl?.eglBaseContext
    fun startView() { start("view", null) }
    fun startCast(permission: Intent) { start("cast", permission) }
    private fun start(kind: String, permission: Intent?) {
        if (job?.isActive == true) return
        job = scope.launch {
            status.value = "connecting"; error.value = null
            try {
                if (initialized.compareAndSet(false, true)) PeerConnectionFactory.initialize(
                    PeerConnectionFactory.InitializationOptions.builder(context).createInitializationOptions())
                egl = EglBase.create()
                val eglContext = requireNotNull(egl).eglBaseContext
                factory = PeerConnectionFactory.builder()
                    .setVideoEncoderFactory(DefaultVideoEncoderFactory(eglContext, true, true))
                    .setVideoDecoderFactory(DefaultVideoDecoderFactory(eglContext)).createPeerConnectionFactory()
                val gathered = CompletableDeferred<Unit>()
                val configuration = PeerConnection.RTCConfiguration(emptyList()).apply {
                    sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
                    continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_ONCE
                }
                val observer = object : PeerConnection.Observer {
                    override fun onSignalingChange(state: PeerConnection.SignalingState) = Unit
                    override fun onIceConnectionChange(state: PeerConnection.IceConnectionState) {
                        if (state == PeerConnection.IceConnectionState.FAILED || state == PeerConnection.IceConnectionState.DISCONNECTED || state == PeerConnection.IceConnectionState.CLOSED) {
                            error.value = context.getString(com.homehub.R.string.screen_failed); stop()
                        }
                    }
                    override fun onIceConnectionReceivingChange(receiving: Boolean) = Unit
                    override fun onIceGatheringChange(state: PeerConnection.IceGatheringState) {
                        if (state == PeerConnection.IceGatheringState.COMPLETE) gathered.complete(Unit)
                    }
                    override fun onIceCandidate(candidate: IceCandidate) = Unit // Full SDP is sent after gathering.
                    override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>) = Unit
                    override fun onAddStream(stream: MediaStream) = Unit
                    override fun onRemoveStream(stream: MediaStream) = Unit
                    override fun onDataChannel(channel: DataChannel) { channel.close() }
                    override fun onRenegotiationNeeded() = Unit
                    override fun onAddTrack(receiver: RtpReceiver, streams: Array<out MediaStream>) {
                        remoteVideo.value = receiver.track() as? VideoTrack
                    }
                }
                peer = requireNotNull(factory).createPeerConnection(configuration, observer) ?: error("Screen connection unavailable")
                val pc = requireNotNull(peer)
                if (kind == "view") pc.addTransceiver(MediaStreamTrack.MediaType.MEDIA_TYPE_VIDEO,
                    RtpTransceiver.RtpTransceiverInit(RtpTransceiver.RtpTransceiverDirection.RECV_ONLY))
                else {
                    requireNotNull(permission) { "Screen permission is required" }
                    source = requireNotNull(factory).createVideoSource(true)
                    texture = SurfaceTextureHelper.create("HomeScreenCapture", eglContext)
                    capturer = ScreenCapturerAndroid(permission, object : MediaProjection.Callback() {
                        override fun onStop() { stop() }
                    }).also {
                        it.initialize(texture, context, requireNotNull(source).capturerObserver)
                        it.startCapture(1280, 720, 30)
                    }
                    localTrack = requireNotNull(factory).createVideoTrack("home-screen", source)
                    pc.addTransceiver(requireNotNull(localTrack),
                        RtpTransceiver.RtpTransceiverInit(RtpTransceiver.RtpTransceiverDirection.SEND_ONLY))
                }
                val offer = withTimeout(20_000) { createOffer(pc) }
                withTimeout(20_000) { setDescription(pc, offer, true); gathered.await() }
                // Only LAN host ICE candidates are present (no ICE servers).
                val fullOffer = LanSdp.hostOnly(requireNotNull(pc.localDescription).description)
                val answer = hub.post("/v1/screen/$kind", JSONObject().put("offer_sdp", fullOffer).put("preset", "balanced"))
                sessionId = answer.getString("session_id")
                withTimeout(20_000) { setDescription(pc, SessionDescription(SessionDescription.Type.ANSWER,
                    LanSdp.hostOnly(answer.getString("answer_sdp"))), false) }
                status.value = if (kind == "view") "viewing" else "casting"
                while (true) {
                    delay(15_000)
                    hub.patch("/v1/screen/${requireNotNull(sessionId)}", JSONObject())
                }
            } catch (e: CancellationException) { throw e }
            catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
            finally {
                val id = sessionId; sessionId = null
                remoteVideo.value = null
                withContext(NonCancellable) {
                    if (id != null) runCatching { hub.delete("/v1/screen/$id") }
                    runCatching { capturer?.stopCapture() }; capturer?.dispose(); capturer = null
                    peer?.close(); peer?.dispose(); peer = null
                    localTrack?.dispose(); localTrack = null; source?.dispose(); source = null
                    texture?.dispose(); texture = null; factory?.dispose(); factory = null
                    egl?.release(); egl = null
                    context.stopService(Intent(context, ScreenCastService::class.java))
                    status.value = "idle"
                }
            }
        }
    }
    fun stop() { job?.cancel() }
    private suspend fun createOffer(pc: PeerConnection): SessionDescription = suspendCancellableCoroutine { continuation ->
        pc.createOffer(object : SdpObserver {
            override fun onCreateSuccess(description: SessionDescription) { if (continuation.isActive) continuation.resume(description) }
            override fun onCreateFailure(message: String) { if (continuation.isActive) continuation.resumeWithException(IllegalStateException(message)) }
            override fun onSetSuccess() = Unit
            override fun onSetFailure(message: String) = Unit
        }, MediaConstraints())
    }
    private suspend fun setDescription(pc: PeerConnection, description: SessionDescription, local: Boolean): Unit = suspendCancellableCoroutine { continuation ->
        val observer = object : SdpObserver {
            override fun onSetSuccess() { if (continuation.isActive) continuation.resume(Unit) }
            override fun onSetFailure(message: String) { if (continuation.isActive) continuation.resumeWithException(IllegalStateException(message)) }
            override fun onCreateSuccess(description: SessionDescription) = Unit
            override fun onCreateFailure(message: String) = Unit
        }
        if (local) pc.setLocalDescription(observer, description) else pc.setRemoteDescription(observer, description)
    }
}
