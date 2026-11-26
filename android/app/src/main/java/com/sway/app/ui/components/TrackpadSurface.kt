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
    isDeckMode: Boolean = false,
    isInvisible: Boolean = false,
    isDragLocked: Boolean = false,
    rotationQuarterTurns: Int = 0,
    onUnlockDrag: () -> Unit = {},
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

    var isDragging by remember { mutableStateOf(false) }
    val latestDragLocked by rememberUpdatedState(isDragLocked)
    val latestRotation by rememberUpdatedState(rotationQuarterTurns)
    val latestOnUnlockDrag by rememberUpdatedState(onUnlockDrag)
    val latestOnInputEvent by rememberUpdatedState(onInputEvent)

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
        modifier = if (isInvisible) {
            modifier
        } else {
            modifier
                .clip(RoundedCornerShape(20.dp))
                .background(TrackpadFill)
                .border(
                    width = 1.5.dp,
                    color = if (isDragging || isDragLocked) TrackpadActiveBorder else TrackpadBorder,
                    shape = RoundedCornerShape(20.dp)
                )
        }
            .pointerInput(sensitivity, scrollSpeed, hapticsEnabled, isDeckMode) {
                var lastTapUpTime = 0L
                var isDragHolding = false
                var accumX = 0f
                var accumY = 0f
                var accumScrollX = 0f
                var accumScrollY = 0f
                var prevFilteredX = 0f
                var prevFilteredY = 0f
                val densityScale = 3.0f / density.coerceAtLeast(1f)
                val moveSlopPx = 6f * density.coerceAtLeast(1f)

                awaitEachGesture {
                    awaitFirstDown()
                    val startTime = System.currentTimeMillis()
                    val timeSinceLastTap = startTime - lastTapUpTime
                    prevFilteredX = 0f
                    prevFilteredY = 0f

                    // Double-tap and hold gesture: initiates drag-select / window drag
                    if (!latestDragLocked && timeSinceLastTap < 350) {
                        isDragHolding = true
                        triggerHaptic(2)
                        latestOnInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                    } else {
                        isDragHolding = false
                    }

                    var maxPointers = 1
                    var hasMoved = false
                    var totalDragX = 0f
                    var totalDragY = 0f

                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            var pressedCount = 0
                            var p0: androidx.compose.ui.input.pointer.PointerInputChange? = null
                            var p1: androidx.compose.ui.input.pointer.PointerInputChange? = null
                            for (i in event.changes.indices) {
                                val c = event.changes[i]
                                if (c.pressed) {
                                    if (pressedCount == 0) p0 = c
                                    else if (pressedCount == 1) p1 = c
                                    pressedCount++
                                }
                            }
                            if (pressedCount > maxPointers) {
                                maxPointers = pressedCount
                            }

                            if (pressedCount == 0) {
                                // Gesture ended (all fingers lifted)
                                val duration = System.currentTimeMillis() - startTime
                                if (isDragHolding) {
                                    // Release the held drag
                                    latestOnInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                    isDragHolding = false
                                    lastTapUpTime = 0L
                                } else if (!hasMoved && duration < 380) {
                                    if (maxPointers == 1) {
                                        if (latestDragLocked) {
                                            // Single tap while drag-locked drops the item and clears lock
                                            triggerHaptic(1)
                                            latestOnInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                            latestOnUnlockDrag()
                                            lastTapUpTime = 0L
                                        } else {
                                            // 1-finger tap -> Left Click
                                            triggerHaptic(1)
                                            latestOnInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_LEFT))
                                            latestOnInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                                            lastTapUpTime = System.currentTimeMillis()
                                        }
                                    } else if (maxPointers >= 2) {
                                        // 2-finger tap -> Right Click
                                        triggerHaptic(2)
                                        latestOnInputEvent(InputEvent.MouseDown(InputEvent.MouseDown.BUTTON_RIGHT))
                                        latestOnInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_RIGHT))
                                        lastTapUpTime = 0L
                                    }
                                } else {
                                    lastTapUpTime = 0L
                                }
                                break
                            }

                            if (!isDragging) isDragging = true

                            if (pressedCount == 1 && p0 != null) {
                                val rawX = p0.position.x - p0.previousPosition.x
                                val rawY = p0.position.y - p0.previousPosition.y
                                val (dragX, dragY) = when (latestRotation and 3) {
                                    1 -> rawY to -rawX
                                    2 -> -rawX to -rawY
                                    3 -> -rawY to rawX
                                    else -> rawX to rawY
                                }
                                totalDragX += Math.abs(rawX)
                                totalDragY += Math.abs(rawY)
                                if (totalDragX > moveSlopPx || totalDragY > moveSlopPx) {
                                    hasMoved = true
                                }

                                val normX = dragX * densityScale
                                val normY = dragY * densityScale

                                // Anti-jitter low-pass exponential filter to eliminate digitizer stepping
                                val alpha = if (isDeckMode) 0.68f else 0.74f
                                val filteredDragX = if (hasMoved) alpha * normX + (1f - alpha) * prevFilteredX else normX
                                val filteredDragY = if (hasMoved) alpha * normY + (1f - alpha) * prevFilteredY else normY
                                prevFilteredX = filteredDragX
                                prevFilteredY = filteredDragY

                                // Precision Touchpad Ballistics: smooth subpixel gliding without teleport jumping
                                val dist = Math.hypot(filteredDragX.toDouble(), filteredDragY.toDouble()).toFloat()
                                val speedFactor = when {
                                    dist < 2.5f -> if (isDeckMode) 0.80f else 0.85f
                                    dist < 12.0f -> 1.0f + (dist - 2.5f) * 0.025f
                                    else -> 1.24f + (dist - 12.0f).coerceAtMost(22f) * 0.038f
                                }

                                val effectiveX = filteredDragX * sensitivity * speedFactor
                                val effectiveY = filteredDragY * sensitivity * speedFactor

                                // Subpixel fractional movement accumulation for ultra-smooth gliding
                                accumX += effectiveX
                                accumY += effectiveY
                                val sendX = accumX.toInt()
                                val sendY = accumY.toInt()
                                accumX -= sendX
                                accumY -= sendY

                                if (sendX != 0 || sendY != 0) {
                                    val dx = sendX.coerceIn(-32767, 32767).toShort()
                                    val dy = sendY.coerceIn(-32767, 32767).toShort()
                                    latestOnInputEvent(InputEvent.MouseMove(dx, dy))
                                }
                                p0.consume()
                            } else if (pressedCount >= 2 && p0 != null && p1 != null) {
                                // 2-finger scroll
                                val rawY = ((p0.position.y - p0.previousPosition.y) + (p1.position.y - p1.previousPosition.y)) / 2f
                                val rawX = ((p0.position.x - p0.previousPosition.x) + (p1.position.x - p1.previousPosition.x)) / 2f
                                val (dragX, dragY) = when (latestRotation and 3) {
                                    1 -> rawY to -rawX
                                    2 -> -rawX to -rawY
                                    3 -> -rawY to rawX
                                    else -> rawX to rawY
                                }
                                totalDragX += Math.abs(rawX)
                                totalDragY += Math.abs(rawY)
                                if (totalDragX > moveSlopPx || totalDragY > moveSlopPx) {
                                    hasMoved = true
                                }

                                // Density-normalized wheel scroll accumulation
                                accumScrollY += -(dragY / density.coerceAtLeast(1f)) * scrollSpeed * 8f
                                accumScrollX += (dragX / density.coerceAtLeast(1f)) * scrollSpeed * 8f
                                val sendScrollY = accumScrollY.toInt()
                                val sendScrollX = accumScrollX.toInt()
                                accumScrollY -= sendScrollY
                                accumScrollX -= sendScrollX

                                if (sendScrollY != 0 || sendScrollX != 0) {
                                    val sY = sendScrollY.coerceIn(-32767, 32767).toShort()
                                    val sX = sendScrollX.coerceIn(-32767, 32767).toShort()
                                    latestOnInputEvent(InputEvent.MouseScroll(sY, sX))
                                }
                                for (i in event.changes.indices) {
                                    if (event.changes[i].pressed) event.changes[i].consume()
                                }
                            }
                        }
                    } finally {
                        if (isDragHolding) {
                            latestOnInputEvent(InputEvent.MouseUp(InputEvent.MouseDown.BUTTON_LEFT))
                            isDragHolding = false
                        }
                        isDragging = false
                        accumX = 0f
                        accumY = 0f
                        accumScrollX = 0f
                        accumScrollY = 0f
                        prevFilteredX = 0f
                        prevFilteredY = 0f
                    }
                }
            }
    )
}
