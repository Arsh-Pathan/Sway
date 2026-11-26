# PowerShell script to remove Sway Desktop Host Windows Autostart background service

Write-Host "========================================" -ForegroundColor Yellow
Write-Host "  Sway Host Autostart Uninstaller       " -ForegroundColor Yellow
Write-Host "========================================" -ForegroundColor Yellow

# Stop any running host processes
$procs = Get-Process -Name "sway-host" -ErrorAction SilentlyContinue
if ($procs) {
    Write-Host "[*] Stopping running sway-host processes..." -ForegroundColor Yellow
    $procs | Stop-Process -Force
    Write-Host "[+] Stopped sway-host processes." -ForegroundColor Green
}

# Remove Windows Startup folder shortcut
$StartupFolder = [System.Environment]::GetFolderPath('Startup')
$ShortcutPath = Join-Path $StartupFolder "SwayHost.lnk"
if (Test-Path $ShortcutPath) {
    Remove-Item -Path $ShortcutPath -Force -ErrorAction SilentlyContinue
    Write-Host "[+] Removed Windows Startup shortcut '$ShortcutPath'." -ForegroundColor Green
} else {
    Write-Host "[*] No Windows Startup shortcut found." -ForegroundColor DarkGray
}

# Remove legacy registry autostart entry if present
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$entry = Get-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -ErrorAction SilentlyContinue
if ($entry) {
    Remove-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -ErrorAction SilentlyContinue
    Write-Host "[+] Removed legacy Windows Autostart registry key 'SwayRemoteHost'." -ForegroundColor Green
}

Write-Host "[*] Note: Windows AC sleep timeout was previously set to Never Sleep (powercfg /change standby-timeout-ac 0)." -ForegroundColor DarkGray
Write-Host "========================================" -ForegroundColor Yellow
Write-Host "  Sway Host Autostart Removed.          " -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Yellow
