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
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.widget.Toast
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

@Composable
private fun DesktopStreamCanvas(
    currentFrameFlow: kotlinx.coroutines.flow.StateFlow<com.sway.app.network.StreamFrame?>,
    rtcState: RtcConnectionState,
    fitMode: ViewFitMode = ViewFitMode.FIT,
    modifier: Modifier = Modifier
) {
    val frameState = currentFrameFlow.collectAsState()
    val paint = remember {
        android.graphics.Paint().apply {
            isFilterBitmap = true
            isDither = false
        }
    }
    val dstRect = remember { android.graphics.Rect() }

    Box(modifier = modifier.fillMaxSize()) {
        androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
            val frameWrapper = frameState.value ?: return@Canvas
            val frame = frameWrapper.bitmap
            val canvasWidth = size.width
            val canvasHeight = size.height

            val imgW = frame.width.toFloat()
            val imgH = frame.height.toFloat()
            val streamAspect = imgW / imgH
            val canvasAspect = canvasWidth / canvasHeight

            val fitW: Float
            val fitH: Float
            val padX: Float
            val padY: Float

            if (fitMode == ViewFitMode.FILL) {
                fitW = canvasWidth
                fitH = canvasHeight
                padX = 0f
                padY = 0f
            } else if (fitMode == ViewFitMode.ROTATE_90) {
                val rotFitW: Float
                val rotFitH: Float
                if (canvasHeight / canvasWidth.coerceAtLeast(1f) > streamAspect) {
                    rotFitH = canvasWidth
                    rotFitW = rotFitH * streamAspect
                } else {
                    rotFitW = canvasHeight
                    rotFitH = rotFitW / streamAspect
                }
                fitW = rotFitW
                fitH = rotFitH
                padX = (canvasWidth - fitW) / 2f
                padY = (canvasHeight - fitH) / 2f
            } else {
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
            }

            dstRect.set(padX.toInt(), padY.toInt(), (padX + fitW).toInt(), (padY + fitH).toInt())

            drawIntoCanvas { canvas ->
                val nativeCanvas = canvas.nativeCanvas
                if (fitMode == ViewFitMode.ROTATE_90) {
                    nativeCanvas.save()
                    nativeCanvas.rotate(90f, canvasWidth / 2f, canvasHeight / 2f)
                    nativeCanvas.drawBitmap(frame, null, dstRect, paint)
                    nativeCanvas.restore()
                } else {
                    nativeCanvas.drawBitmap(frame, null, dstRect, paint)
                }
            }
        }

        if (frameState.value == null) {
            Surface(
                modifier = Modifier
                    .align(Alignment.Center)
                    .padding(16.dp),
                color = SurfaceDark.copy(alpha = 0.95f),
                shape = RoundedCornerShape(12.dp)
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    val statusMsg = if (rtcState is RtcConnectionState.Failed) {
                        val r = (rtcState as RtcConnectionState.Failed).reason
                        if (r.contains("192.168.") || r.contains("10.") || r.contains("172.")) {
                            "Local Wi-Fi IP unreachable on Mobile Data.\nPlease tap 'Back' and select 'Mobile 4G/5G'."
                        } else {
                            "Connection failed:\n$r"
                        }
                    } else {
                        "Connecting to desktop..."
                    }
                    Text(
                        text = statusMsg,
                        style = Typography.bodyMedium,
                        color = Color.White
                    )
                }
            }
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
    isVerticalPc: Boolean,
    onZoomChange: (Float, Float, Float) -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(zoomScale, zoomOffsetX, zoomOffsetY, isVerticalPc, fitMode) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val downPos = down.position
                    var prevPointers = listOf(down)
                    var currentScale = zoomScale
                    var currentOffsetX = zoomOffsetX
                    var currentOffsetY = zoomOffsetY

                    while (true) {
                        val event = awaitPointerEvent()
                        val pressedPointers = event.changes.filter { it.pressed }

                        if (pressedPointers.isEmpty()) {
                            break
                        }

                        if (pressedPointers.size >= 2) {
                            // Multi-touch pinch-to-zoom and two-finger pan
                            val p0 = pressedPointers[0]
                            val p1 = pressedPointers[1]
                            val prev0 = prevPointers.firstOrNull { it.id == p0.id } ?: p0
                            val prev1 = prevPointers.firstOrNull { it.id == p1.id } ?: p1

                            val currentDist = (p0.position - p1.position).getDistance().coerceAtLeast(1f)
                            val prevDist = (prev0.position - prev1.position).getDistance().coerceAtLeast(1f)
                            val zoomFactor = currentDist / prevDist

                            val currentCenter = (p0.position + p1.position) / 2f
                            val prevCenter = (prev0.position + prev1.position) / 2f
                            val pan = currentCenter - prevCenter

                            currentScale = (currentScale * zoomFactor).coerceIn(1.0f, 5.0f)
                            if (currentScale > 1.05f) {
                                currentOffsetX += pan.x
                                currentOffsetY += pan.y
                            } else {
                                currentOffsetX = 0f
                                currentOffsetY = 0f
                            }
                            onZoomChange(currentScale, currentOffsetX, currentOffsetY)
                            for (p in pressedPointers) p.consume()
                        } else if (pressedPointers.size == 1) {
                            val current = pressedPointers[0]

                            if (zoomScale > 1.05f) {
                                // Viewport panning when zoomed in
                                val prev = prevPointers.firstOrNull { it.id == current.id } ?: current
                                val pan = current.position - prev.position
                                currentOffsetX += pan.x
                                currentOffsetY += pan.y
                                onZoomChange(currentScale, currentOffsetX, currentOffsetY)
                                current.consume()
                            }
                        }
                        prevPointers = pressedPointers
                    }
                }
            }
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer(
                    scaleX = zoomScale,
                    scaleY = zoomScale,
                    translationX = zoomOffsetX,
                    translationY = zoomOffsetY
                )
        ) {
            DesktopStreamCanvas(
                currentFrameFlow = webRtcManager.currentFrame,
                rtcState = rtcState,
                fitMode = fitMode
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

    LaunchedEffect(controlMode) {
        if (controlMode == ControlMode.DECK) {
            webRtcManager.sendInput(InputEvent.SetStreaming(false))
        } else {
            webRtcManager.sendInput(InputEvent.SetStreaming(true))
        }
    }

    val liveFps by webRtcManager.fps.collectAsState()
    val liveRtt by webRtcManager.rttMs.collectAsState()

    // Soft Keyboard state
    val baseBuffer = "  "
    var keyboardInput by remember { mutableStateOf(TextFieldValue(baseBuffer, androidx.compose.ui.text.TextRange(baseBuffer.length))) }
    val focusRequester = remember { FocusRequester() }
    var isKeyboardOpen by remember { mutableStateOf(false) }

    // Auto-open software keyboard when text field receives focus or text focus signal arrives
    LaunchedEffect(Unit) {
        webRtcManager.textFocusEvents.collect {
            focusRequester.requestFocus()
            keyboardController?.show()
        }
    }

    var currentMonitor by remember { mutableIntStateOf(0) }

    fun switchMonitor() {
        currentMonitor = (currentMonitor + 1) % 2
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
                webRtcManager.sendInput(InputEvent.SetAudioConfig(false, false))
                Toast.makeText(context, "Preset: Battery Saver (Low Bandwidth)", Toast.LENGTH_SHORT).show()
            }
            QuickPreset.STEALTH_MEETING -> {
                controlMode = ControlMode.TRACKPAD
                privacyCurtainActive = true
                webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(true))
                isSpeakerAudioEnabled = false
                isMicAudioEnabled = false
                webRtcManager.sendInput(InputEvent.SetAudioConfig(false, false))
                Toast.makeText(context, "Preset: Stealth Meeting (PC Screen Black)", Toast.LENGTH_SHORT).show()
            }
        }
        showControlCenter = false
    }

    fun handleDisconnect() {
        // Automatically restore physical screen, rotation & resolution upon disconnect
        webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(false))
        webRtcManager.sendInput(InputEvent.ChangeOrientation(false))
        webRtcManager.sendInput(InputEvent.ChangeResolution(InputEvent.ChangeResolution.DEFAULT))
        onDisconnect()
    }

    BackHandler {
        handleDisconnect()
    }

    DisposableEffect(Unit) {
        onDispose {
            webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(false))
            webRtcManager.sendInput(InputEvent.ChangeOrientation(false))
            webRtcManager.sendInput(InputEvent.ChangeResolution(InputEvent.ChangeResolution.DEFAULT))
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
                        isVerticalPc = isVerticalPcMode,
                        onZoomChange = { z, ox, oy ->
                            zoomScale = z
                            zoomOffsetX = ox
                            zoomOffsetY = oy
                        }
                    )
                } else if (rtcState is RtcConnectionState.Connected) {
                    AndroidView(
                        factory = { ctx ->
                            SurfaceViewRenderer(ctx).apply {
                                setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                webRtcManager.attachRenderer(this)
                            }
                        },
                        modifier = Modifier.fillMaxSize()
                    )
                } else {
                    Surface(
                        modifier = Modifier
                            .align(Alignment.Center)
                            .padding(16.dp),
                        color = SurfaceDark.copy(alpha = 0.95f),
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            val statusMsg = if (rtcState is RtcConnectionState.Failed) {
                                val r = (rtcState as RtcConnectionState.Failed).reason
                                if (r.contains("192.168.") || r.contains("10.") || r.contains("172.")) {
                                    "Local Wi-Fi IP unreachable on Mobile Data.\nPlease tap 'Back' and select 'Mobile 4G/5G'."
                                } else {
                                    "Connection failed:\n$r"
                                }
                            } else {
                                "Connecting to desktop..."
                            }
                            Text(
                                text = statusMsg,
                                style = Typography.bodyMedium,
                                color = Color.White,
                                textAlign = androidx.compose.ui.text.style.TextAlign.Center
                            )
                            if (rtcState is RtcConnectionState.Failed) {
                                Button(
                                    onClick = onDisconnect,
                                    colors = ButtonDefaults.buttonColors(containerColor = Color.White),
                                    shape = RoundedCornerShape(8.dp)
                                ) {
                                    Text("Back to Pairing", color = Color.Black, fontWeight = FontWeight.Bold)
                                }
                            }
                        }
                    }
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
                        onInputEvent = { webRtcManager.sendInput(it) }
                    )
                }

                // Ultra-Subtle Minimalist Floating Trigger (Hidden in corner, zero obstruction on sides or top)
                var showLandscapeMenu by remember { mutableStateOf(false) }

                Box(
                    modifier = Modifier
                        .align(Alignment.BottomEnd)
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
                            isVerticalPc = isVerticalPcMode,
                            onZoomChange = { z, ox, oy ->
                                zoomScale = z
                                zoomOffsetX = ox
                                zoomOffsetY = oy
                            }
                        )
                    } else if (rtcState is RtcConnectionState.Connected) {
                        AndroidView(
                            factory = { ctx ->
                                SurfaceViewRenderer(ctx).apply {
                                    setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                    webRtcManager.attachRenderer(this)
                                }
                            },
                            modifier = Modifier.fillMaxSize()
                        )
                    } else {
                        Surface(
                            modifier = Modifier
                                .align(Alignment.Center)
                                .padding(8.dp),
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = MaterialTheme.shapes.medium
                        ) {
                            val statusMsg = if (rtcState is RtcConnectionState.Failed) {
                                "Connection failed: ${(rtcState as RtcConnectionState.Failed).reason}"
                            } else {
                                "Connecting to desktop..."
                            }
                            Text(
                                text = statusMsg,
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                style = Typography.bodyMedium,
                                color = Color.White
                            )
                        }
                    }

                    // Floating pill to unhide controls in portrait
                    Surface(
                        onClick = { isPanelHidden = false },
                        modifier = Modifier
                            .align(Alignment.TopEnd)
                            .padding(16.dp),
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
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                // Minimalist Clean Top Header (Zero Clutter)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Live Performance HUD
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
                                text = if (controlMode == ControlMode.DECK) "DECK 1ms" else "${liveFps} FPS · ${liveRtt}ms",
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                color = Color.White
                            )
                        }
                    }

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
                            isVerticalPc = isVerticalPcMode,
                            onZoomChange = { z, ox, oy ->
                                zoomScale = z
                                zoomOffsetX = ox
                                zoomOffsetY = oy
                            }
                        )
                    } else if (rtcState is RtcConnectionState.Connected) {
                        AndroidView(
                            factory = { ctx ->
                                SurfaceViewRenderer(ctx).apply {
                                    setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                    webRtcManager.attachRenderer(this)
                                }
                            },
                            modifier = Modifier.fillMaxSize()
                        )
                    } else {
                        Surface(
                            modifier = Modifier
                                .align(Alignment.Center)
                                .padding(8.dp),
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = MaterialTheme.shapes.medium
                        ) {
                            val statusMsg = if (rtcState is RtcConnectionState.Failed) {
                                "Connection failed: ${(rtcState as RtcConnectionState.Failed).reason}"
                            } else {
                                "Connecting to desktop..."
                            }
                            Text(
                                text = statusMsg,
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                style = Typography.bodyMedium,
                                color = Color.White
                            )
                        }
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
                                    text = String.format("%.1fx", zoomScale),
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
                        onInputEvent = { webRtcManager.sendInput(it) }
                    )

                    var isTrackpadDragLocked by remember { mutableStateOf(false) }
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
                    onInputEvent = { webRtcManager.sendInput(it) },
                    onToggleKeyboard = {
                        isKeyboardOpen = !isKeyboardOpen
                        focusRequester.requestFocus()
                        if (isKeyboardOpen) {
                            keyboardController?.show()
                        } else {
                            keyboardController?.hide()
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
                if (newText.length > baseBuffer.length) {
                    val added = newText.substring(baseBuffer.length)
                    webRtcManager.sendInput(InputEvent.TextInput(added))
                } else if (newText.length < baseBuffer.length) {
                    val diff = baseBuffer.length - newText.length
                    for (i in 0 until diff) {
                        webRtcManager.sendInput(InputEvent.KeyDown(0x08, 0))
                        webRtcManager.sendInput(InputEvent.KeyUp(0x08))
                    }
                }
                keyboardInput = TextFieldValue(baseBuffer, androidx.compose.ui.text.TextRange(baseBuffer.length))
            },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(onSend = {
                webRtcManager.sendInput(InputEvent.KeyDown(0x0D, 0))
                webRtcManager.sendInput(InputEvent.KeyUp(0x0D))
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
                    webRtcManager.sendInput(InputEvent.SetAudioConfig(enabled, isMicAudioEnabled))
                    Toast.makeText(context, if (enabled) "PC Audio Streaming ON" else "PC Audio Muted", Toast.LENGTH_SHORT).show()
                },
                onToggleMicAudio = { enabled ->
                    isMicAudioEnabled = enabled
                    webRtcManager.sendInput(InputEvent.SetAudioConfig(isSpeakerAudioEnabled, enabled))
                    Toast.makeText(context, if (enabled) "Phone Mic to PC ON (Aux Driver Req)" else "Mic Muted", Toast.LENGTH_SHORT).show()
                },
                onSendInput = { webRtcManager.sendInput(it) },
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
                onToggleVerticalPc = {
                    toggleVerticalPcMode()
                },
                onDismiss = { showResolutionSheet = false }
            )
        }
    }
}
