package com.sway.app.webrtc

import android.content.Context
import android.util.Log
import com.sway.app.model.CandidatePayload
import com.sway.app.model.InputEvent
import com.sway.app.model.SdpPayload
import com.sway.app.network.SignalingClient
import kotlinx.coroutines.CoroutineScope
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
    private val scope: CoroutineScope,
    private val signalingClient: SignalingClient,
    val eglBase: EglBase = EglBase.create()
) {
    private val tag = "WebRtcManager"

    private var peerConnectionFactory: PeerConnectionFactory? = null
    private var peerConnection: PeerConnection? = null
    private var inputDataChannel: DataChannel? = null

    private val _connectionState = MutableStateFlow<RtcConnectionState>(RtcConnectionState.Idle)
    val connectionState = _connectionState.asStateFlow()

    val currentFrame: kotlinx.coroutines.flow.StateFlow<com.sway.app.network.StreamFrame?> = signalingClient.currentFrame
    val fps = signalingClient.fps
    val rttMs = signalingClient.rttMs

    private var remoteVideoTrack: VideoTrack? = null
    private var videoRenderer: SurfaceViewRenderer? = null

    init {
        initPeerConnectionFactory()
        observeSignaling()
    }

    private fun initPeerConnectionFactory() {
        val initOptions = PeerConnectionFactory.InitializationOptions.builder(context)
            .setEnableInternalTracer(false)
            .createInitializationOptions()
        PeerConnectionFactory.initialize(initOptions)

        val videoDecoderFactory = DefaultVideoDecoderFactory(eglBase.eglBaseContext)
        val videoEncoderFactory = DefaultVideoEncoderFactory(eglBase.eglBaseContext, true, true)

        peerConnectionFactory = PeerConnectionFactory.builder()
            .setVideoDecoderFactory(videoDecoderFactory)
            .setVideoEncoderFactory(videoEncoderFactory)
            .createPeerConnectionFactory()
    }

    fun attachRenderer(renderer: SurfaceViewRenderer) {
        this.videoRenderer = renderer
        renderer.init(eglBase.eglBaseContext, null)
        renderer.setEnableHardwareScaler(true)
        remoteVideoTrack?.addSink(renderer)
    }

    fun startSession(iceServers: List<PeerConnection.IceServer>) {
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

        // Initialize low-latency DataChannel for input events
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
        scope.launch {
            signalingClient.offers.collect { offer ->
                handleRemoteOffer(offer)
            }
        }
        scope.launch {
            signalingClient.answers.collect { answer ->
                handleRemoteAnswer(answer)
            }
        }
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
            override fun onSetSuccess() {
                createAnswer()
            }
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

    /**
     * Sends an input event directly through the low-latency DataChannel,
     * or relays via WebSocket if DataChannel is negotiating.
     */
    fun sendInput(event: InputEvent) {
        val bytes = event.toByteArray()
        val dc = inputDataChannel
        if (dc != null && dc.state() == DataChannel.State.OPEN) {
            val buffer = DataChannel.Buffer(ByteBuffer.wrap(bytes), true)
            dc.send(buffer)
        } else {
            signalingClient.sendBinary(bytes)
        }
    }

    private val peerConnectionObserver = object : PeerConnection.Observer {
        override fun onIceCandidate(candidate: IceCandidate?) {
            candidate?.let {
                signalingClient.sendCandidate(it.sdp, it.sdpMid, it.sdpMLineIndex)
            }
        }

        override fun onConnectionChange(newState: PeerConnection.PeerConnectionState?) {
            Log.i(tag, "PeerConnectionState changed: $newState")
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
                Log.i(tag, "Remote video track received")
                remoteVideoTrack = track
                videoRenderer?.let { track.addSink(it) }
            } else if (track is AudioTrack) {
                Log.i(tag, "Remote audio track received (WASAPI system audio)")
                track.setEnabled(true)
            }
        }

        override fun onDataChannel(dc: DataChannel?) {
            Log.i(tag, "Incoming DataChannel: ${dc?.label()}")
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
        override fun onStateChange() {
            Log.i(tag, "DataChannel '${inputDataChannel?.label()}' state: ${inputDataChannel?.state()}")
        }
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
            remoteVideoTrack?.removeSink(videoRenderer)
            inputDataChannel?.close()
            peerConnection?.close()
            videoRenderer?.release()
            peerConnectionFactory?.dispose()
            eglBase.release()
        } catch (e: Exception) {
            Log.e(tag, "Error releasing WebRTC resources", e)
        }
    }
}
