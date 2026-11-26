# Sway Remote Cloud Tunnel
# Exposes Sway Host (port 8089) over the public Internet via Cloudflare Tunnel for mobile data & remote Wi-Fi connections

function Find-Cloudflared {
    $cmd = Get-Command cloudflared -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidates = @(
        "C:\Program Files (x86)\cloudflared\cloudflared.exe",
        "C:\Program Files\cloudflared\cloudflared.exe",
        "$env:LOCALAPPDATA\Microsoft\WinGet\Links\cloudflared.exe"
    )
    foreach ($p in $candidates) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

$cloudflaredPath = Find-Cloudflared
if (-not $cloudflaredPath) {
    Write-Host "[!] cloudflared is not installed. Installing via winget..." -ForegroundColor Yellow
    winget install --id Cloudflare.cloudflared --exact --silent --accept-package-agreements --accept-source-agreements
    $cloudflaredPath = Find-Cloudflared
}

if (-not $cloudflaredPath) {
    Write-Error "Could not locate cloudflared.exe after installation."
    exit 1
}

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "       SWAY WORLDWIDE CLOUD TUNNEL (4G/5G/REMOTE)      " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "Starting Cloudflare tunnel for local port 8089..." -ForegroundColor White

& $cloudflaredPath tunnel --url http://127.0.0.1:8089
