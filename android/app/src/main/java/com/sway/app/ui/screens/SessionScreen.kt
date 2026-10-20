package com.sway.app.ui.screens

import android.app.Activity
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTransformGestures
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
import androidx.compose.ui.viewinterop.AndroidView
import com.sway.app.model.InputEvent
import com.sway.app.ui.components.ModifierDock
import com.sway.app.ui.components.SettingsSheet
import com.sway.app.ui.components.TrackpadSurface
import com.sway.app.ui.theme.*
import com.sway.app.webrtc.RtcConnectionState
import com.sway.app.webrtc.WebRtcManager
import org.webrtc.RendererCommon
import org.webrtc.SurfaceViewRenderer

enum class ControlMode {
    TRACKPAD,
    TOUCH_SCREEN
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
    val currentFrame by webRtcManager.currentFrame.collectAsState()

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
    var currentQuality by remember { mutableStateOf(InputEvent.SetQuality.BALANCED) }
    var showSettings by remember { mutableStateOf(false) }

    // Soft Keyboard state
    var keyboardInput by remember { mutableStateOf(TextFieldValue("")) }
    val focusRequester = remember { FocusRequester() }
    var isKeyboardOpen by remember { mutableStateOf(false) }

    var currentMonitor by remember { mutableIntStateOf(0) }

    fun switchMonitor() {
        currentMonitor = (currentMonitor + 1) % 2
        webRtcManager.sendInput(InputEvent.SwitchMonitor(currentMonitor.toByte()))
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

    fun toggleVerticalPcMode() {
        isVerticalPcMode = !isVerticalPcMode
        webRtcManager.sendInput(InputEvent.ChangeOrientation(isVerticalPcMode))
        if (isVerticalPcMode) {
            zoomScale = 1.0f
            zoomOffsetX = 0f
            zoomOffsetY = 0f
        }
    }

    fun handleDisconnect() {
        // Automatically restore physical screen, rotation & resolution upon disconnect
        webRtcManager.sendInput(InputEvent.SetPrivacyCurtain(false))
        webRtcManager.sendInput(InputEvent.ChangeOrientation(false))
        webRtcManager.sendInput(InputEvent.ChangeResolution(InputEvent.ChangeResolution.DEFAULT))
        onDisconnect()
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
        if (isLandscape) {
            // ==================== LANDSCAPE MODE: IMMERSIVE HORIZONTAL ====================
            Box(modifier = Modifier.fillMaxSize()) {
                // Video Surface with Zoom & Pan + Direct Touch
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .pointerInput(Unit) {
                            detectTransformGestures { _, pan, zoom, _ ->
                                zoomScale = (zoomScale * zoom).coerceIn(1.0f, 4.0f)
                                if (zoomScale > 1.0f) {
                                    zoomOffsetX += pan.x
                                    zoomOffsetY += pan.y
                                } else {
                                    zoomOffsetX = 0f
                                    zoomOffsetY = 0f
                                }
                            }
                        }
                        .pointerInput(controlMode, zoomScale) {
                            if (controlMode == ControlMode.TOUCH_SCREEN && zoomScale <= 1.05f) {
                                awaitEachGesture {
                                    val down = awaitFirstDown()
                                    val normX = (down.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                    val normY = (down.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                    webRtcManager.sendInput(InputEvent.TouchDirect(normX, normY, InputEvent.TouchDirect.ACTION_DOWN))

                                    val pointerId = down.id
                                    while (true) {
                                        val event = awaitPointerEvent()
                                        val current = event.changes.firstOrNull { it.id == pointerId } ?: break
                                        if (!current.pressed) {
                                            val upNormX = (current.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                            val upNormY = (current.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                            webRtcManager.sendInput(InputEvent.TouchDirect(upNormX, upNormY, InputEvent.TouchDirect.ACTION_UP))
                                            break
                                        }
                                        val curNormX = (current.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                        val curNormY = (current.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                        webRtcManager.sendInput(InputEvent.TouchDirect(curNormX, curNormY, InputEvent.TouchDirect.ACTION_MOVE))
                                        current.consume()
                                    }
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
                        if (rtcState is RtcConnectionState.Connected) {
                            AndroidView(
                                factory = { ctx ->
                                    SurfaceViewRenderer(ctx).apply {
                                        setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                        webRtcManager.attachRenderer(this)
                                    }
                                },
                                modifier = Modifier.fillMaxSize()
                            )
                        } else if (currentFrame != null) {
                            Image(
                                bitmap = currentFrame!!.asImageBitmap(),
                                contentDescription = "Desktop Stream",
                                modifier = Modifier.fillMaxSize(),
                                contentScale = ContentScale.Fit
                            )
                        } else {
                            Surface(
                                modifier = Modifier
                                    .align(Alignment.Center)
                                    .padding(8.dp),
                                color = SurfaceDark.copy(alpha = 0.85f),
                                shape = MaterialTheme.shapes.medium
                            ) {
                                Text(
                                    text = "Connecting to desktop...",
                                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                    style = Typography.bodyMedium,
                                    color = AccentIndigoBright
                                )
                            }
                        }
                    }
                }

                // If in Trackpad mode, overlay transparent trackpad surface
                if (controlMode == ControlMode.TRACKPAD && zoomScale <= 1.05f) {
                    TrackpadSurface(
                        modifier = Modifier.fillMaxSize(),
                        sensitivity = sensitivity,
                        scrollSpeed = scrollSpeed,
                        hapticsEnabled = hapticsEnabled,
                        onInputEvent = { webRtcManager.sendInput(it) }
                    )
                }

                // Floating Action Header
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = { handleDisconnect() },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark.copy(alpha = 0.85f))
                        ) {
                            Icon(Icons.Default.Close, contentDescription = "Close", tint = TextPrimary)
                        }

                        // Mode chip
                        FilterChip(
                            selected = controlMode == ControlMode.TOUCH_SCREEN,
                            onClick = {
                                controlMode = if (controlMode == ControlMode.TRACKPAD) ControlMode.TOUCH_SCREEN else ControlMode.TRACKPAD
                            },
                            label = {
                                Text(
                                    if (controlMode == ControlMode.TOUCH_SCREEN) "Touch Slate" else "Trackpad",
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            },
                            leadingIcon = {
                                Icon(
                                    if (controlMode == ControlMode.TOUCH_SCREEN) Icons.Default.TouchApp else Icons.Default.Mouse,
                                    contentDescription = null,
                                    modifier = Modifier.size(16.dp)
                                )
                            },
                            colors = FilterChipDefaults.filterChipColors(
                                selectedContainerColor = AccentIndigo,
                                selectedLabelColor = TextPrimary,
                                containerColor = SurfaceDark.copy(alpha = 0.85f),
                                labelColor = TextSecondary
                            )
                        )

                        // Privacy shield badge
                        if (privacyCurtainActive) {
                            Surface(
                                color = Color(0xFF10B981).copy(alpha = 0.2f),
                                shape = RoundedCornerShape(16.dp),
                                border = androidx.compose.foundation.BorderStroke(1.dp, Color(0xFF10B981))
                            ) {
                                Row(
                                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                                ) {
                                    Icon(Icons.Default.Security, contentDescription = null, tint = Color(0xFF10B981), modifier = Modifier.size(14.dp))
                                    Text("Stealth Active", color = Color(0xFF10B981), fontSize = 11.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }
                    }

                    // Right action buttons
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        // Quick Zoom button
                        IconButton(
                            onClick = {
                                zoomScale = if (zoomScale >= 2.0f) 1.0f else (zoomScale + 0.5f)
                                if (zoomScale == 1.0f) {
                                    zoomOffsetX = 0f
                                    zoomOffsetY = 0f
                                }
                            },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (zoomScale > 1.0f) AccentIndigo else SurfaceDark.copy(alpha = 0.85f)
                            )
                        ) {
                            Icon(Icons.Default.ZoomIn, contentDescription = "Zoom", tint = if (zoomScale > 1.0f) TextPrimary else TextMuted)
                        }

                        // Privacy curtain toggle
                        IconButton(
                            onClick = { togglePrivacyCurtain() },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (privacyCurtainActive) Color(0xFF10B981) else SurfaceDark.copy(alpha = 0.85f)
                            )
                        ) {
                            Icon(
                                Icons.Default.Security,
                                contentDescription = "Privacy Shield",
                                tint = if (privacyCurtainActive) Color.White else TextMuted
                            )
                        }

                        // Resolution preset toggle (720p / 1080p)
                        IconButton(
                            onClick = { toggleResolution() },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (is720pMode) AccentIndigo else SurfaceDark.copy(alpha = 0.85f)
                            )
                        ) {
                            Icon(
                                Icons.Default.AspectRatio,
                                contentDescription = "Resolution",
                                tint = if (is720pMode) TextPrimary else TextMuted
                            )
                        }

                        // Native Keyboard button
                        IconButton(
                            onClick = {
                                focusRequester.requestFocus()
                                keyboardController?.show()
                            },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark.copy(alpha = 0.85f))
                        ) {
                            Icon(Icons.Default.Keyboard, contentDescription = "Keyboard", tint = TextPrimary)
                        }

                        // Rotate button
                        IconButton(
                            onClick = { toggleOrientation() },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark.copy(alpha = 0.85f))
                        ) {
                            Icon(Icons.Default.ScreenRotation, contentDescription = "Rotate", tint = TextPrimary)
                        }

                        // Settings button
                        IconButton(
                            onClick = { showSettings = true },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark.copy(alpha = 0.85f))
                        ) {
                            Icon(Icons.Default.Settings, contentDescription = "Settings", tint = TextPrimary)
                        }
                    }
                }
            }
        } else {
            // ==================== PORTRAIT MODE: HYBRID CONTROLLER ====================
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                // Top Control Bar
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    IconButton(
                        onClick = { handleDisconnect() },
                        colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                    ) {
                        Icon(Icons.Default.Close, contentDescription = "Close", tint = TextPrimary)
                    }

                    // Mode chip toggle
                    FilterChip(
                        selected = controlMode == ControlMode.TOUCH_SCREEN,
                        onClick = {
                            controlMode = if (controlMode == ControlMode.TRACKPAD) ControlMode.TOUCH_SCREEN else ControlMode.TRACKPAD
                        },
                        label = {
                            Text(
                                if (controlMode == ControlMode.TOUCH_SCREEN) "Touch Screen" else "Trackpad",
                                fontSize = 12.sp,
                                fontWeight = FontWeight.Bold
                            )
                        },
                        leadingIcon = {
                            Icon(
                                if (controlMode == ControlMode.TOUCH_SCREEN) Icons.Default.TouchApp else Icons.Default.Mouse,
                                contentDescription = null,
                                modifier = Modifier.size(16.dp)
                            )
                        },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = AccentIndigo,
                            selectedLabelColor = TextPrimary,
                            containerColor = SurfaceDark,
                            labelColor = TextSecondary
                        )
                    )

                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        // Quick Zoom button
                        IconButton(
                            onClick = {
                                zoomScale = if (zoomScale >= 2.0f) 1.0f else (zoomScale + 0.5f)
                                if (zoomScale == 1.0f) {
                                    zoomOffsetX = 0f
                                    zoomOffsetY = 0f
                                }
                            },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (zoomScale > 1.0f) AccentIndigo else SurfaceDark
                            )
                        ) {
                            Icon(Icons.Default.ZoomIn, contentDescription = "Zoom", tint = if (zoomScale > 1.0f) TextPrimary else TextMuted)
                        }

                        // Privacy shield toggle
                        IconButton(
                            onClick = { togglePrivacyCurtain() },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (privacyCurtainActive) Color(0xFF10B981) else SurfaceDark
                            )
                        ) {
                            Icon(
                                Icons.Default.Security,
                                contentDescription = "Privacy Shield",
                                tint = if (privacyCurtainActive) Color.White else TextMuted
                            )
                        }

                        // Vertical PC Mode toggle (Rotate Windows to 1080x1920 to fit phone vertically!)
                        IconButton(
                            onClick = { toggleVerticalPcMode() },
                            colors = IconButtonDefaults.iconButtonColors(
                                containerColor = if (isVerticalPcMode) AccentIndigo else SurfaceDark
                            )
                        ) {
                            Icon(
                                Icons.Default.StayCurrentPortrait,
                                contentDescription = "Fit Vertical PC",
                                tint = if (isVerticalPcMode) TextPrimary else TextMuted
                            )
                        }

                        // Rotate to Landscape
                        IconButton(
                            onClick = { toggleOrientation() },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.ScreenRotation, contentDescription = "Rotate", tint = TextPrimary)
                        }

                        IconButton(
                            onClick = { showSettings = true },
                            colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceDark)
                        ) {
                            Icon(Icons.Default.Settings, contentDescription = "Settings", tint = TextPrimary)
                        }
                    }
                }

                // Privacy Status Banner if active
                if (privacyCurtainActive) {
                    Surface(
                        modifier = Modifier.fillMaxWidth(),
                        color = Color(0xFF10B981).copy(alpha = 0.15f),
                        shape = RoundedCornerShape(10.dp),
                        border = androidx.compose.foundation.BorderStroke(1.dp, Color(0xFF10B981).copy(alpha = 0.5f))
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Icon(Icons.Default.Security, contentDescription = null, tint = Color(0xFF10B981), modifier = Modifier.size(16.dp))
                            Text(
                                text = "🔒 Laptop screen is pitch black (People at home cannot see)",
                                color = Color(0xFF10B981),
                                fontSize = 12.sp,
                                fontWeight = FontWeight.SemiBold
                            )
                        }
                    }
                }

                // Top Half: Live Video Stream Surface with Pinch-to-zoom
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .clip(MaterialTheme.shapes.large)
                        .background(SurfaceDark)
                        .pointerInput(Unit) {
                            detectTransformGestures { _, pan, zoom, _ ->
                                zoomScale = (zoomScale * zoom).coerceIn(1.0f, 4.0f)
                                if (zoomScale > 1.0f) {
                                    zoomOffsetX += pan.x
                                    zoomOffsetY += pan.y
                                } else {
                                    zoomOffsetX = 0f
                                    zoomOffsetY = 0f
                                }
                            }
                        }
                        .pointerInput(controlMode, zoomScale) {
                            if (controlMode == ControlMode.TOUCH_SCREEN && zoomScale <= 1.05f) {
                                awaitEachGesture {
                                    val down = awaitFirstDown()
                                    val normX = (down.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                    val normY = (down.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                    webRtcManager.sendInput(InputEvent.TouchDirect(normX, normY, InputEvent.TouchDirect.ACTION_DOWN))

                                    val pointerId = down.id
                                    while (true) {
                                        val event = awaitPointerEvent()
                                        val current = event.changes.firstOrNull { it.id == pointerId } ?: break
                                        if (!current.pressed) {
                                            val upNormX = (current.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                            val upNormY = (current.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                            webRtcManager.sendInput(InputEvent.TouchDirect(upNormX, upNormY, InputEvent.TouchDirect.ACTION_UP))
                                            break
                                        }
                                        val curNormX = (current.position.x / size.width.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                        val curNormY = (current.position.y / size.height.toFloat() * 65535f).toInt().coerceIn(0, 65535)
                                        webRtcManager.sendInput(InputEvent.TouchDirect(curNormX, curNormY, InputEvent.TouchDirect.ACTION_MOVE))
                                        current.consume()
                                    }
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
                        if (rtcState is RtcConnectionState.Connected) {
                            AndroidView(
                                factory = { ctx ->
                                    SurfaceViewRenderer(ctx).apply {
                                        setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                        webRtcManager.attachRenderer(this)
                                    }
                                },
                                modifier = Modifier.fillMaxSize()
                            )
                        } else if (currentFrame != null) {
                            Image(
                                bitmap = currentFrame!!.asImageBitmap(),
                                contentDescription = "Desktop Stream",
                                modifier = Modifier.fillMaxSize(),
                                contentScale = if (isVerticalPcMode) ContentScale.Crop else ContentScale.Fit
                            )
                        } else {
                            Surface(
                                modifier = Modifier
                                    .align(Alignment.Center)
                                    .padding(8.dp),
                                color = SurfaceDark.copy(alpha = 0.85f),
                                shape = MaterialTheme.shapes.medium
                            ) {
                                Text(
                                    text = "Connecting to desktop...",
                                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                    style = Typography.bodyMedium,
                                    color = AccentIndigoBright
                                )
                            }
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

                // Middle: Dedicated Virtual Trackpad
                TrackpadSurface(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1.3f),
                    sensitivity = sensitivity,
                    scrollSpeed = scrollSpeed,
                    hapticsEnabled = hapticsEnabled,
                    onInputEvent = { webRtcManager.sendInput(it) }
                )

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
                    onSwitchMonitor = { switchMonitor() }
                )
            }
        }

        // Hidden input field for IME native keyboard capture
        BasicTextField(
            value = keyboardInput,
            onValueChange = { newValue ->
                val oldText = keyboardInput.text
                val newText = newValue.text

                if (newText.length > oldText.length) {
                    val added = newText.substring(oldText.length)
                    webRtcManager.sendInput(InputEvent.TextInput(added))
                } else if (newText.length < oldText.length) {
                    // Backspace
                    webRtcManager.sendInput(InputEvent.KeyDown(0x08, 0))
                    webRtcManager.sendInput(InputEvent.KeyUp(0x08))
                }
                keyboardInput = newValue
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

        // Settings Sheet
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
    }
}
