package com.sway.app.ui.components

import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sway.app.model.InputEvent
import com.sway.app.ui.screens.ControlMode
import com.sway.app.ui.screens.ViewFitMode
import com.sway.app.ui.theme.*

enum class QuickPreset {
    PRODUCTIVITY,
    CINEMA_MEDIA,
    TURBO_GAMING,
    PURE_DECK,
    CODE_REVIEW,
    POWER_SAVER,
    STEALTH_MEETING
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ControlCenterSheet(
    controlMode: ControlMode,
    isVerticalPc: Boolean,
    isPrivacyEnabled: Boolean,
    viewFitMode: ViewFitMode,
    currentQuality: Int,
    sensitivity: Float,
    scrollSpeed: Float,
    isSpeakerAudioEnabled: Boolean,
    isMicAudioEnabled: Boolean,
    onApplyPreset: (QuickPreset) -> Unit,
    onToggleControlMode: (ControlMode) -> Unit,
    onToggleVerticalPc: () -> Unit,
    onTogglePrivacyCurtain: (Boolean) -> Unit,
    onChangeFitMode: (ViewFitMode) -> Unit,
    onChangeQuality: (Int) -> Unit,
    onChangeSensitivity: (Float) -> Unit,
    onChangeScrollSpeed: (Float) -> Unit,
    onToggleSpeakerAudio: (Boolean) -> Unit,
    onToggleMicAudio: (Boolean) -> Unit,
    onSendInput: (InputEvent) -> Unit,
    onDismiss: () -> Unit,
    onDisconnect: () -> Unit
) {
    val context = LocalContext.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = SurfaceDark,
        tonalElevation = 12.dp,
        shape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 8.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Slider Pull Handle Indicator
            Box(
                modifier = Modifier
                    .width(40.dp)
                    .height(4.dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(TextMuted.copy(alpha = 0.5f))
                    .align(Alignment.CenterHorizontally)
            )

            // Header
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "CONTROL CENTER",
                        fontSize = 17.sp,
                        fontWeight = FontWeight.Bold,
                        color = Color.White,
                        letterSpacing = 1.sp
                    )
                    Text(
                        text = "Presets, audio streaming & desktop quick tools",
                        fontSize = 12.sp,
                        color = TextSecondary
                    )
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Default.Close, contentDescription = "Close", tint = TextMuted)
                }
            }

            // ==================== 1. PRESETS ====================
            SectionLabel("QUICK PRESETS")
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PresetCard(
                    title = "Productivity",
                    subtitle = "1080p · Trackpad",
                    icon = Icons.Default.Work,
                    isSelected = controlMode != ControlMode.DECK && !isSpeakerAudioEnabled,
                    onClick = { onApplyPreset(QuickPreset.PRODUCTIVITY) }
                )
                PresetCard(
                    title = "Media Cinema",
                    subtitle = "Audio ON · 1080p",
                    icon = Icons.Default.Movie,
                    isSelected = isSpeakerAudioEnabled,
                    onClick = { onApplyPreset(QuickPreset.CINEMA_MEDIA) }
                )
                PresetCard(
                    title = "Turbo Gaming",
                    subtitle = "60 FPS · Fast Sens",
                    icon = Icons.Default.Gamepad,
                    isSelected = currentQuality == 0,
                    onClick = { onApplyPreset(QuickPreset.TURBO_GAMING) }
                )
                PresetCard(
                    title = "Pure Deck",
                    subtitle = "0 Video · 1ms Latency",
                    icon = Icons.Default.TouchApp,
                    isSelected = controlMode == ControlMode.DECK,
                    onClick = { onApplyPreset(QuickPreset.PURE_DECK) }
                )
                PresetCard(
                    title = "Code & Reading",
                    subtitle = "Crisp Text · Touch",
                    icon = Icons.Default.MenuBook,
                    isSelected = currentQuality == 2 && controlMode == ControlMode.TOUCH_SCREEN,
                    onClick = { onApplyPreset(QuickPreset.CODE_REVIEW) }
                )
                PresetCard(
                    title = "Battery Saver",
                    subtitle = "720p · Data Saver",
                    icon = Icons.Default.BatteryChargingFull,
                    isSelected = currentQuality == 0 && !isSpeakerAudioEnabled && controlMode != ControlMode.DECK,
                    onClick = { onApplyPreset(QuickPreset.POWER_SAVER) }
                )
                PresetCard(
                    title = "Stealth Meeting",
                    subtitle = "Curtain ON · Black Screen",
                    icon = Icons.Default.VisibilityOff,
                    isSelected = isPrivacyEnabled,
                    onClick = { onApplyPreset(QuickPreset.STEALTH_MEETING) }
                )
            }

            // ==================== 2. AUDIO & STREAMING ====================
            SectionLabel("AUDIO & STREAMING (OPTIONAL)")
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                // Speaker Audio (PC Sound to Phone)
                ToggleTile(
                    modifier = Modifier.weight(1f),
                    title = "PC Sound to Phone",
                    subtitle = if (isSpeakerAudioEnabled) "Streaming Active" else "Off",
                    icon = if (isSpeakerAudioEnabled) Icons.Default.VolumeUp else Icons.Default.VolumeOff,
                    isActive = isSpeakerAudioEnabled,
                    onToggle = { onToggleSpeakerAudio(!isSpeakerAudioEnabled) }
                )

                // Microphone Audio (Phone Mic to PC)
                ToggleTile(
                    modifier = Modifier.weight(1f),
                    title = "Phone Mic to PC",
                    subtitle = if (isMicAudioEnabled) "Active (Cable Req)" else "Muted (Virtual Aux)",
                    icon = if (isMicAudioEnabled) Icons.Default.Mic else Icons.Default.MicOff,
                    isActive = isMicAudioEnabled,
                    onToggle = { onToggleMicAudio(!isMicAudioEnabled) }
                )
            }

            // ==================== 3. DISPLAY & PRIVACY ====================
            SectionLabel("DISPLAY & PRIVACY")
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                // Vertical PC Mode
                ToggleTile(
                    modifier = Modifier.weight(1f),
                    title = "Vertical PC Mode",
                    subtitle = if (isVerticalPc) "1080x1920 Active" else "Landscape",
                    icon = Icons.Default.StayCurrentPortrait,
                    isActive = isVerticalPc,
                    onToggle = onToggleVerticalPc
                )

                // Privacy Curtain
                ToggleTile(
                    modifier = Modifier.weight(1f),
                    title = "Privacy Curtain",
                    subtitle = if (isPrivacyEnabled) "Host Screen Black" else "Visible",
                    icon = Icons.Default.VisibilityOff,
                    isActive = isPrivacyEnabled,
                    onToggle = { onTogglePrivacyCurtain(!isPrivacyEnabled) }
                )
            }

            // Fit Mode Selector
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                FitModeButton("Fit Aspect", Icons.Default.AspectRatio, viewFitMode == ViewFitMode.FIT, Modifier.weight(1f)) {
                    onChangeFitMode(ViewFitMode.FIT)
                }
                FitModeButton("Fill Screen", Icons.Default.Fullscreen, viewFitMode == ViewFitMode.FILL, Modifier.weight(1f)) {
                    onChangeFitMode(ViewFitMode.FILL)
                }
                FitModeButton("Rotate 90", Icons.Default.Rotate90DegreesCw, viewFitMode == ViewFitMode.ROTATE_90, Modifier.weight(1f)) {
                    onChangeFitMode(ViewFitMode.ROTATE_90)
                }
            }

            // ==================== 4. QUALITY & SENSITIVITY SLIDERS ====================
            SectionLabel("TUNING & SLIDERS")
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .background(SurfaceCard)
                    .border(1.dp, SurfaceCardBorder, RoundedCornerShape(12.dp))
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                // Pointer Sensitivity
                Column {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("Trackpad Pointer Sensitivity", fontSize = 12.sp, color = TextPrimary, fontWeight = FontWeight.SemiBold)
                        Text(String.format("%.1fx", sensitivity), fontSize = 12.sp, color = Color.White, fontWeight = FontWeight.Bold)
                    }
                    Slider(
                        value = sensitivity,
                        onValueChange = onChangeSensitivity,
                        valueRange = 0.5f..3.0f,
                        colors = SliderDefaults.colors(
                            thumbColor = Color.White,
                            activeTrackColor = Color.White,
                            inactiveTrackColor = SurfaceDark
                        )
                    )
                }

                // Scroll Speed
                Column {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("Two-Finger Scroll Speed", fontSize = 12.sp, color = TextPrimary, fontWeight = FontWeight.SemiBold)
                        Text(String.format("%.1fx", scrollSpeed), fontSize = 12.sp, color = Color.White, fontWeight = FontWeight.Bold)
                    }
                    Slider(
                        value = scrollSpeed,
                        onValueChange = onChangeScrollSpeed,
                        valueRange = 0.5f..3.0f,
                        colors = SliderDefaults.colors(
                            thumbColor = Color.White,
                            activeTrackColor = Color.White,
                            inactiveTrackColor = SurfaceDark
                        )
                    )
                }

                // Stream Quality
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Stream Quality", fontSize = 12.sp, color = TextPrimary, fontWeight = FontWeight.SemiBold)
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        QualityChip("Turbo", currentQuality == 0) { onChangeQuality(0) }
                        QualityChip("Balanced", currentQuality == 1) { onChangeQuality(1) }
                        QualityChip("High", currentQuality == 2) { onChangeQuality(2) }
                    }
                }
            }

            // ==================== 5. QUICK SHORTCUTS & DESKTOP TOOLS ====================
            SectionLabel("DESKTOP QUICK COMMANDS")
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                ToolChip("Desktop", Icons.Default.DesktopWindows, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SHOW_DESKTOP))
                    Toast.makeText(context, "Show Desktop", Toast.LENGTH_SHORT).show()
                }
                ToolChip("Task View", Icons.Default.Layers, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.WIN_TAB))
                }
                ToolChip("Close App", Icons.Default.Close, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.CLOSE_WINDOW))
                }
                ToolChip("Snipping", Icons.Default.Crop, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SNIPPING_TOOL))
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                ToolChip("Paste Clip", Icons.Default.ContentPaste, Modifier.weight(1f)) {
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                    if (clipboard != null && clipboard.hasPrimaryClip()) {
                        val clip = clipboard.primaryClip
                        if (clip != null && clip.itemCount > 0) {
                            val text = clip.getItemAt(0).coerceToText(context).toString()
                            if (text.isNotEmpty()) {
                                onSendInput(InputEvent.ClipboardSync(text))
                                Toast.makeText(context, "Copied to PC!", Toast.LENGTH_SHORT).show()
                            }
                        }
                    }
                }
                ToolChip("Task Mgr", Icons.Default.Build, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.TASK_MANAGER))
                }
                ToolChip("Lock PC", Icons.Default.Lock, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.LOCK))
                }
                ToolChip("Explorer", Icons.Default.Folder, Modifier.weight(1f)) {
                    onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.FILE_EXPLORER))
                }
            }

            // Disconnect Button
            Surface(
                onClick = onDisconnect,
                color = SurfaceDark,
                shape = RoundedCornerShape(12.dp),
                border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    modifier = Modifier.padding(14.dp),
                    horizontalArrangement = Arrangement.Center,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Default.PowerSettingsNew, contentDescription = null, tint = TextMuted, modifier = Modifier.size(16.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("End Sway Session", fontSize = 13.sp, fontWeight = FontWeight.Bold, color = TextMuted)
                }
            }

            Spacer(modifier = Modifier.height(28.dp))
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text = text,
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        color = TextSecondary,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(top = 4.dp, bottom = 2.dp)
    )
}

@Composable
private fun PresetCard(
    title: String,
    subtitle: String,
    icon: ImageVector,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = if (isSelected) Color.White else SurfaceCard,
        shape = RoundedCornerShape(12.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, if (isSelected) Color.White else SurfaceCardBorder),
        modifier = Modifier.width(135.dp)
    ) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = if (isSelected) Color.Black else Color.White,
                modifier = Modifier.size(20.dp)
            )
            Text(
                text = title,
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold,
                color = if (isSelected) Color.Black else Color.White
            )
            Text(
                text = subtitle,
                fontSize = 10.sp,
                color = if (isSelected) Color(0xFF333333) else TextSecondary
            )
        }
    }
}

@Composable
private fun ToggleTile(
    modifier: Modifier = Modifier,
    title: String,
    subtitle: String,
    icon: ImageVector,
    isActive: Boolean,
    onToggle: () -> Unit
) {
    Surface(
        onClick = onToggle,
        color = if (isActive) Color.White else SurfaceCard,
        shape = RoundedCornerShape(12.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, if (isActive) Color.White else SurfaceCardBorder),
        modifier = modifier
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = if (isActive) Color.Black else Color.White,
                modifier = Modifier.size(20.dp)
            )
            Column {
                Text(
                    text = title,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold,
                    color = if (isActive) Color.Black else Color.White
                )
                Text(
                    text = subtitle,
                    fontSize = 10.sp,
                    color = if (isActive) Color(0xFF333333) else TextSecondary
                )
            }
        }
    }
}

@Composable
private fun FitModeButton(
    title: String,
    icon: ImageVector,
    isSelected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = if (isSelected) Color.White else SurfaceCard,
        shape = RoundedCornerShape(8.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, if (isSelected) Color.White else SurfaceCardBorder),
        modifier = modifier
    ) {
        Row(
            modifier = Modifier.padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center
        ) {
            Icon(icon, contentDescription = null, tint = if (isSelected) Color.Black else Color.White, modifier = Modifier.size(14.dp))
            Spacer(modifier = Modifier.width(4.dp))
            Text(title, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = if (isSelected) Color.Black else Color.White)
        }
    }
}

@Composable
private fun QualityChip(
    title: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = if (isSelected) Color.White else SurfaceDark,
        shape = RoundedCornerShape(6.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, if (isSelected) Color.White else SurfaceCardBorder)
    ) {
        Text(
            text = title,
            fontSize = 11.sp,
            fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium,
            color = if (isSelected) Color.Black else TextSecondary,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
        )
    }
}

@Composable
private fun ToolChip(
    title: String,
    icon: ImageVector,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = SurfaceCard,
        shape = RoundedCornerShape(8.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
        modifier = modifier
    ) {
        Column(
            modifier = Modifier.padding(vertical = 8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            Icon(icon, contentDescription = null, tint = Color.White, modifier = Modifier.size(16.dp))
            Text(title, fontSize = 10.sp, fontWeight = FontWeight.Medium, color = Color.White)
        }
    }
}
