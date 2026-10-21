package com.sway.app.ui.components

import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import com.sway.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickActionsSheet(
    onSendInput: (InputEvent) -> Unit,
    onDismiss: () -> Unit
) {
    val context = LocalContext.current

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = SurfaceDark,
        tonalElevation = 8.dp,
        shape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 12.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = Icons.Default.Bolt,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(22.dp)
                    )
                    Text(
                        text = "Remote Command & Tool Deck",
                        fontSize = 17.sp,
                        fontWeight = FontWeight.Bold,
                        color = Color.White
                    )
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Default.Close, contentDescription = "Close", tint = TextMuted)
                }
            }

            Text(
                text = "Instant desktop control commands and keyboard shortcuts.",
                fontSize = 12.sp,
                color = TextSecondary
            )

            // Section: System & Window Management
            ActionSectionHeader("SYSTEM & WINDOWS")
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                QuickActionRow(
                    icon = Icons.Default.ContentPaste,
                    title = "Paste Phone Clipboard",
                    subtitle = "Sends phone copied text directly to Windows",
                    onClick = {
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
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.DesktopWindows,
                    title = "Show Desktop",
                    subtitle = "Minimizes all open windows (Win + D)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SHOW_DESKTOP))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.Layers,
                    title = "Task View",
                    subtitle = "Shows timeline and all desktop spaces (Win + Tab)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.WIN_TAB))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.SwitchRight,
                    title = "Switch Application",
                    subtitle = "Cycle between open applications (Alt + Tab)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.ALT_TAB))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.Crop,
                    title = "Snipping Tool Screenshot",
                    subtitle = "Captures an area of the PC screen (Win + Shift + S)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SNIPPING_TOOL))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.Close,
                    title = "Close Active Window",
                    subtitle = "Closes current window cleanly (Alt + F4)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.CLOSE_WINDOW))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.Build,
                    title = "Task Manager",
                    subtitle = "Launches Windows Task Manager (Ctrl + Shift + Esc)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.TASK_MANAGER))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.Lock,
                    title = "Lock PC",
                    subtitle = "Securely locks Windows workstation (Win + L)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.LOCK))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.VisibilityOff,
                    title = "Turn Off Screen",
                    subtitle = "Puts host display to sleep without ending session",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.TURN_OFF_SCREEN))
                        onDismiss()
                    }
                )

                QuickActionRow(
                    icon = Icons.Default.PowerSettingsNew,
                    title = "Sleep PC",
                    subtitle = "Puts host computer into sleep state",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SLEEP))
                        onDismiss()
                    }
                )
            }

            // Section: Media & Sound
            ActionSectionHeader("MEDIA & AUDIO")
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                QuickActionRow(
                    icon = Icons.Default.PlayArrow,
                    title = "Play / Pause",
                    subtitle = "Toggles music or video playback",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.MEDIA_PLAY_PAUSE))
                        onDismiss()
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.VolumeUp,
                    title = "Volume Up",
                    subtitle = "Increases master output volume",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_UP))
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.VolumeDown,
                    title = "Volume Down",
                    subtitle = "Decreases master output volume",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_DOWN))
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.VolumeMute,
                    title = "Mute / Unmute",
                    subtitle = "Toggles system mute",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_MUTE))
                    }
                )
            }

            // Section: Web Browser
            ActionSectionHeader("BROWSER TOOLS")
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                QuickActionRow(
                    icon = Icons.Default.Add,
                    title = "New Tab",
                    subtitle = "Opens a new tab in browser (Ctrl + T)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_NEW_TAB))
                        onDismiss()
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.Close,
                    title = "Close Tab",
                    subtitle = "Closes active browser tab (Ctrl + W)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_CLOSE_TAB))
                        onDismiss()
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.Refresh,
                    title = "Refresh Page",
                    subtitle = "Reloads current webpage (F5)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_REFRESH))
                        onDismiss()
                    }
                )
            }

            // Section: Windows Tools
            ActionSectionHeader("UTILITIES")
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                QuickActionRow(
                    icon = Icons.Default.Folder,
                    title = "File Explorer",
                    subtitle = "Opens Windows Explorer (Win + E)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.FILE_EXPLORER))
                        onDismiss()
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.Terminal,
                    title = "Run Dialog",
                    subtitle = "Opens Windows Run prompt (Win + R)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.RUN_DIALOG))
                        onDismiss()
                    }
                )
                QuickActionRow(
                    icon = Icons.Default.Search,
                    title = "Windows Search",
                    subtitle = "Opens system search bar (Win + S)",
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.WINDOWS_SEARCH))
                        onDismiss()
                    }
                )
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }
}

@Composable
private fun ActionSectionHeader(title: String) {
    Text(
        text = title,
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        color = TextSecondary,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(top = 8.dp, bottom = 2.dp)
    )
}

@Composable
private fun QuickActionRow(
    icon: ImageVector,
    title: String,
    subtitle: String,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = SurfaceCard,
        shape = RoundedCornerShape(12.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(38.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(SurfaceDark)
                    .border(1.dp, SurfaceCardBorder, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier.size(18.dp)
                )
            }

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = Color.White
                )
                Text(
                    text = subtitle,
                    fontSize = 11.sp,
                    color = TextSecondary,
                    lineHeight = 15.sp
                )
            }

            Icon(
                imageVector = Icons.Default.ChevronRight,
                contentDescription = null,
                tint = TextMuted,
                modifier = Modifier.size(16.dp)
            )
        }
    }
}
