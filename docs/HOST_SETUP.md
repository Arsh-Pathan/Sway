# Sway Windows Host Daemon Setup Guide

The **Sway Host Daemon** runs on your Windows laptop. It captures the screen and audio, injects touch-trackpad, gestures, and keystrokes using Win32 `SendInput`, manages monitor switching, keeps the laptop awake during remote sessions, and connects to your VPS signaling server.

---

## 1. Prerequisites

- Windows 10 or Windows 11 (64-bit).
- Rust toolchain (Rust 1.80+ / Cargo).

---

## 2. Configuration Options

You can configure Sway Host using environment variables or command-line parameters:

| Variable | Default | Description |
| :--- | :--- | :--- |
| `SWAY_SIGNALING_URL` | `127.0.0.1:8080` | Address of your VPS signaling server (e.g., `sway.yourdomain.com`) |
| `SWAY_DEVICE_ID` | `sway-laptop-<random>` | Unique identifier for this machine |
| `SWAY_TOKEN` | `<random 12-char>` | Secret pairing token required for mobile connections |

---

## 3. Running Sway Host

From the repository root:

```powershell
cd host
cargo run --release
```

When started, Sway Host automatically:
1. Detects connected monitors and screen resolutions.
2. Generates an encrypted ASCII QR code directly inside your terminal window.
3. Connects to your signaling server.
4. Waits for your phone to scan the QR code.

---

## 4. Unattended Access & Startup

To run Sway automatically whenever you start your laptop:

### Method A: Windows Startup Folder
1. Press `Win + R`, type `shell:startup`, and press Enter.
2. Create a shortcut to `host/target/release/sway-host.exe`.

### Method B: Windows Service / Scheduled Task (Elevated)
To enable control on the Windows Lock Screen and UAC prompts:
```powershell
# Create an elevated scheduled task that starts at user logon
$action = New-ScheduledTaskAction -Execute "C:\Users\ArshPathan\Projects\Apps\Sway\host\target\release\sway-host.exe"
$trigger = New-ScheduledTaskTrigger -AtLogOn
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -RunLevel Highest
Register-ScheduledTask -TaskName "SwayHost" -Action $action -Trigger $trigger -Principal $principal
```
