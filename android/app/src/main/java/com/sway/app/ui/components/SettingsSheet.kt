package com.sway.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sway.app.model.InputEvent
import com.sway.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsSheet(
    sensitivity: Float,
    onSensitivityChange: (Float) -> Unit,
    scrollSpeed: Float,
    onScrollSpeedChange: (Float) -> Unit,
    hapticsEnabled: Boolean,
    onHapticsToggle: (Boolean) -> Unit,
    currentQuality: Byte,
    onQualityChange: (Byte) -> Unit,
    onDismiss: () -> Unit,
    onDisconnect: () -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = SurfaceDark,
        tonalElevation = 8.dp,
        shape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            Text(
                text = "Session Controls & Preferences",
                style = Typography.headlineMedium,
                color = TextPrimary
            )

            // Sensitivity Slider
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(text = "Cursor Sensitivity", style = Typography.bodyLarge)
                    Text(
                        text = "%.1fx".format(sensitivity),
                        style = Typography.bodyLarge,
                        color = AccentIndigoBright
                    )
                }
                Slider(
                    value = sensitivity,
                    onValueChange = onSensitivityChange,
                    valueRange = 0.5f..3.0f,
                    colors = SliderDefaults.colors(
                        thumbColor = AccentIndigo,
                        activeTrackColor = AccentIndigo
                    )
                )
            }

            // Scroll Speed Slider
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(text = "Scroll Speed", style = Typography.bodyLarge)
                    Text(
                        text = "%.1fx".format(scrollSpeed),
                        style = Typography.bodyLarge,
                        color = AccentIndigoBright
                    )
                }
                Slider(
                    value = scrollSpeed,
                    onValueChange = onScrollSpeedChange,
                    valueRange = 0.5f..3.0f,
                    colors = SliderDefaults.colors(
                        thumbColor = AccentIndigo,
                        activeTrackColor = AccentIndigo
                    )
                )
            }

            // Haptics Toggle
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(text = "Haptic Vibration", style = Typography.bodyLarge)
                    Text(
                        text = "Subtle click and scroll tactile feedback",
                        style = Typography.bodyMedium,
                        color = TextMuted
                    )
                }
                Switch(
                    checked = hapticsEnabled,
                    onCheckedChange = onHapticsToggle,
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = TextPrimary,
                        checkedTrackColor = AccentIndigo
                    )
                )
            }

            // Stream Quality Mode
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(text = "Stream Quality Preset", style = Typography.bodyLarge)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    QualityOption(
                        label = "Data Saver\n720p @ 30fps",
                        isSelected = currentQuality == InputEvent.SetQuality.DATA_SAVER,
                        modifier = Modifier.weight(1f),
                        onClick = { onQualityChange(InputEvent.SetQuality.DATA_SAVER) }
                    )
                    QualityOption(
                        label = "Balanced\n1080p @ 60fps",
                        isSelected = currentQuality == InputEvent.SetQuality.BALANCED,
                        modifier = Modifier.weight(1f),
                        onClick = { onQualityChange(InputEvent.SetQuality.BALANCED) }
                    )
                    QualityOption(
                        label = "Native\nLossless Bitrate",
                        isSelected = currentQuality == InputEvent.SetQuality.NATIVE,
                        modifier = Modifier.weight(1f),
                        onClick = { onQualityChange(InputEvent.SetQuality.NATIVE) }
                    )
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Disconnect Button
            Button(
                onClick = onDisconnect,
                colors = ButtonDefaults.buttonColors(containerColor = AccentRose),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp),
                shape = RoundedCornerShape(12.dp)
            ) {
                Text(
                    text = "End Remote Session",
                    fontWeight = FontWeight.Bold,
                    color = TextPrimary
                )
            }
        }
    }
}

@Composable
fun QualityOption(
    label: String,
    isSelected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(10.dp))
            .background(if (isSelected) AccentIndigoGlow else SurfaceCard)
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp, horizontal = 4.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
            color = if (isSelected) AccentIndigoBright else TextSecondary,
            lineHeight = 15.sp,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center
        )
    }
}
