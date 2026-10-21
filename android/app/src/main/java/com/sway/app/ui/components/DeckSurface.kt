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
import com.sway.app.ui.theme.*

@Composable
fun DeckSurface(
    modifier: Modifier = Modifier,
    sensitivity: Float = 1.0f,
    scrollSpeed: Float = 1.0f,
    hapticsEnabled: Boolean = true,
    onInputEvent: (InputEvent) -> Unit,
    onOpenKeyboard: () -> Unit,
    onSwitchToStream: () -> Unit,
    onOpenActions: () -> Unit,
    onDisconnect: () -> Unit
) {
    val context = LocalContext.current
    var selectedCategory by remember { mutableStateOf(0) }
    val categories = listOf("SYSTEM", "MEDIA", "BROWSER", "EDIT", "LAUNCH")

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(BackgroundDark)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        // Top Header
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .background(Color.White)
                )
                Column {
                    Text(
                        text = "CONTROL DECK",
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Bold,
                        color = Color.White,
                        letterSpacing = 1.sp
                    )
                    Text(
                        text = "Real-Time 1ms · Zero Video Lag",
                        fontSize = 10.sp,
                        color = TextSecondary
                    )
                }
            }

            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Switch to Screen Stream View
                Surface(
                    onClick = onSwitchToStream,
                    color = Color.White,
                    shape = RoundedCornerShape(8.dp)
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Icon(Icons.Default.Tv, contentDescription = "Stream View", tint = Color.Black, modifier = Modifier.size(14.dp))
                        Text("Stream", color = Color.Black, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                    }
                }

                // Keyboard
                Surface(
                    onClick = onOpenKeyboard,
                    color = SurfaceDark,
                    shape = RoundedCornerShape(8.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                ) {
                    Box(modifier = Modifier.padding(6.dp)) {
                        Icon(Icons.Default.Keyboard, contentDescription = "Keyboard", tint = Color.White, modifier = Modifier.size(16.dp))
                    }
                }

                // Quick Actions
                Surface(
                    onClick = onOpenActions,
                    color = SurfaceDark,
                    shape = RoundedCornerShape(8.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                ) {
                    Box(modifier = Modifier.padding(6.dp)) {
                        Icon(Icons.Default.Bolt, contentDescription = "Tools", tint = Color.White, modifier = Modifier.size(16.dp))
                    }
                }

                // Disconnect
                Surface(
                    onClick = onDisconnect,
                    color = SurfaceDark,
                    shape = RoundedCornerShape(8.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder)
                ) {
                    Box(modifier = Modifier.padding(6.dp)) {
                        Icon(Icons.Default.PowerSettingsNew, contentDescription = "Disconnect", tint = TextMuted, modifier = Modifier.size(16.dp))
                    }
                }
            }
        }

        // Trackpad Surface (Adaptive height)
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1.3f)
        ) {
            TrackpadSurface(
                modifier = Modifier.fillMaxSize(),
                sensitivity = sensitivity,
                scrollSpeed = scrollSpeed,
                hapticsEnabled = hapticsEnabled,
                onInputEvent = onInputEvent
            )
        }

        // Physical-Style Mouse Click Bar
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            // Left Click
            Surface(
                onClick = {
                    onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                    onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                },
                color = SurfaceCard,
                shape = RoundedCornerShape(10.dp),
                border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
                modifier = Modifier.weight(1.6f)
            ) {
                Box(
                    modifier = Modifier.padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text("LEFT CLICK", fontSize = 11.sp, fontWeight = FontWeight.Bold, color = Color.White, letterSpacing = 0.5.sp)
                }
            }

            // Middle Click / Scroll Wheel Click
            Surface(
                onClick = {
                    onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_MIDDLE))
                    onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_MIDDLE))
                },
                color = SurfaceCard,
                shape = RoundedCornerShape(10.dp),
                border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
                modifier = Modifier.weight(1.0f)
            ) {
                Box(
                    modifier = Modifier.padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text("MID", fontSize = 11.sp, fontWeight = FontWeight.Bold, color = TextSecondary)
                }
            }

            // Right Click
            Surface(
                onClick = {
                    onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_RIGHT))
                    onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_RIGHT))
                },
                color = SurfaceCard,
                shape = RoundedCornerShape(10.dp),
                border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
                modifier = Modifier.weight(1.4f)
            ) {
                Box(
                    modifier = Modifier.padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text("RIGHT CLICK", fontSize = 11.sp, fontWeight = FontWeight.Bold, color = Color.White, letterSpacing = 0.5.sp)
                }
            }
        }

        // Tool Deck Category Selector
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            categories.forEachIndexed { index, cat ->
                val isSelected = selectedCategory == index
                Surface(
                    onClick = { selectedCategory = index },
                    color = if (isSelected) Color.White else SurfaceDark,
                    shape = RoundedCornerShape(8.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, if (isSelected) Color.White else SurfaceCardBorder)
                ) {
                    Text(
                        text = cat,
                        color = if (isSelected) Color.Black else TextSecondary,
                        fontSize = 11.sp,
                        fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp)
                    )
                }
            }
        }

        // Tool Command Grid for selected category
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1.0f)
                .clip(RoundedCornerShape(12.dp))
                .background(SurfaceDark)
                .border(1.dp, SurfaceCardBorder, RoundedCornerShape(12.dp))
                .padding(8.dp)
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                when (selectedCategory) {
                    0 -> { // SYSTEM
                        DeckRow {
                            DeckToolButton("Desktop", Icons.Default.DesktopWindows) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.SHOW_DESKTOP))
                            }
                            DeckToolButton("Task View", Icons.Default.Layers) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.WIN_TAB))
                            }
                            DeckToolButton("Alt + Tab", Icons.Default.SwitchRight) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.ALT_TAB))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Close App", Icons.Default.Close) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.CLOSE_WINDOW))
                            }
                            DeckToolButton("Snipping", Icons.Default.Crop) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.SNIPPING_TOOL))
                            }
                            DeckToolButton("Task Mgr", Icons.Default.Build) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.TASK_MANAGER))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Lock PC", Icons.Default.Lock) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.LOCK))
                            }
                            DeckToolButton("Screen Off", Icons.Default.VisibilityOff) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.TURN_OFF_SCREEN))
                            }
                            DeckToolButton("Sleep", Icons.Default.PowerSettingsNew) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.SLEEP))
                            }
                        }
                    }
                    1 -> { // MEDIA
                        DeckRow {
                            DeckToolButton("Play / Pause", Icons.Default.PlayArrow) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.MEDIA_PLAY_PAUSE))
                            }
                            DeckToolButton("Prev Track", Icons.Default.SkipPrevious) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.MEDIA_PREV))
                            }
                            DeckToolButton("Next Track", Icons.Default.SkipNext) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.MEDIA_NEXT))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Volume +", Icons.Default.VolumeUp) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_UP))
                            }
                            DeckToolButton("Volume -", Icons.Default.VolumeDown) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_DOWN))
                            }
                            DeckToolButton("Mute Audio", Icons.Default.VolumeMute) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.VOLUME_MUTE))
                            }
                        }
                    }
                    2 -> { // BROWSER
                        DeckRow {
                            DeckToolButton("New Tab", Icons.Default.Add) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_NEW_TAB))
                            }
                            DeckToolButton("Close Tab", Icons.Default.Close) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_CLOSE_TAB))
                            }
                            DeckToolButton("Reopen Tab", Icons.Default.History) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_REOPEN_TAB))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Refresh", Icons.Default.Refresh) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_REFRESH))
                            }
                            DeckToolButton("Back", Icons.Default.ArrowBack) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_BACK))
                            }
                            DeckToolButton("Forward", Icons.Default.ArrowForward) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.BROWSER_FORWARD))
                            }
                        }
                    }
                    3 -> { // EDIT
                        DeckRow {
                            DeckToolButton("Paste Clip", Icons.Default.ContentPaste) {
                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                                if (clipboard != null && clipboard.hasPrimaryClip()) {
                                    val clip = clipboard.primaryClip
                                    if (clip != null && clip.itemCount > 0) {
                                        val text = clip.getItemAt(0).coerceToText(context).toString()
                                        if (text.isNotEmpty()) {
                                            onInputEvent(InputEvent.ClipboardSync(text))
                                            Toast.makeText(context, "Copied ${text.length} chars to PC!", Toast.LENGTH_SHORT).show()
                                        }
                                    }
                                }
                            }
                            DeckToolButton("Copy (Ctrl+C)", Icons.Default.ContentCopy) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.EDIT_COPY))
                            }
                            DeckToolButton("Paste (Ctrl+V)", Icons.Default.Assignment) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.EDIT_PASTE))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Cut (Ctrl+X)", Icons.Default.ContentCut) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.EDIT_CUT))
                            }
                            DeckToolButton("Select All", Icons.Default.SelectAll) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.EDIT_SELECT_ALL))
                            }
                            DeckToolButton("Undo", Icons.Default.Undo) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.EDIT_UNDO))
                            }
                        }
                    }
                    4 -> { // LAUNCH
                        DeckRow {
                            DeckToolButton("Explorer", Icons.Default.Folder) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.FILE_EXPLORER))
                            }
                            DeckToolButton("Run Prompt", Icons.Default.Terminal) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.RUN_DIALOG))
                            }
                            DeckToolButton("Search", Icons.Default.Search) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.WINDOWS_SEARCH))
                            }
                        }
                        DeckRow {
                            DeckToolButton("Escape", Icons.Default.FullscreenExit) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.KEY_ESCAPE))
                            }
                            DeckToolButton("Enter", Icons.Default.KeyboardReturn) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.KEY_ENTER))
                            }
                            DeckToolButton("Action Center", Icons.Default.Notifications) {
                                onInputEvent(InputEvent.SystemCommand(InputEvent.SystemCommand.ACTION_CENTER))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DeckRow(content: @Composable RowScope.() -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        content = content
    )
}

@Composable
private fun RowScope.DeckToolButton(
    title: String,
    icon: ImageVector,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = SurfaceCard,
        shape = RoundedCornerShape(8.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, SurfaceCardBorder),
        modifier = Modifier.weight(1f)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center
        ) {
            Icon(
                imageVector = icon,
                contentDescription = title,
                tint = Color.White,
                modifier = Modifier.size(16.dp)
            )
            Spacer(modifier = Modifier.width(6.dp))
            Text(
                text = title,
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                color = Color.White,
                maxLines = 1
            )
        }
    }
}
