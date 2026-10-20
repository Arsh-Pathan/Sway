package com.sway.app.ui.components

import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.sway.app.model.InputEvent
import com.sway.app.ui.theme.*

@Composable
fun TrackpadSurface(
    modifier: Modifier = Modifier,
    sensitivity: Float = 1.0f,
    scrollSpeed: Float = 1.0f,
    hapticsEnabled: Boolean = true,
    onInputEvent: (InputEvent) -> Unit
) {
    val context = LocalContext.current
    val vibrator = remember {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            context.getSystemService(Vibrator::class.java)
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(android.content.Context.VIBRATOR_SERVICE) as? Vibrator
        }
    }

    var touchPoint by remember { mutableStateOf<Offset?>(null) }
    var isDragging by remember { mutableStateOf(false) }

    fun triggerHaptic(type: Int) {
        if (!hapticsEnabled || vibrator == null || !vibrator.hasVibrator()) return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val effect = when (type) {
                1 -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
                2 -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_HEAVY_CLICK)
                else -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
            }
            vibrator.vibrate(effect)
        } else {
            @Suppress("DEPRECATION")
            vibrator.vibrate(15)
        }
    }

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(20.dp))
            .background(TrackpadFill)
            .border(
                width = 1.5.dp,
                color = if (isDragging) TrackpadActiveBorder else TrackpadBorder,
                shape = RoundedCornerShape(20.dp)
            )
            .pointerInput(sensitivity, hapticsEnabled) {
                detectTapGestures(
                    onTap = {
                        // 1-finger single tap -> Left click
                        triggerHaptic(1)
                        onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                        onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                    },
                    onDoubleTap = {
                        // Double tap -> Double click
                        triggerHaptic(2)
                        onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                        onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                        onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                        onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                    },
                    onLongPress = {
                        // Long press -> Right click
                        triggerHaptic(2)
                        onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_RIGHT))
                        onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_RIGHT))
                    }
                )
            }
            .pointerInput(sensitivity, scrollSpeed) {
                detectDragGestures(
                    onDragStart = { offset ->
                        touchPoint = offset
                        isDragging = true
                    },
                    onDragEnd = {
                        touchPoint = null
                        isDragging = false
                    },
                    onDragCancel = {
                        touchPoint = null
                        isDragging = false
                    },
                    onDrag = { change, dragAmount ->
                        change.consume()
                        touchPoint = change.position

                        // Calculate scaled delta
                        val dx = (dragAmount.x * sensitivity).toInt().coerceIn(-32767, 32767).toShort()
                        val dy = (dragAmount.y * sensitivity).toInt().coerceIn(-32767, 32767).toShort()

                        if (dx != 0.toShort() || dy != 0.toShort()) {
                            onInputEvent(InputEvent.MouseMove(dx, dy))
                        }
                    }
                )
            }
    ) {
        // Visual touch indicator
        Canvas(modifier = Modifier.fillMaxSize()) {
            touchPoint?.let { pos ->
                drawCircle(
                    color = AccentIndigoBright.copy(alpha = 0.25f),
                    radius = 45f,
                    center = pos
                )
                drawCircle(
                    color = AccentIndigo,
                    radius = 16f,
                    center = pos
                )
            }
        }
    }
}
