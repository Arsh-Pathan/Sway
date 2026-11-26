package com.sway.app.ui.screens

import android.app.Activity
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.widget.Toast
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.roundToInt
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.viewinterop.AndroidView
import com.sway.app.model.InputEvent
import com.sway.app.network.StreamFrame
import com.sway.app.ui.components.ControlCenterSheet
import com.sway.app.ui.components.QuickPreset
import com.sway.app.ui.components.DeckSurface
import com.sway.app.ui.components.MouseButton
import com.sway.app.ui.components.ModifierDock
import com.sway.app.ui.components.QuickActionsSheet
import com.sway.app.ui.components.ResolutionSheet
import com.sway.app.ui.components.SettingsSheet
import com.sway.app.ui.components.TrackpadSurface
import com.sway.app.ui.theme.*
import com.sway.app.webrtc.RtcConnectionState
import com.sway.app.webrtc.WebRtcManager
import org.webrtc.RendererCommon
import org.webrtc.SurfaceViewRenderer

enum class ControlMode {
    TRACKPAD,
    DECK
}

enum class ViewFitMode {
    FIT,
    FILL,
    ROTATE_90
}

private data class StreamPlacement(val padX: Float, val padY: Float, val fitW: Float, val fitH: Float)

private fun computeStreamPlacement(
    canvasWidth: Float,
    canvasHeight: Float,
    imgW: Float,
    imgH: Float,
    fitMode: ViewFitMode
): StreamPlacement {
    if (canvasWidth <= 0f || canvasHeight <= 0f || imgW <= 0f || imgH <= 0f) {
        return StreamPlacement(0f, 0f, canvasWidth.coerceAtLeast(1f), canvasHeight.coerceAtLeast(1f))
    }
    val streamAspect = imgW / imgH
    val canvasAspect = canvasWidth / canvasHeight
    return when (fitMode) {
        ViewFitMode.FILL -> StreamPlacement(0f, 0f, canvasWidth, canvasHeight)
        ViewFitMode.ROTATE_90 -> {
            val rotFitW: Float
            val rotFitH: Float
            if (canvasHeight / canvasWidth.coerceAtLeast(1f) > streamAspect) {
                rotFitH = canvasWidth
                rotFitW = rotFitH * streamAspect
            } else {
                rotFitW = canvasHeight
                rotFitH = rotFitW / streamAspect
            }
            StreamPlacement((canvasWidth - rotFitW) / 2f, (canvasHeight - rotFitH) / 2f, rotFitW, rotFitH)
        }
        ViewFitMode.FIT -> {
            val fitW: Float
            val fitH: Float
            val padX: Float
            val padY: Float
            if (canvasAspect > streamAspect) {
                fitH = canvasHeight
                fitW = fitH * streamAspect
                padX = (canvasWidth - fitW) / 2f
                padY = 0f
            } else {
                fitW = canvasWidth
                fitH = fitW / streamAspect
                padX = 0f
                padY = (canvasHeight - fitH) / 2f
            }
            StreamPlacement(padX, padY, fitW, fitH)
        }
    }
}

private fun charToWindowsVKey(ch: Char): Short? {
    val upper = ch.uppercaseChar()
    return when (upper) {
        in 'A'..'Z' -> upper.code.toShort()
        in '0'..'9' -> upper.code.toShort()
        ' ' -> 0x20.toShort()
        '\n', '\r' -> 0x0D.toShort()
        '\t' -> 0x09.toShort()
        '-', '_' -> 0xBD.toShort()
        '=', '+' -> 0xBB.toShort()
        '[', '{' -> 0xDB.toShort()
        ']', '}' -> 0xDD.toShort()
        '\\', '|' -> 0xDC.toShort()
        ';', ':' -> 0xBA.toShort()
        '\'', '"' -> 0xDE.toShort()
        ',', '<' -> 0xBC.toShort()
        '.', '>' -> 0xBE.toShort()
        '/', '?' -> 0xBF.toShort()
        '`', '~' -> 0xC0.toShort()
        else -> null
    }
}

@Composable
private fun PerformanceHud(
    fpsFlow: kotlinx.coroutines.flow.StateFlow<Int>,
    rttFlow: kotlinx.coroutines.flow.StateFlow<Int>,
    isDeckMode: Boolean
) {
    val liveFps by fpsFlow.collectAsState()
    val liveRtt by rttFlow.collectAsState()

    Surface(
        color = SurfaceDark,
        shape = RoundedCornerShape(12.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(7.dp)
                    .clip(CircleShape)
                    .background(Color.White)
            )
            Text(
                text = if (isDeckMode) "DECK 1ms" else "$liveFps FPS · ${liveRtt}ms",
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                color = Color.White
            )
        }
    }
}

@Composable
private fun BoxScope.ConnectionOverlayCard(
    rtcState: RtcConnectionState,
    onReconnect: () -> Unit,
    onDisconnect: () -> Unit
) {
    Surface(
        modifier = Modifier
            .align(Alignment.Center)
            .padding(16.dp),
        color = SurfaceDark.copy(alpha = 0.95f),
        shape = RoundedCornerShape(12.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            val statusMsg = when (rtcState) {
                is RtcConnectionState.Failed -> {
                    val r = rtcState.reason
                    if (r.contains("192.168.") || r.contains("10.") || r.contains("172.")) {
                        "Local Wi-Fi IP unreachable on Mobile Data.\nPlease tap 'Exit' and select 'Mobile 4G/5G'."
                    } else {
                        "Connection failed:\n$r"
                    }
                }
                is RtcConnectionState.Disconnected -> "Connection dropped. Tap Retry to reconnect."
                is RtcConnectionState.Connected -> "Waiting for desktop video..."
                else -> "Connecting to desktop..."
            }
            Text(
                text = statusMsg,
                style = Typography.bodyMedium,
                color = Color.White,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center
            )
            if (rtcState is RtcConnectionState.Failed || rtcState is RtcConnectionState.Disconnected) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
                        onClick = onReconnect,
                        colors = ButtonDefaults.buttonColors(containerColor = Color.White),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text("Retry", color = Color.Black, fontWeight = FontWeight.Bold)
                    }
                    OutlinedButton(
                        onClick = onDisconnect,
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text("Exit", color = Color.White, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

private class StreamBitmapView(context: android.content.Context) : android.view.View(context) {
    var manager: WebRtcManager? = null
    var fitMode: ViewFitMode = ViewFitMode.FIT
    var onSizeTierChanged: ((Int, Int) -> Unit)? = null

    private val bitmapPaint = android.graphics.Paint().apply {
        isFilterBitmap = true
        isDither = false
    }
    private val dstRect = android.graphics.Rect()

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        onSizeTierChanged?.invoke(w, h)
    }

    override fun onDraw(canvas: android.graphics.Canvas) {
        val frame = manager?.latestBitmap ?: return
        if (frame.isRecycled) return
        val w = width.toFloat()
        val h = height.toFloat()
        if (w <= 0f || h <= 0f) return

        val placement = computeStreamPlacement(
            canvasWidth = w,
            canvasHeight = h,
            imgW = frame.width.toFloat(),
            imgH = frame.height.toFloat(),
            fitMode = fitMode
        )
        dstRect.set(
            placement.padX.toInt(),
            placement.padY.toInt(),
            (placement.padX + placement.fitW).toInt(),
            (placement.padY + placement.fitH).toInt()
        )
        if (fitMode == ViewFitMode.ROTATE_90) {
            canvas.save()
            canvas.rotate(90f, w / 2f, h / 2f)
            canvas.drawBitmap(frame, null, dstRect, bitmapPaint)
            canvas.restore()
        } else {
            canvas.drawBitmap(frame, null, dstRect, bitmapPaint)
        }
    }
}

@Composable
private fun DesktopStreamCanvas(
    webRtcManager: WebRtcManager,
    rtcState: RtcConnectionState,
    fitMode: ViewFitMode = ViewFitMode.FIT,
    zoomScale: Float = 1.0f,
    onInputEvent: (InputEvent) -> Unit = {},
    onReconnect: () -> Unit = {},
    onDisconnect: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    val hasFrame by webRtcManager.hasFrame.collectAsState()
    var viewWidthPx by remember { mutableIntStateOf(0) }
    var viewHeightPx by remember { mutableIntStateOf(0) }
    var lastViewportTier by remember { mutableIntStateOf(0) }

    LaunchedEffect(viewWidthPx, viewHeightPx, zoomScale) {
        if (viewWidthPx <= 0 || viewHeightPx <= 0) return@LaunchedEffect
        val rawDim = (maxOf(viewWidthPx, viewHeightPx) * zoomScale).roundToInt()
        val tier = when {
            rawDim <= 854 -> 854
            rawDim <= 960 -> 960
            rawDim <= 1280 -> 1280
            rawDim <= 1600 -> 1600
            else -> 1920
        }
        if (tier != lastViewportTier) {
            lastViewportTier = tier
            onInputEvent(InputEvent.SetViewport(tier))
        }
    }

    Box(modifier = modifier.fillMaxSize()) {
        AndroidView(
            factory = { ctx ->
                StreamBitmapView(ctx).apply {
                    manager = webRtcManager
                    this.fitMode = fitMode
                    onSizeTierChanged = { w, h ->
                        viewWidthPx = w
                        viewHeightPx = h
                    }
                    webRtcManager.setFrameListener {
                        postInvalidateOnAnimation()
                    }
                }
            },
            update = { view ->
                view.fitMode = fitMode
                view.invalidate()
            },
            onRelease = { view ->
                webRtcManager.setFrameListener(null)
                view.onSizeTierChanged = null
                view.manager = null
            },
            modifier = Modifier.fillMaxSize()
        )

        if (!hasFrame || rtcState !is RtcConnectionState.Connected) {
            ConnectionOverlayCard(
                rtcState = rtcState,
                onReconnect = onReconnect,
                onDisconnect = onDisconnect
            )
        }
    }
}

@Composable
private fun DesktopStreamView(
    webRtcManager: WebRtcManager,
    rtcState: RtcConnectionState,
    fitMode: ViewFitMode,
    zoomScale: Float,
    zoomOffsetX: Float,
    zoomOffsetY: Float,
    enableDirectTap: Boolean = true,
    onZoomChange: (Float, Float, Float) -> Unit,
    onInputEvent: (InputEvent) -> Unit = {},
    onReconnect: () -> Unit = {},
    onDisconnect: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    val latestScale by rememberUpdatedState(zoomScale)
    val latestOffsetX by rememberUpdatedState(zoomOffsetX)
    val latestOffsetY by rememberUpdatedState(zoomOffsetY)
    val latestEnableDirectTap by rememberUpdatedState(enableDirectTap)
    val latestOnZoomChange by rememberUpdatedState(onZoomChange)
    val latestOnInputEvent by rememberUpdatedState(onInputEvent)

    Box(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(fitMode) {
                val tapSlopPx = 10f * density.coerceAtLeast(1f)
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val downPos = down.position
                    val downTime = System.currentTimeMillis()
                    var currentScale = latestScale
                    var currentOffsetX = latestOffsetX
                    var currentOffsetY = latestOffsetY
                    var maxPointers = 1
                    var maxMovePx = 0f
                    var lastPos = downPos

                    while (true) {
                        val event = awaitPointerEvent()
                        var pressedCount = 0
                        var p0: androidx.compose.ui.input.pointer.PointerInputChange? = null
                        var p1: androidx.compose.ui.input.pointer.PointerInputChange? = null
                        for (i in event.changes.indices) {
                            val c = event.changes[i]
                            if (c.pressed) {
                                if (pressedCount == 0) p0 = c
                                else if (pressedCount == 1) p1 = c
                                pressedCount++
                            }
                        }
                        if (pressedCount > maxPointers) {
                            maxPointers = pressedCount
                        }

                        if (pressedCount == 0) {
                            if (latestEnableDirectTap &&
                                currentScale <= 1.05f &&
                                maxPointers == 1 &&
                                maxMovePx < tapSlopPx &&
                                (System.currentTimeMillis() - downTime) < 280
                            ) {
                                val bmp = webRtcManager.latestBitmap
                                val imgW = (bmp?.width ?: 1920).toFloat()
                                val imgH = (bmp?.height ?: 1080).toFloat()
                                val cw = size.width.toFloat().coerceAtLeast(1f)
                                val ch = size.height.toFloat().coerceAtLeast(1f)
                                val placement = computeStreamPlacement(cw, ch, imgW, imgH, fitMode)
                                val (ux, uy) = if (fitMode == ViewFitMode.ROTATE_90) {
                                    val cx = cw / 2f
                                    val cy = ch / 2f
                                    val rx = lastPos.x - cx
                                    val ry = lastPos.y - cy
                                    (cx + ry) to (cy - rx)
                                } else {
                                    lastPos.x to lastPos.y
                                }
                                val u = ((ux - placement.padX) / placement.fitW.coerceAtLeast(1f)).coerceIn(0f, 1f)
                                val v = ((uy - placement.padY) / placement.fitH.coerceAtLeast(1f)).coerceIn(0f, 1f)
                                val nx = (u * 65535f).roundToInt().coerceIn(0, 65535)
                                val ny = (v * 65535f).roundToInt().coerceIn(0, 65535)
                                latestOnInputEvent(InputEvent.TouchDirect(nx, ny, InputEvent.TouchDirect.ACTION_DOWN))
                                latestOnInputEvent(InputEvent.TouchDirect(nx, ny, InputEvent.TouchDirect.ACTION_UP))
                            }
                            break
                        }

                        if (pressedCount >= 2 && p0 != null && p1 != null) {
                            val currentDist = (p0.position - p1.position).getDistance().coerceAtLeast(1f)
                            val prevDist = (p0.previousPosition - p1.previousPosition).getDistance().coerceAtLeast(1f)
                            val zoomFactor = currentDist / prevDist

                            val currentCenter = (p0.position + p1.position) / 2f
                            val prevCenter = (p0.previousPosition + p1.previousPosition) / 2f
                            val pan = currentCenter - prevCenter

                            currentScale = (currentScale * zoomFactor).coerceIn(1.0f, 5.0f)
                            if (currentScale > 1.05f) {
                                val maxOffX = size.width * (currentScale - 1f) / 2f
                                val maxOffY = size.height * (currentScale - 1f) / 2f
                                currentOffsetX = (currentOffsetX + pan.x).coerceIn(-maxOffX, maxOffX)
                                currentOffsetY = (currentOffsetY + pan.y).coerceIn(-maxOffY, maxOffY)
                            } else {
                                currentOffsetX = 0f
                                currentOffsetY = 0f
                            }
                            latestOnZoomChange(currentScale, currentOffsetX, currentOffsetY)
                            for (i in event.changes.indices) {
                                if (event.changes[i].pressed) event.changes[i].consume()
                            }
                        } else if (pressedCount == 1 && p0 != null) {
                            lastPos = p0.position
                            val moveDist = (p0.position - downPos).getDistance()
                            if (moveDist > maxMovePx) maxMovePx = moveDist

                            if (currentScale > 1.05f) {
                                val pan = p0.position - p0.previousPosition
                                val maxOffX = size.width * (currentScale - 1f) / 2f
                                val maxOffY = size.height * (currentScale - 1f) / 2f
                                currentOffsetX = (currentOffsetX + pan.x).coerceIn(-maxOffX, maxOffX)
                                currentOffsetY = (currentOffsetY + pan.y).coerceIn(-maxOffY, maxOffY)
                                latestOnZoomChange(currentScale, currentOffsetX, currentOffsetY)
                                p0.consume()
                            }
                        }
                    }
                }
            }
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    scaleX = zoomScale
                    scaleY = zoomScale
                    translationX = zoomOffsetX
                    translationY = zoomOffsetY
                }
        ) {
            DesktopStreamCanvas(
                webRtcManager = webRtcManager,
                rtcState = rtcState,
                fitMode = fitMode,
                zoomScale = zoomScale,
                onInputEvent = onInputEvent,
                onReconnect = onReconnect,
                onDisconnect = onDisconnect
            )
        }
    }
}

@Composable
fun SessionScreen(
    webRtcManager: WebRtcManager,
    onDisconnect: () -> Unit
) {
    val configuration = LocalConfiguration.current
    val isLandscape = configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
    val context = LocalContext.current
    val activity = context as? Activity
    val keyboardController = LocalSoftwareKeyboardController.current

    val rtcState by webRtcManager.connectionState.collectAsState()

    var controlMode by remember { mutableStateOf(ControlMode.TRACKPAD) }
    var privacyCurtainActive by remember { mutableStateOf(false) }
    var is720pMode by remember { mutableStateOf(false) }
    var isVerticalPcMode by remember { mutableStateOf(false) }

    // Zoom & Pan state
    var zoomScale by remember { mutableFloatStateOf(1.0f) }
    var zoomOffsetX by remember { mutableFloatStateOf(0f) }
    var zoomOffsetY by remember { mutableFloatStateOf(0f) }

    var sensitivity by remember { mutableFloatStateOf(1.2f) }
    var scrollSpeed by remember { mutableFloatStateOf(1.0f) }
    var hapticsEnabled by remember { mutableStateOf(true) }
    var currentQuality by remember { mutableStateOf<Byte>(InputEvent.SetQuality.TURBO) }
    var showControlCenter by remember { mutableStateOf(false) }
    var isSpeakerAudioEnabled by remember { mutableStateOf(false) }
    var isMicAudioEnabled by remember { mutableStateOf(false) }
    var showSettings by remember { mutableStateOf(false) }
    var showQuickActions by remember { mutableStateOf(false) }
    var showResolutionSheet by remember { mutableStateOf(false) }
    var viewFitMode by remember { mutableStateOf(ViewFitMode.FIT) }
    var currentResMode by remember { mutableStateOf(InputEvent.ChangeResolution.DEFAULT) }
    var isTrackpadDragLocked by remember { mutableStateOf(false) }

    var ctrlActive by remember { mutableStateOf(false) }
    var altActive by remember { mutableStateOf(false) }
    var shiftActive by remember { mutableStateOf(false) }
    var winActive by remember { mutableStateOf(false) }

    val cleanedUp = remember { AtomicBoolean(false) }

    fun activeModifierMask(): Byte {
        var mask = 0
        if (ctrlActive) mask = mask or 0x01
        if (shiftActive) mask = mask or 0x02
        if (altActive) mask = mask or 0x04
        if (winActive) mask = mask or 0x08
        return mask.toByte()
    }

    fun clearModifiers() {
        if (ctrlActive) {
            ctrlActive = false
            webRtcManager.sendInput(InputEvent.KeyUp(0x11))
        }
        if (altActive) {
            altActive = false
            webRtcManager.sendInput(InputEvent.KeyUp(0x12))
        }
        if (shiftActive) {
            shiftActive = false
            webRtcManager.sendInput(InputEvent.KeyUp(0x10))
        }
        if (winActive) {
            winActive = false
            webRtcManager.sendInput(InputEvent.KeyUp(0x5B))
        }
    }

    LaunchedEffect(controlMode) {
        if (controlMode == ControlMode.DECK) {
            webRtcManager.sendInput(InputEvent.SetStreaming(false))
        } else {
            webRtcManager.sendInput(InputEvent.SetStreaming(true))
        }
    }

    DisposableEffect(controlMode, isLandscape) {
        onDispose {
            if (isTrackpadDragLocked) {
                isTrackpadDragLocked = false
                webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
            }
        }
    }

    val monitorCount by webRtcManager.monitorCount.collectAsState()

    // Soft Keyboard state synced with IME visibility
    val baseBuffer = "  "
    var keyboardInput by remember { mutableStateOf(TextFieldValue(baseBuffer, androidx.compose.ui.text.TextRange(baseBuffer.length))) }
    val focusRequester = remember { FocusRequester() }
    val density = LocalDensity.current
    val imeVisible = WindowInsets.ime.getBottom(density) > 0
    var isKeyboardOpen by remember { mutableStateOf(false) }

    LaunchedEffect(imeVisible) {
        isKeyboardOpen = imeVisible
    }

    // Auto-open software keyboard when text field receives focus or text focus signal arrives
    LaunchedEffect(Unit) {
        webRtcManager.textFocusEvents.collect {
            focusRequester.requestFocus()
            keyboardController?.show()
        }
    }

    var currentMonitor by remember { mutableIntStateOf(0) }

    fun switchMonitor() {
        val count = monitorCount.coerceAtLeast(1)
        currentMonitor = (currentMonitor + 1) % count
        webRtcManager.sendInput(InputEvent.SwitchMonitor(currentMonitor.toByte()))
        Toast.makeText(context, "Switched to Display #${currentMonitor + 1}", Toast.LENGTH_SHORT).show()
    }

    fun cyclePerformanceMode() {
        currentQuality = when (currentQuality) {
            InputEvent.SetQuality.TURBO -> InputEvent.SetQuality.BALANCED
            InputEvent.SetQuality.BALANCED -> InputEvent.SetQuality.NATIVE
            else -> InputEvent.SetQuality.TURBO
        }
        webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
    }

    fun toggleOrientation() {
        activity?.let {
            it.requestedOrientation = if (isLandscape) {
                ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            } else {
                ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            }
        }
    }

    fun togglePrivacyCurtain() {
        privacyCurtainActive = !privacyCurtainActive
        webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(privacyCurtainActive))
    }

    fun toggleResolution() {
        is720pMode = !is720pMode
        val mode = if (is720pMode) InputEvent.ChangeResolution.MOBILE_720P else InputEvent.ChangeResolution.DEFAULT
        webRtcManager.sendInput(InputEvent.ChangeResolution(mode))
    }

    var isPanelHidden by remember { mutableStateOf(false) }

    fun cycleZoom() {
        zoomScale = when {
            zoomScale < 1.4f -> 1.5f
            zoomScale < 1.9f -> 2.0f
            zoomScale < 2.9f -> 3.0f
            zoomScale < 4.9f -> 5.0f
            else -> 1.0f
        }
        if (zoomScale == 1.0f) {
            zoomOffsetX = 0f
            zoomOffsetY = 0f
        }
    }

    fun toggleVerticalPcMode() {
        isVerticalPcMode = !isVerticalPcMode
        webRtcManager.sendInput(InputEvent.ChangeOrientation(isVerticalPcMode))
        zoomScale = 1.0f
        zoomOffsetX = 0f
        zoomOffsetY = 0f
        if (isVerticalPcMode) {
            controlMode = ControlMode.TRACKPAD
            viewFitMode = ViewFitMode.FILL // Full screen fill, larger scale matching phone screen
            isPanelHidden = true           // Automatically fullscreen with zero black bars
            Toast.makeText(context, "Vertical PC: Fullscreen Active", Toast.LENGTH_SHORT).show()
        } else {
            controlMode = ControlMode.TRACKPAD
            viewFitMode = ViewFitMode.FIT
            isPanelHidden = false
            Toast.makeText(context, "Landscape PC: Trackpad Active", Toast.LENGTH_SHORT).show()
        }
    }

    fun applyPreset(preset: QuickPreset) {
        when (preset) {
            QuickPreset.PRODUCTIVITY -> {
                controlMode = ControlMode.TRACKPAD
                currentQuality = InputEvent.SetQuality.BALANCED
                webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                sensitivity = 1.0f
                scrollSpeed = 1.0f
                isSpeakerAudioEnabled = false
                isMicAudioEnabled = false
                webRtcManager.signalingClient.audioPlayer.stop()
                webRtcManager.sendInput(InputEvent.SetAudioConfig(false, false))
                viewFitMode = ViewFitMode.FIT
                Toast.makeText(context, "Preset: Productivity Active", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.CINEMA_MEDIA -> {
                controlMode = ControlMode.TRACKPAD
                currentQuality = InputEvent.SetQuality.NATIVE
                webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                viewFitMode = ViewFitMode.FILL
                isSpeakerAudioEnabled = true
                webRtcManager.sendInput(InputEvent.SetAudioConfig(true, isMicAudioEnabled))
                Toast.makeText(context, "Preset: Media Cinema Active (Audio ON)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.TURBO_GAMING -> {
                controlMode = ControlMode.TRACKPAD
                currentQuality = InputEvent.SetQuality.TURBO
                webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                sensitivity = 1.6f
                scrollSpeed = 1.4f
                Toast.makeText(context, "Preset: Turbo Gaming Active (60 FPS)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.PURE_DECK -> {
                controlMode = ControlMode.DECK
                Toast.makeText(context, "Preset: Pure Control Deck (1ms Latency)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.CODE_REVIEW -> {
                controlMode = ControlMode.TRACKPAD
                currentQuality = InputEvent.SetQuality.NATIVE
                webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                viewFitMode = ViewFitMode.FIT
                Toast.makeText(context, "Preset: Code & Reading (True Color HD)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.POWER_SAVER -> {
                controlMode = ControlMode.TRACKPAD
                currentQuality = InputEvent.SetQuality.DATA_SAVER
                webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                isSpeakerAudioEnabled = false
                isMicAudioEnabled = false
                webRtcManager.signalingClient.audioPlayer.stop()
                webRtcManager.sendInput(InputEvent.SetAudioConfig(false, false))
                Toast.makeText(context, "Preset: Battery Saver (Low Bandwidth)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.STEALTH_MEETING -> {
                controlMode = ControlMode.TRACKPAD
                privacyCurtainActive = true
                webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(true))
                isSpeakerAudioEnabled = false
                isMicAudioEnabled = false
                webRtcManager.signalingClient.audioPlayer.stop()
                webRtcManager.sendInput(InputEvent.SetAudioConfig(false, false))
                Toast.makeText(context, "Preset: Stealth Meeting (PC Screen Black)", Toast.LENGTH_SHORT).show()
            }
        }
        showControlCenter = false
    }

    fun cleanupAndRestore() {
        if (!cleanedUp.compareAndSet(false, true)) return
        if (isTrackpadDragLocked) {
            isTrackpadDragLocked = false
            webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
        }
        clearModifiers()
        webRtcManager.signalingClient.audioPlayer.stop()
        // Automatically restore physical screen, rotation & resolution upon disconnect
        webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(false))
        webRtcManager.sendInput(InputEvent.ChangeOrientation(false))
        webRtcManager.sendInput(InputEvent.ChangeResolution(InputEvent.ChangeResolution.DEFAULT))
    }

    fun handleDisconnect() {
        cleanupAndRestore()
        onDisconnect()
    }

    BackHandler {
        handleDisconnect()
    }

    DisposableEffect(Unit) {
        onDispose {
            cleanupAndRestore()
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BackgroundDark)
    ) {
        if (controlMode == ControlMode.DECK) {
            DeckSurface(
                sensitivity = sensitivity,
                scrollSpeed = scrollSpeed,
                hapticsEnabled = hapticsEnabled,
                onInputEvent = { webRtcManager.sendInput(it) },
                onOpenKeyboard = {
                    focusRequester.requestFocus()
                    keyboardController?.show()
                },
                onSwitchToStream = {
                    controlMode = ControlMode.TRACKPAD
                },
                onOpenActions = { showQuickActions = true },
                onDisconnect = { handleDisconnect() }
            )
        } else if (isLandscape) {
            // ==================== LANDSCAPE MODE: IMMERSIVE HORIZONTAL ====================
            Box(modifier = Modifier.fillMaxSize()) {
                if (webRtcManager.isDirectLocal) {
                    DesktopStreamView(
                        webRtcManager = webRtcManager,
                        rtcState = rtcState,
                        fitMode = viewFitMode,
                        zoomScale = zoomScale,
                        zoomOffsetX = zoomOffsetX,
                        zoomOffsetY = zoomOffsetY,
                        enableDirectTap = controlMode != ControlMode.TRACKPAD,
                        onZoomChange = { z, ox, oy ->
                            zoomScale = z
                            zoomOffsetX = ox
                            zoomOffsetY = oy
                        },
                        onInputEvent = { webRtcManager.sendInput(it) },
                        onReconnect = { webRtcManager.reconnect() },
                        onDisconnect = { handleDisconnect() }
                    )
                } else if (rtcState is RtcConnectionState.Connected) {
                    AndroidView(
                        factory = { ctx ->
                            SurfaceViewRenderer(ctx).apply {
                                setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                webRtcManager.attachRenderer(this)
                            }
                        },
                        onRelease = { webRtcManager.detachRenderer(it) },
                        modifier = Modifier.fillMaxSize()
                    )
                } else {
                    ConnectionOverlayCard(
                        rtcState = rtcState,
                        onReconnect = { webRtcManager.reconnect() },
                        onDisconnect = { handleDisconnect() }
                    )
                }

                // ==================== HORIZONTAL CLEAN MODE ====================
                // Full-Screen Edge-to-Edge Invisible Glide Trackpad (100% clean desktop view)
                if (controlMode == ControlMode.TRACKPAD && zoomScale <= 1.05f) {
                    TrackpadSurface(
                        modifier = Modifier.fillMaxSize(),
                        sensitivity = sensitivity,
                        scrollSpeed = scrollSpeed,
                        hapticsEnabled = hapticsEnabled,
                        isInvisible = true,
                        isDragLocked = isTrackpadDragLocked,
                        rotationQuarterTurns = if (viewFitMode == ViewFitMode.ROTATE_90) 1 else 0,
                        onUnlockDrag = { isTrackpadDragLocked = false },
                        onInputEvent = { webRtcManager.sendInput(it) }
                    )
                }

                // Ultra-Subtle Minimalist Floating Trigger (Hidden in corner, zero obstruction on sides or top)
                var showLandscapeMenu by remember { mutableStateOf(false) }

                Box(
                    modifier = Modifier
                        .align(Alignment.BottomEnd)
                        .systemBarsPadding()
                        .padding(16.dp)
                ) {
                    Surface(
                        onClick = { showLandscapeMenu = !showLandscapeMenu },
                        modifier = Modifier.size(36.dp),
                        shape = CircleShape,
                        color = Color.Black.copy(alpha = 0.35f),
                        border = androidx.compose.foundation.BorderStroke(1.dp, Color.White.copy(alpha = 0.25f))
                    ) {
                        Box(contentAlignment = Alignment.Center) {
                            Icon(
                                if (showLandscapeMenu) Icons.Default.Close else Icons.Default.MoreVert,
                                contentDescription = "Menu",
                                tint = Color.White.copy(alpha = 0.7f),
                                modifier = Modifier.size(18.dp)
                            )
                        }
                    }

                    // Floating Compact Quick Action Menu (Only when tapped)
                    if (showLandscapeMenu) {
                        Surface(
                            modifier = Modifier
                                .align(Alignment.BottomEnd)
                                .padding(bottom = 44.dp),
                            shape = RoundedCornerShape(16.dp),
                            color = SurfaceDark.copy(alpha = 0.95f),
                            border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                        ) {
                            Column(
                                modifier = Modifier.padding(8.dp),
                                verticalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                // Fit / Fill Toggle
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            viewFitMode = if (viewFitMode == ViewFitMode.FILL) ViewFitMode.FIT else ViewFitMode.FILL
                                            showLandscapeMenu = false
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.AspectRatio, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text(if (viewFitMode == ViewFitMode.FILL) "Letterbox (FIT)" else "Edge-to-Edge (FILL)", color = Color.White, fontSize = 13.sp)
                                }

                                // Zoom Cycle
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            cycleZoom()
                                            showLandscapeMenu = false
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.ZoomIn, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text("Zoom (${String.format(Locale.US, "%.1fx", zoomScale)})", color = Color.White, fontSize = 13.sp)
                                }

                                // Rotate back to Portrait
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            toggleOrientation()
                                            showLandscapeMenu = false
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.ScreenRotation, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text("Portrait Mode", color = Color.White, fontSize = 13.sp)
                                }

                                // Keyboard
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            focusRequester.requestFocus()
                                            keyboardController?.show()
                                            showLandscapeMenu = false
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.Keyboard, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text("Keyboard", color = Color.White, fontSize = 13.sp)
                                }

                                // Quick Tools
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            showQuickActions = true
                                            showLandscapeMenu = false
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.Bolt, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text("Shortcuts & Tools", color = Color.White, fontSize = 13.sp)
                                }

                                // Disconnect
                                Row(
                                    modifier = Modifier
                                        .clickable {
                                            showLandscapeMenu = false
                                            handleDisconnect()
                                        }
                                        .padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Icon(Icons.Default.PowerSettingsNew, contentDescription = null, tint = Color.White, modifier = Modifier.size(18.dp))
                                    Text("Disconnect", color = Color.White, fontSize = 13.sp)
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // ==================== PORTRAIT MODE: HYBRID CONTROLLER ====================
            if (isPanelHidden) {
                // Immersive full-screen video in portrait
                Box(modifier = Modifier.fillMaxSize()) {
                    if (webRtcManager.isDirectLocal) {
                        DesktopStreamView(
                            webRtcManager = webRtcManager,
                            rtcState = rtcState,
                            fitMode = viewFitMode,
                            zoomScale = zoomScale,
                            zoomOffsetX = zoomOffsetX,
                            zoomOffsetY = zoomOffsetY,
                            enableDirectTap = controlMode != ControlMode.TRACKPAD,
                            onZoomChange = { z, ox, oy ->
                                zoomScale = z
                                zoomOffsetX = ox
                                zoomOffsetY = oy
                            },
                            onInputEvent = { webRtcManager.sendInput(it) },
                            onReconnect = { webRtcManager.reconnect() },
                            onDisconnect = { handleDisconnect() }
                        )
                    } else if (rtcState is RtcConnectionState.Connected) {
                        AndroidView(
                            factory = { ctx ->
                                SurfaceViewRenderer(ctx).apply {
                                    setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                    webRtcManager.attachRenderer(this)
                                }
                            },
                            onRelease = { webRtcManager.detachRenderer(it) },
                            modifier = Modifier.fillMaxSize()
                        )
                    } else {
                        ConnectionOverlayCard(
                            rtcState = rtcState,
                            onReconnect = { webRtcManager.reconnect() },
                            onDisconnect = { handleDisconnect() }
                        )
                    }

                    if (controlMode == ControlMode.TRACKPAD && zoomScale <= 1.05f) {
                        TrackpadSurface(
                            modifier = Modifier.fillMaxSize(),
                            sensitivity = sensitivity,
                            scrollSpeed = scrollSpeed,
                            hapticsEnabled = hapticsEnabled,
                            isInvisible = true,
                            isDragLocked = isTrackpadDragLocked,
                            rotationQuarterTurns = if (viewFitMode == ViewFitMode.ROTATE_90) 1 else 0,
                            onUnlockDrag = { isTrackpadDragLocked = false },
                            onInputEvent = { webRtcManager.sendInput(it) }
                        )
                    }

                    // Floating pills for Zoom and Show Controls in portrait fullscreen
                    Row(
                        modifier = Modifier
                            .align(Alignment.TopEnd)
                            .systemBarsPadding()
                            .padding(16.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Surface(
                            onClick = { cycleZoom() },
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = RoundedCornerShape(20.dp),
                            border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                Icon(Icons.Default.ZoomIn, contentDescription = "Zoom", tint = Color.White, modifier = Modifier.size(16.dp))
                                Text(String.format(Locale.US, "%.1fx", zoomScale), color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                            }
                        }

                        Surface(
                            onClick = { isPanelHidden = false },
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = RoundedCornerShape(20.dp),
                            border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                Icon(Icons.Default.FullscreenExit, contentDescription = "Show Controls", tint = Color.White, modifier = Modifier.size(16.dp))
                                Text("Controls", color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                            }
                        }
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .systemBarsPadding()
                        .padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                // Minimalist Clean Top Header (Zero Clutter)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    PerformanceHud(
                        fpsFlow = webRtcManager.fps,
                        rttFlow = webRtcManager.rttMs,
                        isDeckMode = controlMode == ControlMode.DECK
                    )

                    // Mode Toggle Pill (Stream vs Deck)
                    Surface(
                        onClick = {
                            controlMode = if (controlMode == ControlMode.DECK) ControlMode.TRACKPAD else ControlMode.DECK
                        },
                        color = SurfaceDark,
                        shape = RoundedCornerShape(12.dp),
                        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp)
                        ) {
                            Icon(
                                if (controlMode == ControlMode.DECK) Icons.Default.Tv else Icons.Default.Gamepad,
                                contentDescription = null,
                                tint = Color.White,
                                modifier = Modifier.size(14.dp)
                            )
                            Text(
                                if (controlMode == ControlMode.DECK) "Stream" else "Deck Mode (1ms)",
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                color = Color.White
                            )
                        }
                    }

                    // Quick Action Buttons
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        // Rotate to Landscape Game Controller Mode
                        IconButton(
                            onClick = { toggleOrientation() },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.ScreenRotation, contentDescription = "Horizontal Mode", tint = Color.White)
                        }

                        // Native Keyboard
                        IconButton(
                            onClick = {
                                focusRequester.requestFocus()
                                keyboardController?.show()
                            },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.Keyboard, contentDescription = "Keyboard", tint = TextPrimary)
                        }

                        // Bottom Slider Trigger: Control Center
                        IconButton(
                            onClick = { showControlCenter = true },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.Tune, contentDescription = "Control Center", tint = Color.White)
                        }

                        // Fullscreen toggle
                        IconButton(
                            onClick = { isPanelHidden = true },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.Fullscreen, contentDescription = "Fullscreen", tint = TextPrimary)
                        }
                    }
                }

                // Sleek Minimal Slider Pull Handle to Open Control Center & Presets from Bottom
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 2.dp)
                        .clickable { showControlCenter = true },
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .width(54.dp)
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp))
                            .background(Color.White.copy(alpha = 0.45f))
                    )
                }

                // Privacy Status Indicator if active (Zero Card Background)
                if (privacyCurtainActive) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.Center
                    ) {
                        Box(
                            modifier = Modifier
                                .size(6.dp)
                                .clip(CircleShape)
                                .background(Color.White)
                        )
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(
                            text = "Laptop screen is stealth black",
                            color = TextSecondary,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Medium
                        )
                    }
                }

                // Top Half: Live Video Stream Surface with Pinch-to-zoom
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .clip(MaterialTheme.shapes.large)
                        .background(SurfaceDark)
                ) {
                    if (webRtcManager.isDirectLocal) {
                        DesktopStreamView(
                            webRtcManager = webRtcManager,
                            rtcState = rtcState,
                            fitMode = viewFitMode,
                            zoomScale = zoomScale,
                            zoomOffsetX = zoomOffsetX,
                            zoomOffsetY = zoomOffsetY,
                            enableDirectTap = true,
                            onZoomChange = { z, ox, oy ->
                                zoomScale = z
                                zoomOffsetX = ox
                                zoomOffsetY = oy
                            },
                            onInputEvent = { webRtcManager.sendInput(it) },
                            onReconnect = { webRtcManager.reconnect() },
                            onDisconnect = { handleDisconnect() }
                        )
                    } else if (rtcState is RtcConnectionState.Connected) {
                        AndroidView(
                            factory = { ctx ->
                                SurfaceViewRenderer(ctx).apply {
                                    setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                    webRtcManager.attachRenderer(this)
                                }
                            },
                            onRelease = { webRtcManager.detachRenderer(it) },
                            modifier = Modifier.fillMaxSize()
                        )
                    } else {
                        ConnectionOverlayCard(
                            rtcState = rtcState,
                            onReconnect = { webRtcManager.reconnect() },
                            onDisconnect = { handleDisconnect() }
                        )
                    }

                    // Zoom reset pill if zoomed in
                    if (zoomScale > 1.05f) {
                        Surface(
                            modifier = Modifier
                                .align(Alignment.BottomEnd)
                                .padding(10.dp),
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = RoundedCornerShape(12.dp)
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                Text(
                                    text = String.format(Locale.US, "%.1fx", zoomScale),
                                    color = AccentIndigoBright,
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                                IconButton(
                                    onClick = {
                                        zoomScale = 1.0f
                                        zoomOffsetX = 0f
                                        zoomOffsetY = 0f
                                    },
                                    modifier = Modifier.size(20.dp)
                                ) {
                                    Icon(Icons.Default.Close, contentDescription = "Reset Zoom", tint = TextPrimary, modifier = Modifier.size(14.dp))
                                }
                            }
                        }
                    }
                }

                // Middle: Dedicated Virtual Trackpad with Physical Click & Drag Bar
                if (controlMode == ControlMode.TRACKPAD) {
                    TrackpadSurface(
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1.3f),
                        sensitivity = sensitivity,
                        scrollSpeed = scrollSpeed,
                        hapticsEnabled = hapticsEnabled,
                        isDragLocked = isTrackpadDragLocked,
                        rotationQuarterTurns = if (viewFitMode == ViewFitMode.ROTATE_90) 1 else 0,
                        onUnlockDrag = { isTrackpadDragLocked = false },
                        onInputEvent = { webRtcManager.sendInput(it) }
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        MouseButton(
                            modifier = Modifier.weight(1.4f),
                            label = "LEFT CLICK",
                            isActive = isTrackpadDragLocked,
                            onDown = {
                                if (!isTrackpadDragLocked) {
                                    webRtcManager.sendInput(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                                }
                            },
                            onUp = {
                                if (!isTrackpadDragLocked) {
                                    webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                }
                            }
                        )

                        Surface(
                            onClick = {
                                isTrackpadDragLocked = !isTrackpadDragLocked
                                if (isTrackpadDragLocked) {
                                    webRtcManager.sendInput(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                                } else {
                                    webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                }
                            },
                            color = if (isTrackpadDragLocked) Color.White else SurfaceCard,
                            shape = RoundedCornerShape(10.dp),
                            border = androidx.compose.foundation.BorderStroke(1.dp, if (isTrackpadDragLocked) Color.White else SurfaceCardBorder),
                            modifier = Modifier.weight(1.1f)
                        ) {
                            Box(
                                modifier = Modifier.padding(vertical = 12.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    if (isTrackpadDragLocked) "DRAG [ON]" else "DRAG LOCK",
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Bold,
                                    color = if (isTrackpadDragLocked) Color.Black else TextSecondary
                                )
                            }
                        }

                        MouseButton(
                            modifier = Modifier.weight(0.8f),
                            label = "MID",
                            onDown = { webRtcManager.sendInput(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_MIDDLE)) },
                            onUp = { webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_MIDDLE)) }
                        )

                        MouseButton(
                            modifier = Modifier.weight(1.3f),
                            label = "RIGHT CLICK",
                            onDown = { webRtcManager.sendInput(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_RIGHT)) },
                            onUp = { webRtcManager.sendInput(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_RIGHT)) }
                        )
                    }
                }

                // Bottom: Sticky Modifier Dock
                ModifierDock(
                    ctrlActive = ctrlActive,
                    altActive = altActive,
                    shiftActive = shiftActive,
                    winActive = winActive,
                    onToggleCtrl = {
                        ctrlActive = !ctrlActive
                        webRtcManager.sendInput(if (ctrlActive) InputEvent.KeyDown(0x11, 0) else InputEvent.KeyUp(0x11))
                    },
                    onToggleAlt = {
                        altActive = !altActive
                        webRtcManager.sendInput(if (altActive) InputEvent.KeyDown(0x12, 0) else InputEvent.KeyUp(0x12))
                    },
                    onToggleShift = {
                        shiftActive = !shiftActive
                        webRtcManager.sendInput(if (shiftActive) InputEvent.KeyDown(0x10, 0) else InputEvent.KeyUp(0x10))
                    },
                    onToggleWin = {
                        winActive = !winActive
                        webRtcManager.sendInput(if (winActive) InputEvent.KeyDown(0x5B, 0) else InputEvent.KeyUp(0x5B))
                    },
                    onClearModifiers = { clearModifiers() },
                    onInputEvent = { webRtcManager.sendInput(it) },
                    onToggleKeyboard = {
                        if (imeVisible) {
                            keyboardController?.hide()
                        } else {
                            focusRequester.requestFocus()
                            keyboardController?.show()
                        }
                    },
                    onOpenSettings = { showSettings = true },
                    onSwitchMonitor = { switchMonitor() },
                    onOpenQuickActions = { showQuickActions = true }
                )
            }
        }
    }

        // Hidden input field for IME native keyboard capture with fixed cyclic buffer
        BasicTextField(
            value = keyboardInput,
            onValueChange = { newValue ->
                val newText = newValue.text
                var commonPrefix = 0
                val minLen = minOf(baseBuffer.length, newText.length)
                while (commonPrefix < minLen && baseBuffer[commonPrefix] == newText[commonPrefix]) {
                    commonPrefix++
                }
                val deleted = baseBuffer.length - commonPrefix
                val added = if (newText.length > commonPrefix) newText.substring(commonPrefix) else ""

                for (i in 0 until deleted) {
                    webRtcManager.sendInput(InputEvent.KeyDown(0x08, 0))
                    webRtcManager.sendInput(InputEvent.KeyUp(0x08))
                }
                if (added.isNotEmpty()) {
                    if (ctrlActive || altActive || winActive) {
                        val mask = activeModifierMask()
                        for (ch in added) {
                            val vkey = charToWindowsVKey(ch)
                            if (vkey != null) {
                                webRtcManager.sendInput(InputEvent.KeyDown(vkey, mask))
                                webRtcManager.sendInput(InputEvent.KeyUp(vkey))
                            } else {
                                webRtcManager.sendInput(InputEvent.TextInput(ch.toString()))
                            }
                        }
                        clearModifiers()
                    } else {
                        webRtcManager.sendInput(InputEvent.TextInput(added))
                    }
                }
                keyboardInput = TextFieldValue(baseBuffer, androidx.compose.ui.text.TextRange(baseBuffer.length))
            },
            keyboardOptions = KeyboardOptions(
                autoCorrectEnabled = false,
                capitalization = KeyboardCapitalization.None,
                imeAction = ImeAction.Send
            ),
            keyboardActions = KeyboardActions(onSend = {
                val mask = activeModifierMask()
                webRtcManager.sendInput(InputEvent.KeyDown(0x0D, mask))
                webRtcManager.sendInput(InputEvent.KeyUp(0x0D))
                if (mask.toInt() != 0) clearModifiers()
            }),
            modifier = Modifier
                .size(1.dp)
                .focusRequester(focusRequester)
        )

        // Bottom Slider: Control Center & Presets Sheet
        if (showControlCenter) {
            ControlCenterSheet(
                controlMode = controlMode,
                isVerticalPc = isVerticalPcMode,
                isPrivacyEnabled = privacyCurtainActive,
                viewFitMode = viewFitMode,
                currentQuality = currentQuality.toInt(),
                sensitivity = sensitivity,
                scrollSpeed = scrollSpeed,
                isSpeakerAudioEnabled = isSpeakerAudioEnabled,
                isMicAudioEnabled = isMicAudioEnabled,
                onApplyPreset = { applyPreset(it) },
                onToggleControlMode = { controlMode = it },
                onToggleVerticalPc = { toggleVerticalPcMode() },
                onTogglePrivacyCurtain = { enabled ->
                    privacyCurtainActive = enabled
                    webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(enabled))
                },
                onChangeFitMode = { viewFitMode = it },
                onChangeQuality = { q ->
                    currentQuality = q.toByte()
                    webRtcManager.sendInput(InputEvent.SetQuality(currentQuality))
                },
                onChangeSensitivity = { sensitivity = it },
                onChangeScrollSpeed = { scrollSpeed = it },
                onToggleSpeakerAudio = { enabled ->
                    isSpeakerAudioEnabled = enabled
                    if (!enabled) webRtcManager.signalingClient.audioPlayer.stop()
                    webRtcManager.sendInput(InputEvent.SetAudioConfig(enabled, isMicAudioEnabled))
                    Toast.makeText(context, if (enabled) "PC Audio Streaming ON" else "PC Audio Muted", Toast.LENGTH_SHORT).show()
                },
                onToggleMicAudio = { enabled ->
                    isMicAudioEnabled = enabled
                    webRtcManager.sendInput(InputEvent.SetAudioConfig(isSpeakerAudioEnabled, enabled))
                    Toast.makeText(context, if (enabled) "Phone Mic to PC ON (Aux Driver Req)" else "Mic Muted", Toast.LENGTH_SHORT).show()
                },
                onSendInput = { webRtcManager.sendInput(it) },
                onOpenResolution = {
                    showControlCenter = false
                    showResolutionSheet = true
                },
                onDismiss = { showControlCenter = false },
                onDisconnect = { handleDisconnect() }
            )
        }
        if (showSettings) {
            SettingsSheet(
                sensitivity = sensitivity,
                onSensitivityChange = { sensitivity = it },
                scrollSpeed = scrollSpeed,
                onScrollSpeedChange = { scrollSpeed = it },
                hapticsEnabled = hapticsEnabled,
                onHapticsToggle = { hapticsEnabled = it },
                currentQuality = currentQuality,
                onQualityChange = { quality ->
                    currentQuality = quality
                    webRtcManager.sendInput(InputEvent.SetQuality(quality))
                },
                onDismiss = { showSettings = false },
                onDisconnect = { handleDisconnect() }
            )
        }

        // Quick Actions & Power Sheet
        if (showQuickActions) {
            QuickActionsSheet(
                onSendInput = { webRtcManager.sendInput(it) },
                onDismiss = { showQuickActions = false }
            )
        }

        // Display Resolution & Scaling Sheet
        if (showResolutionSheet) {
            ResolutionSheet(
                currentResMode = currentResMode,
                currentFitMode = viewFitMode,
                isVerticalPc = isVerticalPcMode,
                onSelectRes = { mode ->
                    currentResMode = mode
                    webRtcManager.sendInput(InputEvent.ChangeResolution(mode))
                    showResolutionSheet = false
                },
                onSelectFit = { fit ->
                    viewFitMode = fit
                    showResolutionSheet = false
                },
                onSetVerticalPc = { vertical ->
                    if (vertical != isVerticalPcMode) {
                        toggleVerticalPcMode()
                    }
                },
                onDismiss = { showResolutionSheet = false }
            )
        }
    }
}
