package com.sway.app.webrtc

import android.content.Context
import android.util.Log
import com.sway.app.model.CandidatePayload
import com.sway.app.model.InputEvent
import com.sway.app.model.SdpPayload
import com.sway.app.network.SignalingClient
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.webrtc.*
import java.nio.ByteBuffer

sealed class RtcConnectionState {
    object Idle : RtcConnectionState()
    object Connecting : RtcConnectionState()
    object Connected : RtcConnectionState()
    data class Failed(val reason: String) : RtcConnectionState()
    object Disconnected : RtcConnectionState()
}

class WebRtcManager(
    private val context: Context,
    parentScope: CoroutineScope,
    val signalingClient: SignalingClient,
    val isDirectLocal: Boolean = false
) {
    private val tag = "WebRtcManager"
    private val scope = CoroutineScope(parentScope.coroutineContext + SupervisorJob())

    // Only allocated when WebRTC mode (!isDirectLocal) is actually active; saves 2-5MB native EGL state.
    private var eglBase: EglBase? = null
    private var peerConnectionFactory: PeerConnectionFactory? = null
    private var peerConnection: PeerConnection? = null
    private var inputDataChannel: DataChannel? = null

    private val _connectionState = MutableStateFlow<RtcConnectionState>(RtcConnectionState.Idle)
    val connectionState = _connectionState.asStateFlow()

    val currentFrame = signalingClient.currentFrame
    val fps = signalingClient.fps
    val rttMs = signalingClient.rttMs
    val monitorCount = signalingClient.monitorCount
    val textFocusEvents = signalingClient.textFocusEvents

    private var remoteVideoTrack: VideoTrack? = null
    private var videoRenderer: SurfaceViewRenderer? = null

    init {
        if (!isDirectLocal) {
            eglBase = EglBase.create()
            initPeerConnectionFactory()
            observeSignaling()
        } else {
            scope.launch {
                signalingClient.state.collect { st ->
                    _connectionState.value = when (st) {
                        is com.sway.app.network.SignalingState.Paired -> RtcConnectionState.Connected
                        is com.sway.app.network.SignalingState.Connected,
                        is com.sway.app.network.SignalingState.Connecting -> RtcConnectionState.Connecting
                        is com.sway.app.network.SignalingState.Error -> RtcConnectionState.Failed(st.message)
                        is com.sway.app.network.SignalingState.Disconnected -> RtcConnectionState.Disconnected
                    }
                }
            }
        }
    }

    private fun initPeerConnectionFactory() {
        val egl = eglBase ?: return
        val initOptions = PeerConnectionFactory.InitializationOptions.builder(context)
            .setEnableInternalTracer(false)
            .createInitializationOptions()
        PeerConnectionFactory.initialize(initOptions)

        val videoDecoderFactory = DefaultVideoDecoderFactory(egl.eglBaseContext)
        val videoEncoderFactory = DefaultVideoEncoderFactory(egl.eglBaseContext, true, true)

        peerConnectionFactory = PeerConnectionFactory.builder()
            .setVideoDecoderFactory(videoDecoderFactory)
            .setVideoEncoderFactory(videoEncoderFactory)
            .createPeerConnectionFactory()
    }

    fun attachRenderer(renderer: SurfaceViewRenderer) {
        val egl = eglBase ?: return
        this.videoRenderer?.let { detachRenderer(it) }
        this.videoRenderer = renderer
        renderer.init(egl.eglBaseContext, null)
        renderer.setEnableHardwareScaler(true)
        remoteVideoTrack?.addSink(renderer)
    }

    fun detachRenderer(renderer: SurfaceViewRenderer) {
        try {
            remoteVideoTrack?.removeSink(renderer)
            renderer.release()
        } catch (_: Exception) {}
        if (this.videoRenderer === renderer) {
            this.videoRenderer = null
        }
    }

    fun reconnect() {
        _connectionState.value = RtcConnectionState.Connecting
        signalingClient.reconnectNow()
    }

    fun startSession(iceServers: List<PeerConnection.IceServer>) {
        if (isDirectLocal) return
        _connectionState.value = RtcConnectionState.Connecting

        val rtcConfig = PeerConnection.RTCConfiguration(iceServers).apply {
            tcpCandidatePolicy = PeerConnection.TcpCandidatePolicy.ENABLED
            bundlePolicy = PeerConnection.BundlePolicy.MAXBUNDLE
            rtcpMuxPolicy = PeerConnection.RtcpMuxPolicy.REQUIRE
            continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
            keyType = PeerConnection.KeyType.ECDSA
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
        }

        peerConnection = peerConnectionFactory?.createPeerConnection(rtcConfig, peerConnectionObserver)

        val dcInit = DataChannel.Init().apply {
            ordered = false
            maxRetransmits = 0
            negotiated = false
            id = -1
        }
        inputDataChannel = peerConnection?.createDataChannel("sway-input", dcInit)
        inputDataChannel?.registerObserver(dataChannelObserver)
    }

    private fun observeSignaling() {
        scope.launch { signalingClient.offers.collect { handleRemoteOffer(it) } }
        scope.launch { signalingClient.answers.collect { handleRemoteAnswer(it) } }
        scope.launch {
            signalingClient.candidates.collect { cand ->
                val iceCandidate = IceCandidate(cand.sdpMid ?: "0", cand.sdpMLineIndex, cand.candidate)
                peerConnection?.addIceCandidate(iceCandidate)
            }
        }
    }

    private fun handleRemoteOffer(offer: SdpPayload) {
        val sdp = SessionDescription(SessionDescription.Type.OFFER, offer.sdp)
        peerConnection?.setRemoteDescription(object : SimpleSdpObserver() {
            override fun onSetSuccess() { createAnswer() }
        }, sdp)
    }

    private fun createAnswer() {
        val constraints = MediaConstraints().apply {
            mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveVideo", "true"))
            mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveAudio", "true"))
        }

        peerConnection?.createAnswer(object : SimpleSdpObserver() {
            override fun onCreateSuccess(desc: SessionDescription?) {
                desc?.let {
                    peerConnection?.setLocalDescription(SimpleSdpObserver(), it)
                    signalingClient.sendAnswer(it.description)
                }
            }
        }, constraints)
    }

    private fun handleRemoteAnswer(answer: SdpPayload) {
        val sdp = SessionDescription(SessionDescription.Type.ANSWER, answer.sdp)
        peerConnection?.setRemoteDescription(SimpleSdpObserver(), sdp)
    }

    fun sendInput(event: InputEvent) {
        val bytes = event.toByteArray()
        val dc = inputDataChannel
        if (!isDirectLocal && dc != null && dc.state() == DataChannel.State.OPEN) {
            val buffer = DataChannel.Buffer(ByteBuffer.wrap(bytes), true)
            dc.send(buffer)
        } else {
            signalingClient.sendBinary(bytes)
        }
    }

    private val peerConnectionObserver = object : PeerConnection.Observer {
        override fun onIceCandidate(candidate: IceCandidate?) {
            candidate?.let { signalingClient.sendCandidate(it.sdp, it.sdpMid, it.sdpMLineIndex) }
        }

        override fun onConnectionChange(newState: PeerConnection.PeerConnectionState?) {
            when (newState) {
                PeerConnection.PeerConnectionState.CONNECTED -> _connectionState.value = RtcConnectionState.Connected
                PeerConnection.PeerConnectionState.FAILED -> _connectionState.value = RtcConnectionState.Failed("WebRTC peer connection failed")
                PeerConnection.PeerConnectionState.DISCONNECTED,
                PeerConnection.PeerConnectionState.CLOSED -> _connectionState.value = RtcConnectionState.Disconnected
                else -> {}
            }
        }

        override fun onTrack(transceiver: RtpTransceiver?) {
            val track = transceiver?.receiver?.track()
            if (track is VideoTrack) {
                remoteVideoTrack = track
                videoRenderer?.let { track.addSink(it) }
            } else if (track is AudioTrack) {
                track.setEnabled(true)
            }
        }

        override fun onDataChannel(dc: DataChannel?) {
            dc?.let {
                inputDataChannel = it
                it.registerObserver(dataChannelObserver)
            }
        }

        override fun onSignalingChange(state: PeerConnection.SignalingState?) {}
        override fun onIceConnectionChange(state: PeerConnection.IceConnectionState?) {}
        override fun onIceConnectionReceivingChange(receiving: Boolean) {}
        override fun onIceGatheringChange(state: PeerConnection.IceGatheringState?) {}
        override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>?) {}
        override fun onRemoveStream(stream: MediaStream?) {}
        override fun onRenegotiationNeeded() {}
        override fun onAddStream(stream: MediaStream?) {}
    }

    private val dataChannelObserver = object : DataChannel.Observer {
        override fun onBufferedAmountChange(previousAmount: Long) {}
        override fun onStateChange() {}
        override fun onMessage(buffer: DataChannel.Buffer?) {}
    }

    open class SimpleSdpObserver : SdpObserver {
        override fun onCreateSuccess(p0: SessionDescription?) {}
        override fun onSetSuccess() {}
        override fun onCreateFailure(p0: String?) { Log.e("WebRtcManager", "SDP Create Failure: $p0") }
        override fun onSetFailure(p0: String?) { Log.e("WebRtcManager", "SDP Set Failure: $p0") }
    }

    fun release() {
        try {
            scope.cancel()
            if (!isDirectLocal) {
                videoRenderer?.let {
                    remoteVideoTrack?.removeSink(it)
                    it.release()
                }
                videoRenderer = null
                inputDataChannel?.unregisterObserver()
                inputDataChannel?.close()
                inputDataChannel?.dispose()
                inputDataChannel = null
                peerConnection?.close()
                peerConnection?.dispose()
                peerConnection = null
                peerConnectionFactory?.dispose()
                peerConnectionFactory = null
            }
            eglBase?.release()
            eglBase = null
        } catch (e: Exception) {
            Log.e(tag, "Error releasing WebRTC resources", e)
        }
    }
}
