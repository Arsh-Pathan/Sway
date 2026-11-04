# PowerShell script to install Sway Desktop Host as a silent Windows Autostart background service

[CmdletBinding()]
param(
    [string]$SignalingUrl = "51.79.249.143:8080",
    [string]$DeviceId = "",
    [string]$Token = ""
)

if ([string]::IsNullOrWhiteSpace($DeviceId)) {
    $DeviceId = "sway-host-$((New-Guid).ToString().Substring(0,8))"
}
if ([string]::IsNullOrWhiteSpace($Token)) {
    $Token = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 16 | ForEach-Object {[char]$_})
}

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

# Save or update config.json in application directory
$ConfigPath = "$AppDir\config.json"
if (-not (Test-Path $ConfigPath)) {
    $ConfigJson = @{
        signaling_url = $SignalingUrl
        device_id = $DeviceId
        token = $Token
        local_port = 8089
    } | ConvertTo-Json -Compress
    Set-Content -Path $ConfigPath -Value $ConfigJson -Encoding UTF8
    Write-Host "[+] Created configuration: $ConfigPath" -ForegroundColor Green
}

# 1. Register in Windows Registry Run key (starts automatically on user logon)
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$RegValue = "`"$TargetExe`""
Set-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -Value $RegValue
Write-Host "[+] Registered in Windows Registry Autostart: SwayRemoteHost -> $TargetExe" -ForegroundColor Green

# 2. Register in Windows Startup folder (shell:startup) for dual-layer boot persistence
$StartupFolder = [System.Environment]::GetFolderPath('Startup')
$ShortcutPath = "$StartupFolder\SwayHost.lnk"
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $TargetExe
$Shortcut.WorkingDirectory = $AppDir
$Shortcut.WindowStyle = 7 # Minimized/Hidden
$Shortcut.Description = "Sway Remote Host 24/7 Background Service"
$Shortcut.Save()
Write-Host "[+] Registered in Windows Startup Folder: $ShortcutPath" -ForegroundColor Green

# 3. Configure Windows Power Management so laptop never sleeps on AC power
try {
    powercfg /change standby-timeout-ac 0
    powercfg /change hibernate-timeout-ac 0
    Write-Host "[+] Configured Windows Power: Standby on AC = Never Sleep" -ForegroundColor Green
} catch {
    Write-Host "[!] Could not configure powercfg: $_" -ForegroundColor Yellow
}

# Stop any old instances before launching clean native background process
Stop-Process -Name "sway-host" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Launch native background process silently
Write-Host "[*] Launching Sway Host 24/7 silent background service..." -ForegroundColor Yellow
Start-Process -FilePath $TargetExe -WorkingDirectory $AppDir -WindowStyle Hidden
Start-Sleep -Seconds 2

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Sway Desktop Host Autostart Installed! " -ForegroundColor Green
Write-Host "  - Host will start automatically on Windows boot"
Write-Host "  - Web Dashboard: http://localhost:8089"
Write-Host "  - UDP Beacon Broadcast: Port 8088"
Write-Host "========================================" -ForegroundColor Cyan
