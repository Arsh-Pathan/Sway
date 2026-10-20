# Sway Low-Latency Input Protocol Specification

Sway utilizes an unordered, unreliable WebRTC Data Channel named **`sway-input`** (`ordered: false, maxRetransmits: 0`) to achieve real-time, zero-delay input streaming.

Unreliable transmission prevents TCP-style head-of-line blocking: if an intermediate cursor delta packet is dropped in transit, subsequent packets are applied immediately without stutter or queuing lag.

---

## 1. Binary Packet Header & OpCodes

Every packet begins with a single unsigned byte (`uint8`) representing the **OpCode**, followed by compact little-endian payload bytes:

| OpCode | Name | Payload Structure | Total Bytes | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x01` | **MouseMove** | `dx: int16`, `dy: int16` | 5 | Relative mouse displacement (trackpad drag) |
| `0x02` | **MouseDown** | `button: uint8` | 2 | Mouse press (1: Left, 2: Right, 3: Middle) |
| `0x03` | **MouseUp** | `button: uint8` | 2 | Mouse release (1: Left, 2: Right, 3: Middle) |
| `0x04` | **MouseScroll** | `delta_y: int16`, `delta_x: int16` | 5 | Scroll wheel clicks (vertical & horizontal) |
| `0x05` | **KeyDown** | `vkey: uint16`, `modifiers: uint8` | 4 | Key press (Win32 Virtual Key code) |
| `0x06` | **KeyUp** | `vkey: uint16` | 3 | Key release |
| `0x07` | **TextInput** | `length: uint16`, `utf8_bytes...` | 3 + N | Direct Unicode character injection |
| `0x08` | **SwitchMonitor** | `monitor_index: uint8` | 2 | Switch captured screen display |
| `0x09` | **SetQuality** | `mode: uint8` | 2 | Quality mode (1: DataSaver, 2: Balanced, 3: Native) |

---

## 2. Modifier Bitmask (`0x05 KeyDown`)

When sending a key down event, the `modifiers` byte communicates active modifier keys:

```
Bit 0 (0x01): Control (Ctrl)
Bit 1 (0x02): Shift
Bit 2 (0x04): Alt
Bit 3 (0x08): Windows Key (Super / Meta)
Bit 4 (0x10): Caps Lock
```

---

## 3. Windows Virtual Key (VK) Reference

Standard Windows Virtual Key codes used in `KeyDown` and `KeyUp`:
- `VK_RETURN` (`0x0D`): Enter
- `VK_ESCAPE` (`0x1B`): Escape
- `VK_BACK` (`0x08`): Backspace
- `VK_TAB` (`0x09`): Tab
- `VK_SPACE` (`0x20`): Spacebar
- `VK_LEFT` (`0x25`): Left Arrow
- `VK_UP` (`0x26`): Up Arrow
- `VK_RIGHT` (`0x27`): Right Arrow
- `VK_DOWN` (`0x28`): Down Arrow
- `VK_MEDIA_PLAY_PAUSE` (`0xB3`): Media Play/Pause
- `VK_VOLUME_MUTE` (`0xAD`): Volume Mute
- `VK_VOLUME_DOWN` (`0xAE`): Volume Down
- `VK_VOLUME_UP` (`0xAF`): Volume Up
