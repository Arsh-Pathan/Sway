package com.sway.app.ui.components

import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
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
            .pointerInput(sensitivity, scrollSpeed, hapticsEnabled) {
                var lastTapUpTime = 0L
                var isDragHolding = false
                var accumX = 0f
                var accumY = 0f
                var accumScrollX = 0f
                var accumScrollY = 0f

                awaitEachGesture {
                    val firstDown = awaitFirstDown()
                    val startTime = System.currentTimeMillis()
                    val timeSinceLastTap = startTime - lastTapUpTime

                    // Double-tap and hold gesture: initiates drag-select / window drag
                    if (timeSinceLastTap < 350) {
                        isDragHolding = true
                        triggerHaptic(2)
                        onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                    } else {
                        isDragHolding = false
                    }

                    var maxPointers = 1
                    var hasMoved = false
                    var totalDragX = 0f
                    var totalDragY = 0f

                    while (true) {
                        val event = awaitPointerEvent()
                        val activePointers = event.changes.filter { it.pressed }
                        if (activePointers.size > maxPointers) {
                            maxPointers = activePointers.size
                        }

                        if (activePointers.isEmpty()) {
                            // Gesture ended (all fingers lifted)
                            val duration = System.currentTimeMillis() - startTime
                            if (isDragHolding) {
                                // Release the held drag
                                onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                isDragHolding = false
                                lastTapUpTime = 0L
                            } else if (!hasMoved && duration < 380) {
                                if (maxPointers == 1) {
                                    // 1-finger tap -> Left Click
                                    triggerHaptic(1)
                                    onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                                    onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                    lastTapUpTime = System.currentTimeMillis()
                                } else if (maxPointers >= 2) {
                                    // 2-finger tap -> Right Click!
                                    triggerHaptic(2)
                                    onInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_RIGHT))
                                    onInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_RIGHT))
                                    lastTapUpTime = 0L
                                }
                            } else {
                                lastTapUpTime = 0L
                            }
                            touchPoint = null
                            isDragging = false
                            accumX = 0f
                            accumY = 0f
                            accumScrollX = 0f
                            accumScrollY = 0f
                            break
                        }

                        touchPoint = activePointers.first().position
                        isDragging = true

                        if (activePointers.size == 1) {
                            val change = activePointers.first()
                            val dragX = change.position.x - change.previousPosition.x
                            val dragY = change.position.y - change.previousPosition.y
                            totalDragX += Math.abs(dragX)
                            totalDragY += Math.abs(dragY)
                            if (totalDragX > 16f || totalDragY > 16f) {
                                hasMoved = true
                            }

                            // Subpixel fractional movement accumulation for ultra-smooth gliding
                            accumX += dragX * sensitivity
                            accumY += dragY * sensitivity
                            val sendX = accumX.toInt()
                            val sendY = accumY.toInt()
                            accumX -= sendX
                            accumY -= sendY

                            if (sendX != 0 || sendY != 0) {
                                val dx = sendX.coerceIn(-32767, 32767).toShort()
                                val dy = sendY.coerceIn(-32767, 32767).toShort()
                                onInputEvent(InputEvent.MouseMove(dx, dy))
                            }
                            change.consume()
                        } else if (activePointers.size >= 2) {
                            // 2-finger scroll
                            val p0 = activePointers[0]
                            val p1 = activePointers[1]
                            val dragY = ((p0.position.y - p0.previousPosition.y) + (p1.position.y - p1.previousPosition.y)) / 2f
                            val dragX = ((p0.position.x - p0.previousPosition.x) + (p1.position.x - p1.previousPosition.x)) / 2f
                            totalDragX += Math.abs(dragX)
                            totalDragY += Math.abs(dragY)
                            if (totalDragX > 16f || totalDragY > 16f) {
                                hasMoved = true
                            }

                            // Smooth wheel scroll accumulation
                            accumScrollY += -dragY * scrollSpeed * 25f
                            accumScrollX += dragX * scrollSpeed * 25f
                            val sendScrollY = accumScrollY.toInt()
                            val sendScrollX = accumScrollX.toInt()
                            accumScrollY -= sendScrollY
                            accumScrollX -= sendScrollX

                            if (sendScrollY != 0 || sendScrollX != 0) {
                                val sY = sendScrollY.coerceIn(-32767, 32767).toShort()
                                val sX = sendScrollX.coerceIn(-32767, 32767).toShort()
                                onInputEvent(InputEvent.MouseScroll(sY, sX))
                            }
                            for (p in activePointers) p.consume()
                        }
                    }
                }
            }
    )
}
