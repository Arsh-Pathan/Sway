# PowerShell script to install Sway Desktop Host as a silent Windows Autostart background service

[CmdletBinding()]
param(
    [string]$SignalingUrl = "51.79.249.143:8080",
    [string]$DeviceId = "sway-laptop-arsh",
    [string]$Token = "swaypass123"
)

$ErrorActionPreference = "Stop"
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Sway Host Windows Autostart Setup     " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$AppDir = "$env:LOCALAPPDATA\Sway"
if (-not (Test-Path $AppDir)) {
    New-Item -ItemType Directory -Path $AppDir -Force | Out-Null
    Write-Host "[+] Created application directory: $AppDir" -ForegroundColor Green
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$SourceExe = "$RepoRoot\host\target\release\sway-host.exe"

if (-not (Test-Path $SourceExe)) {
    Write-Host "[*] sway-host.exe not found in target\release. Building release binary..." -ForegroundColor Yellow
    Push-Location "$RepoRoot\host"
    try {
        cargo build --release
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $SourceExe)) {
    Write-Error "Failed to locate sway-host.exe after build."
    exit 1
}

$TargetExe = "$AppDir\sway-host.exe"
Copy-Item -Path $SourceExe -Destination $TargetExe -Force
Write-Host "[+] Copied host binary to $TargetExe" -ForegroundColor Green

# Create the silent launcher VBS script
$VbsContent = @"
Set WshShell = CreateObject("WScript.Shell")
Set WshEnv = WshShell.Environment("PROCESS")
WshEnv("SWAY_SIGNALING_URL") = "$SignalingUrl"
WshEnv("SWAY_DEVICE_ID") = "$DeviceId"
WshEnv("SWAY_TOKEN") = "$Token"
WshShell.CurrentDirectory = "$AppDir"
WshShell.Run """$TargetExe""", 0, False
"@

$TargetVbs = "$AppDir\run-sway-hidden.vbs"
Set-Content -Path $TargetVbs -Value $VbsContent -Encoding ASCII
Write-Host "[+] Created silent background launcher: $TargetVbs" -ForegroundColor Green

# Register in Windows Run key (starts automatically on user logon)
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$RegValue = "wscript.exe `"$TargetVbs`""
Set-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -Value $RegValue
Write-Host "[+] Registered in Windows Autostart: SwayRemoteHost -> $RegValue" -ForegroundColor Green

# Launch right now if not already running
$running = Get-Process -Name "sway-host" -ErrorAction SilentlyContinue
if (-not $running) {
    Write-Host "[*] Launching Sway Host now in background..." -ForegroundColor Yellow
    Start-Process -FilePath "wscript.exe" -ArgumentList "`"$TargetVbs`""
    Start-Sleep -Seconds 1
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Sway Desktop Host Autostart Installed! " -ForegroundColor Green
Write-Host "  - Host will start automatically on Windows boot"
Write-Host "  - Web Dashboard: http://localhost:8089"
Write-Host "  - UDP Beacon Broadcast: Port 8088"
Write-Host "========================================" -ForegroundColor Cyan
