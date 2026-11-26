package com.sway.app.network

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import com.sway.app.model.CandidatePayload
import com.sway.app.model.SdpPayload
import com.sway.app.model.SignalingEnvelope
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.*
import okio.ByteString
import okio.ByteString.Companion.toByteString
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

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
    parentScope: CoroutineScope
) : WebSocketListener() {

    private val tag = "SignalingClient"
    private val json = Json { ignoreUnknownKeys = true }

    companion object {
        // Shared across sessions so reconnects never leak OkHttp dispatcher threads or sockets.
        private val sharedHttpClient: OkHttpClient by lazy {
            OkHttpClient.Builder()
                .pingInterval(25, TimeUnit.SECONDS)
                .connectTimeout(10, TimeUnit.SECONDS)
                .build()
        }

        private val ipv4Regex = Regex("""^\d{1,3}(\.\d{1,3}){3}(:\d+)?$""")

        fun buildWsUrl(rawUrl: String): String {
            val trimmed = rawUrl.trim().trimEnd('/').removeSuffix("/ws")
            val scheme = when {
                trimmed.startsWith("ws://") || trimmed.startsWith("wss://") -> return "$trimmed/ws"
                trimmed.startsWith("http://") -> return "ws://${trimmed.removePrefix("http://")}/ws"
                trimmed.startsWith("https://") -> return "wss://${trimmed.removePrefix("https://")}/ws"
                trimmed.endsWith(".local") ||
                    trimmed.contains(".local:") ||
                    trimmed.endsWith(".lan") ||
                    trimmed.contains(".lan:") ||
                    trimmed.startsWith("[") ||
                    trimmed.endsWith(":8089") ||
                    ipv4Regex.matches(trimmed) ||
                    trimmed.startsWith("localhost") -> "ws://"
                else -> "wss://"
            }
            return "$scheme$trimmed/ws"
        }
    }

    private val clientScope = CoroutineScope(parentScope.coroutineContext + SupervisorJob())
    private val decodeDispatcher = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "SwayFrameDecoder").apply { priority = Thread.MAX_PRIORITY }
    }.asCoroutineDispatcher()
    private val inputDispatcher = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "SwayInputSender").apply { priority = Thread.MAX_PRIORITY }
    }.asCoroutineDispatcher()
    private val userDisconnected = AtomicBoolean(false)
    @Volatile
    private var webSocket: WebSocket? = null
    private var pingJob: Job? = null
    private var reconnectJob: Job? = null
    private var reconnectAttempt = 0

    private val ctrlOutChannel = Channel<ByteArray>(capacity = 64, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    private val inputWake = Channel<Unit>(Channel.CONFLATED)
    private val pendingMoveDx = AtomicInteger(0)
    private val pendingMoveDy = AtomicInteger(0)
    private val pendingScrollY = AtomicInteger(0)
    private val pendingScrollX = AtomicInteger(0)

    private val _state = MutableStateFlow<SignalingState>(SignalingState.Disconnected)
    val state = _state.asStateFlow()

    @Volatile
    var latestBitmap: Bitmap? = null
        private set

    @Volatile
    var frameListener: (() -> Unit)? = null

    private val _hasFrame = MutableStateFlow(false)
    val hasFrame = _hasFrame.asStateFlow()

    private val _fps = MutableStateFlow(0)
    val fps = _fps.asStateFlow()

    private val _rttMs = MutableStateFlow(0)
    val rttMs = _rttMs.asStateFlow()

    private val _monitorCount = MutableStateFlow(2)
    val monitorCount = _monitorCount.asStateFlow()

    private var frameCounter = 0
    private var lastFpsTimestamp = System.currentTimeMillis()
    private val lastPingTimestamp = AtomicLong(0L)

    private val _offers = MutableSharedFlow<SdpPayload>()
    val offers = _offers.asSharedFlow()

    private val _answers = MutableSharedFlow<SdpPayload>()
    val answers = _answers.asSharedFlow()

    val textFocusEvents = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    private val _candidates = MutableSharedFlow<CandidatePayload>()
    val candidates = _candidates.asSharedFlow()

    fun connect() {
        userDisconnected.set(false)
        reconnectJob?.cancel()
        openSocket()
    }

    fun reconnectNow() {
        if (userDisconnected.get()) return
        reconnectAttempt = 0
        reconnectJob?.cancel()
        pingJob?.cancel()
        webSocket?.cancel()
        webSocket = null
        openSocket()
    }

    private fun openSocket() {
        _state.value = SignalingState.Connecting
        val wsUrl = buildWsUrl(serverUrl)
        val request = Request.Builder().url(wsUrl).build()
        webSocket = sharedHttpClient.newWebSocket(request, this)
    }

    private fun scheduleReconnect(delayOverrideMs: Long? = null) {
        if (userDisconnected.get()) return
        if (reconnectJob?.isActive == true) return
        val delayMs = delayOverrideMs ?: when (reconnectAttempt) {
            0 -> 300L
            1 -> 800L
            2 -> 2000L
            else -> 4000L
        }
        reconnectAttempt = (reconnectAttempt + 1).coerceAtMost(5)
        reconnectJob = clientScope.launch {
            delay(delayMs)
            if (!userDisconnected.get()) {
                Log.i(tag, "Auto-reconnecting WebSocket (attempt $reconnectAttempt)...")
                openSocket()
            }
        }
    }

    override fun onOpen(webSocket: WebSocket, response: Response) {
        if (userDisconnected.get()) {
            webSocket.close(1000, "Disconnected")
            return
        }
        Log.i(tag, "WebSocket opened, sending join request for device: $deviceId")
        reconnectAttempt = 0
        _state.value = SignalingState.Connected

        val joinEnvelope = SignalingEnvelope(
            type = "join",
            deviceId = deviceId,
            token = token
        )
        sendEnvelope(joinEnvelope)

        pingJob?.cancel()
        pingJob = clientScope.launch {
            while (isActive) {
                val ts = System.currentTimeMillis()
                lastPingTimestamp.set(ts)
                webSocket.send("{\"type\":\"ping\",\"t\":$ts}")
                delay(1500)
            }
        }
    }

    private var frameSeq = 0L

    // Quad-buffered Bitmap pool: never reuse the bitmap currently held by _currentFrame/RenderThread,
    // and never call recycle() manually so hwui never encounters a recycled texture mid-draw.
    private val bitmapPool = arrayOfNulls<Bitmap>(4)
    private var poolIndex = 0

    val audioPlayer = com.sway.app.audio.AudioPlayer()

    private val decodeChannel = Channel<ByteArray>(Channel.CONFLATED)

    init {
        clientScope.launch(decodeDispatcher) {
            val decodeOpts = BitmapFactory.Options().apply {
                inPreferredConfig = Bitmap.Config.RGB_565
                inMutable = true
                inSampleSize = 1
                inPremultiplied = false // JPEGs have no alpha; RGB_565 halves memory & GPU upload cost
            }

            for (toDecode in decodeChannel) {
                if (userDisconnected.get()) break
                val activeBitmap = latestBitmap
                var candidate = bitmapPool[poolIndex]
                if (candidate != null && (candidate.isRecycled || !candidate.isMutable || candidate === activeBitmap)) {
                    poolIndex = (poolIndex + 1) % bitmapPool.size
                    candidate = bitmapPool[poolIndex]
                }
                decodeOpts.inBitmap = if (candidate != null && !candidate.isRecycled && candidate.isMutable && candidate !== activeBitmap) {
                    candidate
                } else {
                    null
                }

                val bmp = try {
                    BitmapFactory.decodeByteArray(toDecode, 0, toDecode.size, decodeOpts)
                } catch (_: IllegalArgumentException) {
                    decodeOpts.inBitmap = null
                    try {
                        BitmapFactory.decodeByteArray(toDecode, 0, toDecode.size, decodeOpts)
                    } catch (e: Exception) {
                        Log.e(tag, "Failed to decode JPEG frame fallback", e)
                        null
                    }
                } catch (e: Exception) {
                    Log.e(tag, "Failed to decode JPEG frame", e)
                    null
                }

                if (bmp != null && !userDisconnected.get()) {
                    try {
                        bmp.prepareToDraw()
                    } catch (_: Exception) {}
                    bitmapPool[poolIndex] = bmp
                    poolIndex = (poolIndex + 1) % bitmapPool.size
                    frameSeq++
                    latestBitmap = bmp
                    if (!_hasFrame.value) {
                        _hasFrame.value = true
                    }
                    frameListener?.invoke()

                    frameCounter++
                    val now = System.currentTimeMillis()
                    if (now - lastFpsTimestamp >= 1000) {
                        _fps.value = frameCounter
                        frameCounter = 0
                        lastFpsTimestamp = now
                    }
                }
            }
        }

        clientScope.launch(inputDispatcher) {
            for (_wake in inputWake) {
                if (userDisconnected.get()) break
                val ws = webSocket ?: continue

                // Flush accumulated pointer deltas before any queued click/key so clicks hit exact targets.
                val dx = pendingMoveDx.getAndSet(0)
                val dy = pendingMoveDy.getAndSet(0)
                if (dx != 0 || dy != 0) {
                    val cx = dx.coerceIn(-32767, 32767)
                    val cy = dy.coerceIn(-32767, 32767)
                    ws.send(
                        byteArrayOf(
                            0x01,
                            (cx and 0xFF).toByte(),
                            ((cx ushr 8) and 0xFF).toByte(),
                            (cy and 0xFF).toByte(),
                            ((cy ushr 8) and 0xFF).toByte()
                        ).toByteString()
                    )
                }

                while (true) {
                    val pkt = ctrlOutChannel.tryReceive().getOrNull() ?: break
                    ws.send(pkt.toByteString())
                }

                val sy = pendingScrollY.getAndSet(0)
                val sx = pendingScrollX.getAndSet(0)
                if (sy != 0 || sx != 0) {
                    val cy = sy.coerceIn(-32767, 32767)
                    val cx = sx.coerceIn(-32767, 32767)
                    ws.send(
                        byteArrayOf(
                            0x04,
                            (cy and 0xFF).toByte(),
                            ((cy ushr 8) and 0xFF).toByte(),
                            (cx and 0xFF).toByte(),
                            ((cx ushr 8) and 0xFF).toByte()
                        ).toByteString()
                    )
                }

                val dx2 = pendingMoveDx.getAndSet(0)
                val dy2 = pendingMoveDy.getAndSet(0)
                if (dx2 != 0 || dy2 != 0) {
                    val cx = dx2.coerceIn(-32767, 32767)
                    val cy = dy2.coerceIn(-32767, 32767)
                    ws.send(
                        byteArrayOf(
                            0x01,
                            (cx and 0xFF).toByte(),
                            ((cx ushr 8) and 0xFF).toByte(),
                            (cy and 0xFF).toByte(),
                            ((cy ushr 8) and 0xFF).toByte()
                        ).toByteString()
                    )
                }
            }
        }
    }

    override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
        val raw = bytes.toByteArray()

        // SWAU Audio Packet: route directly to async low-latency PCM player
        if (raw.size >= 4 &&
            raw[0] == 'S'.code.toByte() &&
            raw[1] == 'W'.code.toByte() &&
            raw[2] == 'A'.code.toByte() &&
            raw[3] == 'U'.code.toByte()
        ) {
            audioPlayer.playPcmChunk(raw)
            return
        }

        decodeChannel.trySend(raw)
    }

    override fun onMessage(webSocket: WebSocket, text: String) {
        try {
            val elem = json.parseToJsonElement(text)
            val obj = elem.jsonObject
            val msgType = obj["type"]?.jsonPrimitive?.content ?: return

            when (msgType) {
                "text_focus" -> {
                    clientScope.launch { textFocusEvents.emit(Unit) }
                }
                "host_info" -> {
                    val count = obj["monitors"]?.jsonPrimitive?.content?.toIntOrNull()
                    if (count != null && count > 0) {
                        _monitorCount.value = count
                    }
                }
                "pong" -> {
                    val echoedTs = obj["t"]?.jsonPrimitive?.content?.toLongOrNull()
                        ?: obj["payload"]?.jsonObject?.get("t")?.jsonPrimitive?.content?.toLongOrNull()
                        ?: lastPingTimestamp.get()
                    if (echoedTs > 0L) {
                        val rtt = (System.currentTimeMillis() - echoedTs).coerceIn(1L, 9999L)
                        _rttMs.value = rtt.toInt()
                    }
                }
                "status" -> {
                    val payloadElem = obj["payload"]
                    val statusStr = when {
                        payloadElem == null -> ""
                        payloadElem is kotlinx.serialization.json.JsonObject ->
                            payloadElem["status"]?.jsonPrimitive?.content ?: ""
                        else -> payloadElem.jsonPrimitive.content
                    }
                    when (statusStr) {
                        "paired" -> {
                            Log.i(tag, "Successfully paired with host $deviceId")
                            _state.value = SignalingState.Paired
                        }
                        "offline" -> {
                            _fps.value = 0
                            _state.value = SignalingState.Error("Host is offline")
                        }
                    }
                }
                "offer" -> {
                    val env = json.decodeFromString<SignalingEnvelope>(text)
                    env.payload?.let {
                        val sdp = json.decodeFromJsonElement<SdpPayload>(it)
                        clientScope.launch { _offers.emit(sdp) }
                    }
                }
                "answer" -> {
                    val env = json.decodeFromString<SignalingEnvelope>(text)
                    env.payload?.let {
                        val sdp = json.decodeFromJsonElement<SdpPayload>(it)
                        clientScope.launch { _answers.emit(sdp) }
                    }
                }
                "candidate" -> {
                    val env = json.decodeFromString<SignalingEnvelope>(text)
                    env.payload?.let {
                        val cand = json.decodeFromJsonElement<CandidatePayload>(it)
                        clientScope.launch { _candidates.emit(cand) }
                    }
                }
                "error" -> {
                    val errMsg = obj["error"]?.jsonPrimitive?.content ?: "Unknown signaling error"
                    Log.e(tag, "Server error: $errMsg")
                    _fps.value = 0
                    _state.value = SignalingState.Error(errMsg)
                    if (!errMsg.contains("authentication", ignoreCase = true) && !errMsg.contains("invalid token", ignoreCase = true)) {
                        scheduleReconnect(3000L)
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(tag, "Failed to parse message: $text", e)
        }
    }

    fun sendInputFast(event: com.sway.app.model.InputEvent): Boolean {
        if (userDisconnected.get() || webSocket == null) return false
        when (event) {
            is com.sway.app.model.InputEvent.MouseMove -> {
                if (event.dx.toInt() != 0) pendingMoveDx.addAndGet(event.dx.toInt())
                if (event.dy.toInt() != 0) pendingMoveDy.addAndGet(event.dy.toInt())
                inputWake.trySend(Unit)
            }
            is com.sway.app.model.InputEvent.MouseScroll -> {
                if (event.deltaY.toInt() != 0) pendingScrollY.addAndGet(event.deltaY.toInt())
                if (event.deltaX.toInt() != 0) pendingScrollX.addAndGet(event.deltaX.toInt())
                inputWake.trySend(Unit)
            }
            else -> {
                ctrlOutChannel.trySend(event.toByteArray())
                inputWake.trySend(Unit)
            }
        }
        return true
    }

    fun sendBinary(bytes: ByteArray): Boolean {
        val ws = webSocket ?: return false
        return ws.send(bytes.toByteString())
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
        if (this.webSocket !== webSocket || userDisconnected.get()) return
        Log.e(tag, "WebSocket failure: ${t.message}", t)
        pingJob?.cancel()
        pingJob = null
        _fps.value = 0
        _state.value = SignalingState.Error(t.message ?: "Connection failed")
        scheduleReconnect()
    }

    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
        if (this.webSocket !== webSocket) return
        Log.i(tag, "WebSocket closed: $reason ($code)")
        pingJob?.cancel()
        pingJob = null
        _fps.value = 0
        _state.value = SignalingState.Disconnected
        if (!userDisconnected.get()) {
            scheduleReconnect()
        }
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
        userDisconnected.set(true)
        reconnectJob?.cancel()
        reconnectJob = null
        pingJob?.cancel()
        pingJob = null
        audioPlayer.release()
        webSocket?.close(1000, "User disconnected")
        webSocket = null
        frameListener = null
        latestBitmap = null
        _hasFrame.value = false
        decodeChannel.close()
        ctrlOutChannel.close()
        inputWake.close()
        _fps.value = 0
        for (i in bitmapPool.indices) {
            // Do NOT call bitmap.recycle() here: RenderThread may still finish drawing the active frame.
            bitmapPool[i] = null
        }
        clientScope.cancel()
        try {
            decodeDispatcher.close()
        } catch (_: Exception) {}
        try {
            inputDispatcher.close()
        } catch (_: Exception) {}
        _state.value = SignalingState.Disconnected
    }
}
