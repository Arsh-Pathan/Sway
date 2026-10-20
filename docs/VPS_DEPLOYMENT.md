# Sway VPS Deployment Guide

This guide walks you through deploying the **Sway Signaling Server** and **Coturn STUN/TURN Relay** to your own VPS (Ubuntu, Debian, or any Linux server with Docker).

---

## 1. Prerequisites

- A Linux VPS (DigitalOcean, Hetzner, Linode, AWS EC2, etc.) with a public IPv4 address.
- Docker and Docker Compose installed:
  ```bash
  curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh
  ```
- Open ports on your VPS firewall (UFW / Cloud Security Groups):
  - **`8080/tcp`**: Sway WebSocket Signaling Server
  - **`3478/tcp` & `3478/udp`**: Coturn STUN/TURN listening port
  - **`49152-65535/udp`**: Coturn TURN media relay port range

---

## 2. Deploying with Docker Compose

1. Clone or copy the `signaling/` folder to your VPS:
   ```bash
   mkdir -p /opt/sway
   cd /opt/sway
   ```

2. Create a `.env` file in `/opt/sway`:
   ```bash
   VPS_PUBLIC_IP=YOUR_VPS_PUBLIC_IP
   TURN_USER=swayadmin
   TURN_PASS=choose_a_strong_password_here
   PORT=8080
   ```

3. Start the stack:
   ```bash
   docker compose up -d --build
   ```

4. Verify the containers are running:
   ```bash
   docker compose ps
   curl http://localhost:8080/health
   ```
   Output:
   ```json
   {"active_clients":0,"active_hosts":0,"status":"healthy","uptime_seconds":5.2}
   ```

---

## 3. Production Domain & SSL (Recommended via Nginx or Caddy)

To secure WebRTC signaling over `wss://` (secure WebSockets):

### Using Caddy (Automatic Let's Encrypt SSL)
Create a `Caddyfile`:
```caddy
sway.yourdomain.com {
    reverse_proxy localhost:8080
}
```
Run Caddy, and your signaling endpoint will immediately be available at `wss://sway.yourdomain.com/ws`.
