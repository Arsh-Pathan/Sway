package com.sway.app.ui.components

import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
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
                .padding(horizontal = 24.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
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
                        tint = AccentIndigoBright,
                        modifier = Modifier.size(24.dp)
                    )
                    Text(
                        text = "Quick Actions & Power",
                        style = Typography.headlineMedium,
                        color = TextPrimary
                    )
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Default.Close, contentDescription = "Close", tint = TextMuted)
                }
            }

            Text(
                text = "Instant desktop commands and synchronized clipboard utilities.",
                style = Typography.bodyMedium,
                color = TextSecondary
            )

            // Grid / list of actions
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                // 1. Paste Phone Clipboard
                QuickActionRow(
                    icon = Icons.Default.ContentPaste,
                    title = "Paste Phone Clipboard to PC",
                    subtitle = "Sends phone's copied text to host clipboard instantly",
                    accentColor = AccentIndigo,
                    onClick = {
                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                        if (clipboard != null && clipboard.hasPrimaryClip()) {
                            val clip = clipboard.primaryClip
                            if (clip != null && clip.itemCount > 0) {
                                val text = clip.getItemAt(0).coerceToText(context).toString()
                                if (text.isNotEmpty()) {
                                    onSendInput(InputEvent.ClipboardSync(text))
                                    Toast.makeText(context, "Copied ${text.length} chars to PC!", Toast.LENGTH_SHORT).show()
                                } else {
                                    Toast.makeText(context, "Clipboard text is empty", Toast.LENGTH_SHORT).show()
                                }
                            }
                        } else {
                            Toast.makeText(context, "Phone clipboard is empty", Toast.LENGTH_SHORT).show()
                        }
                        onDismiss()
                    }
                )

                // 2. Show Desktop (Win + D)
                QuickActionRow(
                    icon = Icons.Default.DesktopWindows,
                    title = "Show Desktop",
                    subtitle = "Minimizes all windows (Win + D)",
                    accentColor = Color(0xFF3B82F6),
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SHOW_DESKTOP))
                        Toast.makeText(context, "Showing Desktop", Toast.LENGTH_SHORT).show()
                        onDismiss()
                    }
                )

                // 3. Task Manager (Ctrl + Shift + Esc)
                QuickActionRow(
                    icon = Icons.Default.Build,
                    title = "Open Task Manager",
                    subtitle = "Launches Windows Task Manager (Ctrl + Shift + Esc)",
                    accentColor = Color(0xFFF59E0B),
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.TASK_MANAGER))
                        Toast.makeText(context, "Opening Task Manager", Toast.LENGTH_SHORT).show()
                        onDismiss()
                    }
                )

                // 4. Lock Workstation
                QuickActionRow(
                    icon = Icons.Default.Lock,
                    title = "Lock PC",
                    subtitle = "Securely locks Windows session (Win + L)",
                    accentColor = Color(0xFF8B5CF6),
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.LOCK))
                        Toast.makeText(context, "Host PC locked", Toast.LENGTH_SHORT).show()
                        onDismiss()
                    }
                )

                // 5. Turn Off Monitor
                QuickActionRow(
                    icon = Icons.Default.VisibilityOff,
                    title = "Turn Off Monitor Display",
                    subtitle = "Powers down PC screen without stopping session",
                    accentColor = Color(0xFF10B981),
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.TURN_OFF_SCREEN))
                        Toast.makeText(context, "Monitor display powered off", Toast.LENGTH_SHORT).show()
                        onDismiss()
                    }
                )

                // 6. Sleep PC
                QuickActionRow(
                    icon = Icons.Default.PowerSettingsNew,
                    title = "Sleep Laptop / PC",
                    subtitle = "Puts host computer into low-power sleep mode",
                    accentColor = AccentRose,
                    onClick = {
                        onSendInput(InputEvent.SystemCommand(InputEvent.SystemCommand.SLEEP))
                        Toast.makeText(context, "Sending PC to sleep...", Toast.LENGTH_SHORT).show()
                        onDismiss()
                    }
                )
            }

            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

@Composable
private fun QuickActionRow(
    icon: ImageVector,
    title: String,
    subtitle: String,
    accentColor: Color,
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
                    .size(40.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(SurfaceDark)
                    .border(1.dp, SurfaceCardBorder, RoundedCornerShape(10.dp)),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = TextPrimary,
                    modifier = Modifier.size(20.dp)
                )
            }

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary
                )
                Text(
                    text = subtitle,
                    fontSize = 12.sp,
                    color = TextSecondary,
                    lineHeight = 16.sp
                )
            }

            Icon(
                imageVector = Icons.Default.ChevronRight,
                contentDescription = null,
                tint = TextMuted,
                modifier = Modifier.size(18.dp)
            )
        }
    }
}
