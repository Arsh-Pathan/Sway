package com.sway.app.ui.components

import androidx.compose.foundation.background
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sway.app.model.InputEvent
import com.sway.app.ui.screens.ViewFitMode
import com.sway.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ResolutionSheet(
    currentResMode: Byte,
    currentFitMode: ViewFitMode,
    isVerticalPc: Boolean,
    onSelectRes: (Byte) -> Unit,
    onSelectFit: (ViewFitMode) -> Unit,
    onToggleVerticalPc: () -> Unit,
    onDismiss: () -> Unit
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
                        imageVector = Icons.Default.AspectRatio,
                        contentDescription = null,
                        tint = AccentIndigoBright,
                        modifier = Modifier.size(24.dp)
                    )
                    Text(
                        text = "Display Resolution & Scaling",
                        style = Typography.headlineMedium,
                        color = TextPrimary
                    )
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Default.Close, contentDescription = "Close", tint = TextMuted)
                }
            }

            Text(
                text = "Adjust Windows host display resolution and phone screen fit modes for the best remote experience.",
                style = Typography.bodyMedium,
                color = TextSecondary
            )

            // Section 1: Phone Screen Fit Mode
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "Phone Screen Fit Mode",
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextPrimary
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    FitModeChip(
                        title = "Proportional",
                        desc = "Fit with black bars",
                        isSelected = currentFitMode == ViewFitMode.FIT,
                        modifier = Modifier.weight(1f),
                        onClick = { onSelectFit(ViewFitMode.FIT) }
                    )
                    FitModeChip(
                        title = "Full Fill",
                        desc = "Zero black bars",
                        isSelected = currentFitMode == ViewFitMode.FILL,
                        modifier = Modifier.weight(1f),
                        onClick = { onSelectFit(ViewFitMode.FILL) }
                    )
                    FitModeChip(
                        title = "Rotate 90°",
                        desc = "Full horizontal",
                        isSelected = currentFitMode == ViewFitMode.ROTATE_90,
                        modifier = Modifier.weight(1f),
                        onClick = { onSelectFit(ViewFitMode.ROTATE_90) }
                    )
                }
            }

            // Section 2: Windows Host Resolution
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "Windows Desktop Resolution",
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextPrimary
                )

                ResolutionRow(
                    title = "📱 Mobile 720p (1280 × 720)",
                    subtitle = "Large readable text & icons, optimized for phone touch",
                    isSelected = currentResMode == InputEvent.ChangeResolution.MOBILE_720P,
                    onClick = { onSelectRes(InputEvent.ChangeResolution.MOBILE_720P) }
                )

                ResolutionRow(
                    title = "⚖️ HD+ Balanced (1600 × 900)",
                    subtitle = "Crisp clarity and comfortable touch targets",
                    isSelected = currentResMode == InputEvent.ChangeResolution.BALANCED_900P,
                    onClick = { onSelectRes(InputEvent.ChangeResolution.BALANCED_900P) }
                )

                ResolutionRow(
                    title = "🖥️ Full HD (1920 × 1080)",
                    subtitle = "Native high-definition desktop rendering",
                    isSelected = currentResMode == InputEvent.ChangeResolution.DESKTOP_1080P,
                    onClick = { onSelectRes(InputEvent.ChangeResolution.DESKTOP_1080P) }
                )

                ResolutionRow(
                    title = "📲 Fit Vertical PC (1080 × 1920)",
                    subtitle = "Rotates Windows to match vertical phone orientation",
                    isSelected = isVerticalPc || currentResMode == InputEvent.ChangeResolution.VERTICAL_1080P,
                    onClick = {
                        onToggleVerticalPc()
                        onSelectRes(InputEvent.ChangeResolution.VERTICAL_1080P)
                    }
                )

                ResolutionRow(
                    title = "🔄 Reset Native Default",
                    subtitle = "Restores standard Windows desktop resolution",
                    isSelected = currentResMode == InputEvent.ChangeResolution.DEFAULT,
                    onClick = { onSelectRes(InputEvent.ChangeResolution.DEFAULT) }
                )
            }

            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

@Composable
private fun FitModeChip(
    title: String,
    desc: String,
    isSelected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(if (isSelected) AccentIndigo else SurfaceCard)
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp, horizontal = 6.dp),
        contentAlignment = Alignment.Center
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = title,
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold,
                color = if (isSelected) Color.White else TextPrimary
            )
            Text(
                text = desc,
                fontSize = 10.sp,
                color = if (isSelected) Color.White.copy(alpha = 0.85f) else TextSecondary
            )
        }
    }
}

@Composable
private fun ResolutionRow(
    title: String,
    subtitle: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        onClick = onClick,
        color = if (isSelected) AccentIndigoGlow else SurfaceCard,
        shape = RoundedCornerShape(12.dp),
        border = if (isSelected) androidx.compose.foundation.BorderStroke(1.5.dp, AccentIndigo) else null,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (isSelected) AccentIndigoBright else TextPrimary
                )
                Text(
                    text = subtitle,
                    fontSize = 11.sp,
                    color = TextSecondary
                )
            }
            if (isSelected) {
                Icon(
                    imageVector = Icons.Default.Check,
                    contentDescription = "Selected",
                    tint = AccentIndigoBright,
                    modifier = Modifier.size(18.dp)
                )
            }
        }
    }
}
