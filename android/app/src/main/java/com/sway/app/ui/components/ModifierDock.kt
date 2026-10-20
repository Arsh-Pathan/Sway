package com.sway.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.foundation.border
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sway.app.model.InputEvent
import com.sway.app.ui.theme.*

// Windows Virtual Key codes
const val VK_BACK: Short = 0x08
const val VK_TAB: Short = 0x09
const val VK_RETURN: Short = 0x0D
const val VK_ESCAPE: Short = 0x1B
const val VK_SPACE: Short = 0x20
const val VK_LEFT: Short = 0x25
const val VK_UP: Short = 0x26
const val VK_RIGHT: Short = 0x27
const val VK_DOWN: Short = 0x28
const val VK_VOLUME_MUTE: Short = 0xAD
const val VK_VOLUME_DOWN: Short = 0xAE
const val VK_VOLUME_UP: Short = 0xAF
const val VK_MEDIA_PLAY_PAUSE: Short = 0xB3

@Composable
fun ModifierDock(
    modifier: Modifier = Modifier,
    onInputEvent: (InputEvent) -> Unit,
    onToggleKeyboard: () -> Unit,
    onOpenSettings: () -> Unit,
    onSwitchMonitor: () -> Unit,
    onOpenQuickActions: () -> Unit = {}
) {
    var ctrlActive by remember { mutableStateOf(false) }
    var altActive by remember { mutableStateOf(false) }
    var shiftActive by remember { mutableStateOf(false) }
    var winActive by remember { mutableStateOf(false) }

    DisposableEffect(Unit) {
        onDispose {
            if (ctrlActive) onInputEvent(InputEvent.KeyUp(0x11))
            if (altActive) onInputEvent(InputEvent.KeyUp(0x12))
            if (shiftActive) onInputEvent(InputEvent.KeyUp(0x10))
            if (winActive) onInputEvent(InputEvent.KeyUp(0x5B))
        }
    }

    fun calculateModifierMask(): Byte {
        var mask = 0
        if (ctrlActive) mask = mask or 0x01
        if (shiftActive) mask = mask or 0x02
        if (altActive) mask = mask or 0x04
        if (winActive) mask = mask or 0x08
        return mask.toByte()
    }

    fun sendKeyPress(vkey: Short) {
        val mask = calculateModifierMask()
        onInputEvent(InputEvent.KeyDown(vkey, mask))
        onInputEvent(InputEvent.KeyUp(vkey))
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(SurfaceDark)
            .border(1.dp, SurfaceCardBorder, RoundedCornerShape(16.dp))
            .padding(8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        // Row 1: Modifier Keys + Esc + Tab
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            ModifierKeyButton(
                label = "CTRL",
                isActive = ctrlActive,
                modifier = Modifier.weight(1f),
                onClick = {
                    ctrlActive = !ctrlActive
                    if (ctrlActive) onInputEvent(InputEvent.KeyDown(0x11, 0)) else onInputEvent(InputEvent.KeyUp(0x11))
                }
            )
            ModifierKeyButton(
                label = "ALT",
                isActive = altActive,
                modifier = Modifier.weight(1f),
                onClick = {
                    altActive = !altActive
                    if (altActive) onInputEvent(InputEvent.KeyDown(0x12, 0)) else onInputEvent(InputEvent.KeyUp(0x12))
                }
            )
            ModifierKeyButton(
                label = "SHIFT",
                isActive = shiftActive,
                modifier = Modifier.weight(1f),
                onClick = {
                    shiftActive = !shiftActive
                    if (shiftActive) onInputEvent(InputEvent.KeyDown(0x10, 0)) else onInputEvent(InputEvent.KeyUp(0x10))
                }
            )
            ModifierKeyButton(
                label = "WIN",
                isActive = winActive,
                modifier = Modifier.weight(1f),
                onClick = {
                    winActive = !winActive
                    if (winActive) onInputEvent(InputEvent.KeyDown(0x5B, 0)) else onInputEvent(InputEvent.KeyUp(0x5B))
                }
            )
            ActionKeyButton(
                label = "ESC",
                modifier = Modifier.weight(1f),
                onClick = { sendKeyPress(VK_ESCAPE) }
            )
            ActionKeyButton(
                label = "TAB",
                modifier = Modifier.weight(1f),
                onClick = { sendKeyPress(VK_TAB) }
            )
        }

        // Row 2: Navigation & Quick Shortcuts
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            IconButton(
                icon = Icons.Default.Keyboard,
                modifier = Modifier.weight(1f),
                onClick = onToggleKeyboard
            )
            IconButton(
                icon = Icons.Default.DesktopWindows,
                modifier = Modifier.weight(1f),
                onClick = onSwitchMonitor
            )
            ActionKeyButton(
                label = "ENTER",
                modifier = Modifier.weight(1.2f),
                onClick = { sendKeyPress(VK_RETURN) }
            )
            ActionKeyButton(
                label = "BKSP",
                modifier = Modifier.weight(1.1f),
                onClick = { sendKeyPress(VK_BACK) }
            )
            IconButton(
                icon = Icons.AutoMirrored.Filled.ArrowBack,
                modifier = Modifier.weight(0.9f),
                onClick = { sendKeyPress(VK_LEFT) }
            )
            IconButton(
                icon = Icons.Default.ArrowUpward,
                modifier = Modifier.weight(0.9f),
                onClick = { sendKeyPress(VK_UP) }
            )
            IconButton(
                icon = Icons.Default.ArrowDownward,
                modifier = Modifier.weight(0.9f),
                onClick = { sendKeyPress(VK_DOWN) }
            )
            IconButton(
                icon = Icons.AutoMirrored.Filled.ArrowForward,
                modifier = Modifier.weight(0.9f),
                onClick = { sendKeyPress(VK_RIGHT) }
            )
            IconButton(
                icon = Icons.Default.Bolt,
                modifier = Modifier.weight(0.9f),
                onClick = onOpenQuickActions
            )
            IconButton(
                icon = Icons.Default.Settings,
                modifier = Modifier.weight(0.9f),
                onClick = onOpenSettings
            )
        }
    }
}

@Composable
fun ModifierKeyButton(
    label: String,
    isActive: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .height(38.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (isActive) KeyModifierActive else KeyModifierInactive)
            .border(1.dp, if (isActive) AccentWhite else SurfaceCardBorder, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            color = if (isActive) Color.Black else TextPrimary
        )
    }
}

@Composable
fun ActionKeyButton(
    label: String,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .height(38.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(SurfaceCard)
            .border(1.dp, SurfaceCardBorder, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
            color = TextPrimary
        )
    }
}

@Composable
fun IconButton(
    icon: ImageVector,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .height(38.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(SurfaceCard)
            .border(1.dp, SurfaceCardBorder, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = TextPrimary,
            modifier = Modifier.size(18.dp)
        )
    }
}
