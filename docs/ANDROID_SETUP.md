# Sway Android Client Documentation & Setup

The **Sway Android Client** is built using native **Kotlin** and **Jetpack Compose (Material 3)**. It leverages the modern Google WebRTC distribution (`io.getstream:stream-webrtc-android`) for low-latency streaming and CameraX + ML Kit for seamless QR code pairing.

---

## 1. Prerequisites

- **Android Studio Ladybug (2024.2+)** or command-line Gradle.
- **Android SDK Platform 35** and Build-Tools.
- **Java Development Kit (JDK 17+)** (such as Amazon Corretto 25, installed on this machine).
- A physical Android device or emulator running **Android 8.0 (API level 26)** or newer.

---

## 2. Project Architecture

```
android/app/src/main/java/com/sway/app/
├── MainActivity.kt          # Main lifecycle coordinator & session manager
├── SwayApplication.kt      # Application base class
├── model/
│   ├── Device.kt           # Paired host persistence model
│   ├── InputEvent.kt       # Binary low-latency serialization engine
│   └── SignalingPayloads.kt# JSON schemas for WebSocket signaling
├── network/
│   └── SignalingClient.kt  # OkHttp WebSocket client with auto-reconnect & heartbeats
├── webrtc/
│   └── WebRtcManager.kt    # PeerConnectionFactory, DataChannel ('sway-input'), media sink
└── ui/
    ├── theme/              # OLED dark theme, indigo accent palette, typography
    ├── components/
    │   ├── ModifierDock.kt # Sticky modifiers (Ctrl/Alt/Win/Shift), Esc, Tab, Arrows
    │   ├── SettingsSheet.kt# Sensitivity & scroll sliders, haptics, quality picker
    │   └── TrackpadSurface.kt # Multi-touch gesture surface & touch ripple canvas
    └── screens/
        ├── PairingScreen.kt# CameraX QR scanner & manual connection tabs
        └── SessionScreen.kt# Adaptive portrait / landscape remote desktop viewport
```

---

## 3. Gesture Controls & Key Mapping

| Gesture | Action |
| :--- | :--- |
| **1-finger Drag** | Smooth mouse cursor movement with velocity scaling |
| **1-finger Tap** | Left mouse click (with haptic feedback) |
| **1-finger Double-Tap** | Double click |
| **Long Press** | Right mouse click |
| **2-finger Drag** | Vertical and horizontal wheel scrolling |
| **Modifier Bar** | Sticky toggle for `Ctrl`, `Alt`, `Shift`, `Win` |
| **Keyboard Icon** | Opens Android soft keyboard with direct text streaming |

---

## 4. Building the App

Open the `android/` directory in Android Studio, or compile directly from the terminal:

```bash
cd android
./gradlew assembleDebug
```

To install on a connected physical device:
```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
```
