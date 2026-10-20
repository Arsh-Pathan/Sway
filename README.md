# Sway

> Ultra-low latency remote control & desktop streaming for Windows from Android over WebRTC.

## Overview

**Sway** is an end-to-end remote control system that lets you control your Windows laptop from your Android device from anywhere in the world. It combines low-latency video and audio streaming with a touch-friendly virtual trackpad, multi-touch gestures, and keyboard input simulation.

Detailed architecture specifications are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Architecture at a Glance

- **Windows Host Daemon (`host/`)**: Built in **Rust** for minimal CPU/RAM overhead and native performance.
  - Screen capture via Windows Graphics Capture / DXGI Desktop Duplication API.
  - Audio capture via Windows Core Audio (WASAPI loopback).
  - Native mouse & keyboard injection via Win32 `SendInput`.
  - Minimal system tray icon with one-click QR code pairing.
- **Android Mobile App (`android/`)**: Native **Kotlin + Jetpack Compose**.
  - Google WebRTC Android SDK for low-latency streaming.
  - Virtual trackpad surface supporting multi-touch gestures (two-finger scroll, right click, drag-and-drop).
  - Modifier bar dock (`Ctrl`, `Alt`, `Win`, `Shift`, `Esc`, `Tab`, arrows) and soft keyboard.
  - CameraX scanner for quick QR code pairing.
- **Signaling & Relay Service (`signaling/`)**: Containerized service for your **VPS**.
  - Lightweight WebSocket signaling server.
  - Bundled Docker Compose configuration with Coturn STUN/TURN for guaranteed NAT traversal.

---

## Repository Structure

```
Sway/
├── android/               # Kotlin + Jetpack Compose Android Client
├── host/                  # Rust Windows Host Daemon
├── signaling/             # VPS Signaling Server & Coturn Relay
└── docs/                  # Architecture specs & protocol guides
```

---

## Status

Project initialized and architectural design resolved. Currently bootstrapping core monorepo modules.
