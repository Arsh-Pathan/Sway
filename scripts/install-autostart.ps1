# PowerShell script to install Sway Desktop Host as a silent Windows Autostart background service

[CmdletBinding()]
param(
    [string]$SignalingUrl = "51.79.249.143:8080",
    [string]$DeviceId = "",
    [string]$Token = ""
)

function New-SecureToken {
    $chars = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789"
    $bytes = New-Object byte[] 16
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $rng.GetBytes($bytes)
    $rng.Dispose()
    return -join ($bytes | ForEach-Object { $chars[$_ % $chars.Length] })
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

$CargoBin = "$env:USERPROFILE\.cargo\bin"
if (Test-Path $CargoBin) {
    $env:PATH = "$CargoBin;$env:PATH"
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

# Stop running instances BEFORE copying to avoid Windows file-in-use lock errors
Stop-Process -Name "sway-host" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 600

$TargetExe = "$AppDir\sway-host.exe"
Copy-Item -Path $SourceExe -Destination $TargetExe -Force
Write-Host "[+] Copied host binary to $TargetExe" -ForegroundColor Green

# Save or update BOM-less UTF-8 config.json (PowerShell Set-Content UTF8 adds a BOM that breaks JSON parsers)
$ConfigPath = "$AppDir\config.json"
$Config = $null
if (Test-Path $ConfigPath) {
    try {
        $raw = [System.IO.File]::ReadAllText($ConfigPath).TrimStart([char]0xFEFF)
        $Config = $raw | ConvertFrom-Json
    } catch {
        $Config = $null
    }
}

$NeedWrite = ($null -eq $Config) -or
    $PSBoundParameters.ContainsKey('SignalingUrl') -or
    $PSBoundParameters.ContainsKey('DeviceId') -or
    $PSBoundParameters.ContainsKey('Token')

if ($NeedWrite) {
    $FinalUrl = if ($PSBoundParameters.ContainsKey('SignalingUrl')) { $SignalingUrl } elseif ($Config -and $Config.signaling_url) { $Config.signaling_url } else { $SignalingUrl }
    $FinalId = if ($PSBoundParameters.ContainsKey('DeviceId') -and -not [string]::IsNullOrWhiteSpace($DeviceId)) { $DeviceId } elseif ($Config -and $Config.device_id) { $Config.device_id } else { "sway-host-$((New-Guid).ToString().Substring(0,8))" }
    $FinalToken = if ($PSBoundParameters.ContainsKey('Token') -and -not [string]::IsNullOrWhiteSpace($Token)) { $Token } elseif ($Config -and $Config.token) { $Config.token } else { New-SecureToken }
    $FinalPort = if ($Config -and $Config.local_port) { [int]$Config.local_port } else { 8089 }

    $ConfigJson = [ordered]@{
        signaling_url = $FinalUrl
        device_id     = $FinalId
        token         = $FinalToken
        local_port    = $FinalPort
    } | ConvertTo-Json -Compress
    $Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($ConfigPath, $ConfigJson, $Utf8NoBom)
    Write-Host "[+] Wrote BOM-less configuration: $ConfigPath" -ForegroundColor Green
}

# 1. Clean up any redundant Registry Run key to avoid dual-launch and heuristic false-positives
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Remove-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -ErrorAction SilentlyContinue

# 2. Register in Windows Startup folder (shell:startup) for persistent boot autostart
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

# Launch native background process detached via WMI Win32_Process
Write-Host "[*] Launching Sway Host 24/7 silent background service..." -ForegroundColor Yellow
$launchRes = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
    CommandLine = "`"$TargetExe`""
    CurrentDirectory = $AppDir
}
if ($launchRes.ReturnValue -ne 0) {
    Write-Error "Failed to launch sway-host.exe via WMI (ReturnValue=$($launchRes.ReturnValue))."
    exit 1
}
Start-Sleep -Seconds 2
$proc = Get-Process -Id $launchRes.ProcessId -ErrorAction SilentlyContinue
if (-not $proc) {
    Write-Error "sway-host.exe exited immediately after launch."
    exit 1
}
Write-Host "[+] Sway Host running silently in background (PID: $($proc.Id))" -ForegroundColor Green

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Sway Desktop Host Autostart Installed! " -ForegroundColor Green
Write-Host "  - Host will start automatically on Windows boot"
Write-Host "  - Web Dashboard: http://localhost:8089"
Write-Host "  - UDP Beacon Broadcast: Port 8088"
Write-Host "========================================" -ForegroundColor Cyan
