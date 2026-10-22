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
            var bytes = text.toByteArray(Charsets.UTF_8)
            if (bytes.size > 65535) {
                bytes = bytes.copyOf(65535)
            }
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
            const val TURBO: Byte = 0
            const val DATA_SAVER: Byte = 1
            const val BALANCED: Byte = 2
            const val NATIVE: Byte = 3
        }
    }

    data class TouchDirect(val normX: Int, val normY: Int, val action: Byte) : InputEvent() {
        override val opCode: Byte = 0x0A
        override fun toByteArray(): ByteArray {
            return ByteBuffer.allocate(6)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(normX.toShort())
                .putShort(normY.toShort())
                .put(action)
                .array()
        }
        companion object {
            const val ACTION_DOWN: Byte = 0
            const val ACTION_MOVE: Byte = 1
            const val ACTION_UP: Byte = 2
        }
    }

    data class SetPrivacyCurtain(val enabled: Boolean) : InputEvent() {
        override val opCode: Byte = 0x0B
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, if (enabled) 1 else 0)
    }

    data class ChangeResolution(val mode: Byte) : InputEvent() {
        override val opCode: Byte = 0x0C
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, mode)
        companion object {
            const val DEFAULT: Byte = 0
            const val MOBILE_720P: Byte = 1
            const val DESKTOP_1080P: Byte = 2
            const val BALANCED_900P: Byte = 3
            const val COMPACT_1024: Byte = 4
            const val QHD_1440P: Byte = 5
            const val VERTICAL_1080P: Byte = 10
            const val VERTICAL_720P: Byte = 11
        }
    }

    data class ChangeOrientation(val vertical: Boolean) : InputEvent() {
        override val opCode: Byte = 0x0D
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, if (vertical) 1 else 0)
    }

    data class ClipboardSync(val text: String) : InputEvent() {
        override val opCode: Byte = 0x0E
        override fun toByteArray(): ByteArray {
            var bytes = text.toByteArray(Charsets.UTF_8)
            if (bytes.size > 65535) {
                bytes = bytes.copyOf(65535)
            }
            return ByteBuffer.allocate(3 + bytes.size)
                .order(ByteOrder.LITTLE_ENDIAN)
                .put(opCode)
                .putShort(bytes.size.toShort())
                .put(bytes)
                .array()
        }
    }

    data class SystemCommand(val action: Byte) : InputEvent() {
        override val opCode: Byte = 0x0F
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, action)
        companion object {
            const val LOCK: Byte = 1
            const val SHOW_DESKTOP: Byte = 2
            const val TASK_MANAGER: Byte = 3
            const val SLEEP: Byte = 4
            const val TURN_OFF_SCREEN: Byte = 5
            const val VOLUME_UP: Byte = 6
            const val VOLUME_DOWN: Byte = 7
            const val VOLUME_MUTE: Byte = 8
            const val MEDIA_PLAY_PAUSE: Byte = 9
            const val MEDIA_NEXT: Byte = 10
            const val MEDIA_PREV: Byte = 11
            const val ALT_TAB: Byte = 12
            const val WIN_TAB: Byte = 13
            const val CLOSE_WINDOW: Byte = 14
            const val SNIPPING_TOOL: Byte = 15
            const val FILE_EXPLORER: Byte = 16
            const val RUN_DIALOG: Byte = 17
            const val MAXIMIZE_WINDOW: Byte = 18
            const val MINIMIZE_WINDOW: Byte = 19
            const val SNAP_LEFT: Byte = 20
            const val SNAP_RIGHT: Byte = 21
            const val BROWSER_NEW_TAB: Byte = 22
            const val BROWSER_CLOSE_TAB: Byte = 23
            const val BROWSER_REOPEN_TAB: Byte = 24
            const val BROWSER_REFRESH: Byte = 25
            const val BROWSER_BACK: Byte = 26
            const val BROWSER_FORWARD: Byte = 27
            const val EDIT_SELECT_ALL: Byte = 28
            const val EDIT_COPY: Byte = 29
            const val EDIT_PASTE: Byte = 30
            const val EDIT_CUT: Byte = 31
            const val EDIT_UNDO: Byte = 32
            const val EDIT_REDO: Byte = 33
            const val WINDOWS_SEARCH: Byte = 34
            const val ACTION_CENTER: Byte = 35
            const val KEY_ESCAPE: Byte = 36
            const val KEY_ENTER: Byte = 37
            const val KEY_BACKSPACE: Byte = 38
            const val KEY_TAB: Byte = 39
        }
    }

    data class SetStreaming(val enabled: Boolean) : InputEvent() {
        override val opCode: Byte = 0x10
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, if (enabled) 1 else 0)
    }

    data class SetAudioConfig(val speakerEnabled: Boolean, val micEnabled: Boolean) : InputEvent() {
        override val opCode: Byte = 0x11
        override fun toByteArray(): ByteArray = byteArrayOf(opCode, if (speakerEnabled) 1 else 0, if (micEnabled) 1 else 0)
    }
}
