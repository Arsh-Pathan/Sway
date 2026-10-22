# Sway Remote Cloud Tunnel
# Exposes Sway Host (port 8089) over the public Internet via Cloudflare Tunnel for mobile data & remote Wi-Fi connections

$cloudflaredPath = "C:\Program Files (x86)\cloudflared\cloudflared.exe"
if (-not (Test-Path $cloudflaredPath)) {
    $cloudflaredPath = "C:\Program Files\cloudflared\cloudflared.exe"
}

if (-not (Test-Path $cloudflaredPath)) {
    Write-Host "[!] cloudflared is not installed. Installing via winget..." -ForegroundColor Yellow
    winget install --id Cloudflare.cloudflared --exact --silent --accept-package-agreements --accept-source-agreements
}

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "       SWAY WORLDWIDE CLOUD TUNNEL (4G/5G/REMOTE)      " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "Starting Cloudflare tunnel for local port 8089..." -ForegroundColor White

& $cloudflaredPath tunnel --url http://127.0.0.1:8089
