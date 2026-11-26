$TargetExe = "$env:LOCALAPPDATA\Sway\sway-host.exe"
$AppDir = "$env:LOCALAPPDATA\Sway"

if (-not (Test-Path $TargetExe)) {
    Write-Error "sway-host.exe not found at $TargetExe. Run scripts\install-autostart.ps1 first."
    exit 1
}

# Terminate any existing instance cleanly
Stop-Process -Name "sway-host" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Launch sway-host detached under Win32_Process so it is completely independent of the launching shell
$res = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
    CommandLine = "`"$TargetExe`""
    CurrentDirectory = $AppDir
}

if ($res.ReturnValue -ne 0 -or -not $res.ProcessId) {
    Write-Error "FAILED: Win32_Process Create returned $($res.ReturnValue)"
    exit 1
}

Write-Host "Launch Result: $($res.ReturnValue), ProcessId: $($res.ProcessId)"
Start-Sleep -Seconds 2

$proc = Get-Process -Id $res.ProcessId -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "SUCCESS: sway-host is running with PID $($proc.Id)!"
} else {
    Write-Error "FAILED: Process exited immediately."
    exit 1
}
