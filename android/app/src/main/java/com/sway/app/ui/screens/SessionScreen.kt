package com.sway.app.ui.screens

import android.content.res.Configuration
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
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

@Composable
fun SessionScreen(
    webRtcManager: WebRtcManager,
    onDisconnect: () -> Unit
) {
    val configuration = LocalConfiguration.current
    val isLandscape = configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
    val rtcState by webRtcManager.connectionState.collectAsState()

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

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BackgroundDark)
    ) {
        if (isLandscape) {
            // ==================== LANDSCAPE MODE: IMMERSIVE STREAM ====================
            Box(modifier = Modifier.fillMaxSize()) {
                AndroidView(
                    factory = { ctx ->
                        SurfaceViewRenderer(ctx).apply {
                            setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                            webRtcManager.attachRenderer(this)
                        }
                    },
                    modifier = Modifier.fillMaxSize()
                )

                // Transparent gesture surface on top of screen
                TrackpadSurface(
                    modifier = Modifier.fillMaxSize(),
                    sensitivity = sensitivity,
                    scrollSpeed = scrollSpeed,
                    hapticsEnabled = hapticsEnabled,
                    onInputEvent = { webRtcManager.sendInput(it) }
                )

                // Floating Action Header
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    IconButton(
                        onClick = onDisconnect,
                        colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceCard.copy(alpha = 0.8f))
                    ) {
                        Icon(Icons.Default.Close, contentDescription = "Close", tint = TextPrimary)
                    }

                    IconButton(
                        onClick = { showSettings = true },
                        colors = IconButtonDefaults.iconButtonColors(containerColor = SurfaceCard.copy(alpha = 0.8f))
                    ) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings", tint = TextPrimary)
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
                // Top Half: Live Video Stream Surface
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .clip(MaterialTheme.shapes.large)
                        .background(SurfaceDark)
                ) {
                    AndroidView(
                        factory = { ctx ->
                            SurfaceViewRenderer(ctx).apply {
                                setScalingType(RendererCommon.ScalingType.SCALE_ASPECT_FIT)
                                webRtcManager.attachRenderer(this)
                            }
                        },
                        modifier = Modifier.fillMaxSize()
                    )

                    // Connection status badge
                    if (rtcState !is RtcConnectionState.Connected) {
                        Surface(
                            modifier = Modifier
                                .align(Alignment.Center)
                                .padding(8.dp),
                            color = SurfaceDark.copy(alpha = 0.85f),
                            shape = MaterialTheme.shapes.medium
                        ) {
                            Text(
                                text = when (rtcState) {
                                    is RtcConnectionState.Connecting -> "Connecting to desktop..."
                                    is RtcConnectionState.Failed -> "Connection failed"
                                    else -> "Waiting for stream..."
                                },
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                style = Typography.bodyMedium,
                                color = AccentIndigoBright
                            )
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
                        if (isKeyboardOpen) {
                            focusRequester.requestFocus()
                        }
                    },
                    onOpenSettings = { showSettings = true },
                    onSwitchMonitor = { switchMonitor() }
                )
            }
        }

        // Hidden input field for IME keyboard capture
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
                onDisconnect = onDisconnect
            )
        }
    }
}
