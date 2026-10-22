package com.sway.app.network

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import com.sway.app.model.CandidatePayload
import com.sway.app.model.SdpPayload
import com.sway.app.model.SignalingEnvelope
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import okhttp3.*
import okio.ByteString
import okio.ByteString.Companion.toByteString
import java.util.concurrent.TimeUnit

data class StreamFrame(
    val bitmap: Bitmap,
    val sequence: Long
)

sealed class SignalingState {
    object Disconnected : SignalingState()
    object Connecting : SignalingState()
    object Connected : SignalingState()
    object Paired : SignalingState()
    data class Error(val message: String) : SignalingState()
}

class SignalingClient(
    private val serverUrl: String,
    private val deviceId: String,
    private val token: String,
    private val scope: CoroutineScope
) : WebSocketListener() {

    private val tag = "SignalingClient"
    private val json = Json { ignoreUnknownKeys = true }
    private val okHttpClient = OkHttpClient.Builder()
        .pingInterval(25, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null
    private var pingJob: Job? = null

    private val _state = MutableStateFlow<SignalingState>(SignalingState.Disconnected)
    val state = _state.asStateFlow()

    private val _currentFrame = MutableStateFlow<StreamFrame?>(null)
    val currentFrame = _currentFrame.asStateFlow()

    private val _fps = MutableStateFlow(0)
    val fps = _fps.asStateFlow()

    private val _rttMs = MutableStateFlow(1)
    val rttMs = _rttMs.asStateFlow()

    private var frameCounter = 0
    private var lastFpsTimestamp = System.currentTimeMillis()
    private var lastPingTimestamp = 0L

    private val _offers = MutableSharedFlow<SdpPayload>()
    val offers = _offers.asSharedFlow()

    private val _answers = MutableSharedFlow<SdpPayload>()
    val answers = _answers.asSharedFlow()

    val textFocusEvents = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    private val _candidates = MutableSharedFlow<CandidatePayload>()
    val candidates = _candidates.asSharedFlow()

    fun connect() {
        _state.value = SignalingState.Connecting
        val wsUrl = if (serverUrl.startsWith("http://")) {
            serverUrl.replace("http://", "ws://") + "/ws"
        } else if (serverUrl.startsWith("https://")) {
            serverUrl.replace("https://", "wss://") + "/ws"
        } else if (!serverUrl.startsWith("ws://") && !serverUrl.startsWith("wss://")) {
            "ws://$serverUrl/ws"
        } else {
            "$serverUrl/ws"
        }

        val request = Request.Builder().url(wsUrl).build()
        webSocket = okHttpClient.newWebSocket(request, this)
    }

    override fun onOpen(webSocket: WebSocket, response: Response) {
        Log.i(tag, "WebSocket opened, sending join request for device: $deviceId")
        _state.value = SignalingState.Connected

        // Send Join message
        val joinEnvelope = SignalingEnvelope(
            type = "join",
            deviceId = deviceId,
            token = token
        )
        sendEnvelope(joinEnvelope)

        // Start real-time RTT latency ping loop every 1.5 seconds
        pingJob?.cancel()
        pingJob = scope.launch {
            while (isActive) {
                delay(1500)
                lastPingTimestamp = System.currentTimeMillis()
                webSocket.send("{\"type\":\"ping\",\"t\":$lastPingTimestamp}")
            }
        }
    }

    private val isDecoding = java.util.concurrent.atomic.AtomicBoolean(false)
    @Volatile
    private var pendingFrameBytes: ByteArray? = null
    private var frameSeq = 0L

    private val ackByteString = okio.ByteString.of(0xAA.toByte())

    override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
        // Send instantaneous 1-byte frame ack to host to pace capture loop and eliminate bufferbloat
        webSocket.send(ackByteString)

        pendingFrameBytes = bytes.toByteArray()

        // Update real-time FPS counter
        frameCounter++
        val now = System.currentTimeMillis()
        if (now - lastFpsTimestamp >= 1000) {
            _fps.value = frameCounter
            frameCounter = 0
            lastFpsTimestamp = now
        }

        if (isDecoding.compareAndSet(false, true)) {
            scope.launch(Dispatchers.Default) {
                try {
                    while (true) {
                        val toDecode = pendingFrameBytes ?: break
                        pendingFrameBytes = null

                        val decodeOpts = BitmapFactory.Options().apply {
                            inPreferredConfig = Bitmap.Config.RGB_565
                        }
                        val bmp = try {
                            BitmapFactory.decodeByteArray(toDecode, 0, toDecode.size, decodeOpts)
                        } catch (e: Exception) {
                            Log.e(tag, "Failed to decode JPEG frame", e)
                            null
                        }

                        if (bmp != null) {
                            frameSeq++
                            _currentFrame.value = StreamFrame(bmp, frameSeq)
                        }
                    }
                } catch (e: Exception) {
                    Log.e(tag, "Failed in decode loop", e)
                } finally {
                    isDecoding.set(false)
                }
            }
        }
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        try {
            if (text.contains("\"text_focus\"")) {
                scope.launch { textFocusEvents.emit(Unit) }
                return
            }

            // Handle real-time RTT pong response
            if (text.contains("\"pong\"")) {
                if (lastPingTimestamp > 0) {
                    val rtt = (System.currentTimeMillis() - lastPingTimestamp).coerceAtLeast(1)
                    _rttMs.value = rtt.toInt()
                }
                return
            }

            val env = json.decodeFromString<SignalingEnvelope>(text)
            when (env.type) {
                "status" -> {
                    val statusText = env.payload?.toString() ?: ""
                    if (statusText.contains("paired")) {
                        Log.i(tag, "Successfully paired with host $deviceId")
                        _state.value = SignalingState.Paired
                    } else if (statusText.contains("offline")) {
                        _state.value = SignalingState.Error("Host is offline")
                    }
                }
                "offer" -> {
                    env.payload?.let {
                        val sdp = json.decodeFromJsonElement<SdpPayload>(it)
                        scope.launch { _offers.emit(sdp) }
                    }
                }
                "answer" -> {
                    env.payload?.let {
                        val sdp = json.decodeFromJsonElement<SdpPayload>(it)
                        scope.launch { _answers.emit(sdp) }
                    }
                }
                "candidate" -> {
                    env.payload?.let {
                        val cand = json.decodeFromJsonElement<CandidatePayload>(it)
                        scope.launch { _candidates.emit(cand) }
                    }
                }
                "error" -> {
                    val errMsg = env.error ?: "Unknown signaling error"
                    Log.e(tag, "Server error: $errMsg")
                    _state.value = SignalingState.Error(errMsg)
                }
            }
        } catch (e: Exception) {
            Log.e(tag, "Failed to parse message: $text", e)
        }
    }

    fun sendBinary(bytes: ByteArray): Boolean {
        val ws = webSocket ?: return false
        return ws.send(bytes.toByteString())
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
        Log.e(tag, "WebSocket failure: ${t.message}", t)
        pingJob?.cancel()
        pingJob = null
        _state.value = SignalingState.Error(t.message ?: "Connection failed")
    }

    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
        Log.i(tag, "WebSocket closed: $reason ($code)")
        pingJob?.cancel()
        pingJob = null
        _state.value = SignalingState.Disconnected
    }

    fun sendOffer(sdp: String) {
        val payload = json.encodeToJsonElement(SdpPayload(type = "offer", sdp = sdp))
        sendEnvelope(SignalingEnvelope(type = "offer", role = "client", deviceId = deviceId, payload = payload))
    }

    fun sendAnswer(sdp: String) {
        val payload = json.encodeToJsonElement(SdpPayload(type = "answer", sdp = sdp))
        sendEnvelope(SignalingEnvelope(type = "answer", role = "client", deviceId = deviceId, payload = payload))
    }

    fun sendCandidate(candidate: String, sdpMid: String?, sdpMLineIndex: Int) {
        val payload = json.encodeToJsonElement(
            CandidatePayload(candidate = candidate, sdpMid = sdpMid, sdpMLineIndex = sdpMLineIndex)
        )
        sendEnvelope(SignalingEnvelope(type = "candidate", role = "client", deviceId = deviceId, payload = payload))
    }

    private fun sendEnvelope(envelope: SignalingEnvelope) {
        val raw = json.encodeToString(envelope)
        webSocket?.send(raw)
    }

    fun disconnect() {
        pingJob?.cancel()
        pingJob = null
        webSocket?.close(1000, "User disconnected")
        webSocket = null
        _currentFrame.value = null
        _state.value = SignalingState.Disconnected
    }
}
