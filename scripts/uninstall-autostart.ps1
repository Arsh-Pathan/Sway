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

# Remove registry autostart entry
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$entry = Get-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -ErrorAction SilentlyContinue
if ($entry) {
    Remove-ItemProperty -Path $RegKey -Name "SwayRemoteHost" -ErrorAction SilentlyContinue
    Write-Host "[+] Removed Windows Autostart registry key 'SwayRemoteHost'." -ForegroundColor Green
} else {
    Write-Host "[*] No Windows Autostart registry entry found." -ForegroundColor DarkGray
}

Write-Host "========================================" -ForegroundColor Yellow
Write-Host "  Sway Host Autostart Removed.          " -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Yellow
