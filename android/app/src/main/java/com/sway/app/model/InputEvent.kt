package com.sway.app.model

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Binary input serialization for ultra-low latency (<5ms) transmission
 * across the WebRTC unreliable DataChannel ('sway-input').
 */
sealed class InputEvent {
    abstract val opCode: Byte
    abstract fun toByteArray(): ByteArray

    data class MouseMove(val dx: Short, val dy: Short) : InputEvent() {
        override val opCode: Byte = 0x01
        override fun toByteArray(): ByteArray {
            return ByteBuffer.allocate(5)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(dx)
                .putShort(dy)
                .array()
        }
    }

    data class MouseDown(val button: Byte) : InputEvent() {
        override val opCode: Byte = 0x02
        override fun toByteArray(): ByteArray {
            return byteArrayOf(opCode, button)
        }
        companion object {
            const val BUTTON_LEFT: Byte = 1
            const val BUTTON_RIGHT: Byte = 2
            const val BUTTON_MIDDLE: Byte = 3
        }
    }

    data class MouseUp(val button: Byte) : InputEvent() {
        override val opCode: Byte = 0x03
        override fun toByteArray(): ByteArray {
            return byteArrayOf(opCode, button)
        }
    }

    data class MouseScroll(val deltaY: Short, val deltaX: Short = 0) : InputEvent() {
        override val opCode: Byte = 0x04
        override fun toByteArray(): ByteArray {
            return ByteBuffer.allocate(5)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(deltaY)
                .putShort(deltaX)
                .array()
        }
    }

    data class KeyDown(val vkey: Short, val modifiers: Byte = 0) : InputEvent() {
        override val opCode: Byte = 0x05
        override fun toByteArray(): ByteArray {
            return ByteBuffer.allocate(4)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(vkey)
                .put(modifiers)
                .array()
        }
    }

    data class KeyUp(val vkey: Short) : InputEvent() {
        override val opCode: Byte = 0x06
        override fun toByteArray(): ByteArray {
            return ByteBuffer.allocate(3)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(vkey)
                .array()
        }
    }

    data class TextInput(val text: String) : InputEvent() {
        override val opCode: Byte = 0x07
        override fun toByteArray(): ByteArray {
            val bytes = text.toByteArray(Charsets.UTF_8)
            return ByteBuffer.allocate(3 + bytes.size)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(bytes.size.toShort())
                .put(bytes)
                .array()
        }
    }

    data class SwitchMonitor(val monitorIndex: Byte) : InputEvent() {
        override val opCode: Byte = 0x08
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, monitorIndex)
    }

    data class SetQuality(val qualityMode: Byte) : InputEvent() {
        override val opCode: Byte = 0x09
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, qualityMode)
        companion object {
            const val DATA_SAVER: Byte = 1
            const val BALANCED: Byte = 2
            const val NATIVE: Byte = 3
        }
    }
}
