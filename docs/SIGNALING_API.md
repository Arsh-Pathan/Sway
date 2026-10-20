# Sway Signaling API & Protocol Specification

The **Sway Signaling Server** coordinates WebRTC peer connections between Windows Host machines and Android mobile clients. It facilitates exchange of Session Description Protocol (SDP) offers, answers, and Interactive Connectivity Establishment (ICE) candidates.

---

## 1. Endpoints

### 1.1 HTTP Health Check
- **Path**: `GET /health`
- **Response**: `application/json`
```json
{
  "status": "healthy",
  "uptime_seconds": 1240.5,
  "active_hosts": 1,
  "active_clients": 1,
  "timestamp": "2026-10-20T14:15:00Z"
}
```

### 1.2 WebRTC Configuration (ICE Servers)
- **Path**: `GET /config`
- **Response**: `application/json`
```json
{
  "status": "ok",
  "version": "1.0.0",
  "iceServers": [
    {
      "urls": [
        "stun:stun.l.google.com:19302",
        "stun:stun1.l.google.com:19302"
      ]
    },
    {
      "urls": [
        "turn:YOUR_VPS_IP:3478?transport=udp",
        "turn:YOUR_VPS_IP:3478?transport=tcp"
      ],
      "username": "swayuser",
      "credential": "YOUR_SECURE_PASSWORD"
    }
  ]
}
```

### 1.3 WebSocket Signaling Channel
- **Path**: `GET /ws` (Upgrade: websocket)
- **Protocol**: JSON envelopes over WebSocket text frames.

---

## 2. Message Formats

All WebSocket frames are serialized JSON objects with this structure:

```typescript
interface SignalingMessage {
  type: "register" | "join" | "offer" | "answer" | "candidate" | "status" | "ping" | "pong" | "error" | "bye";
  role?: "host" | "client" | "server";
  deviceId?: string;
  token?: string;
  payload?: any;
  error?: string;
}
```

### 2.1 Host Registration (`register`)
Sent by the Windows Host immediately after connecting:
```json
{
  "type": "register",
  "deviceId": "win-laptop-9f82c",
  "token": "e2a89cb1e6a0d48"
}
```
**Server Response**:
```json
{
  "type": "status",
  "role": "server",
  "deviceId": "win-laptop-9f82c",
  "payload": {
    "status": "registered"
  }
}
```

### 2.2 Mobile Client Join (`join`)
Sent by Android Client after scanning the host's QR code:
```json
{
  "type": "join",
  "deviceId": "win-laptop-9f82c",
  "token": "e2a89cb1e6a0d48"
}
```
**Server Response**:
```json
{
  "type": "status",
  "role": "server",
  "deviceId": "win-laptop-9f82c",
  "payload": {
    "status": "paired"
  }
}
```
*Note: The host also receives a notification: `{"type": "status", "role": "client", "payload": {"status": "client_joined"}}`.*

### 2.3 WebRTC Offer & Answer (`offer` / `answer`)
Relayed transparently between paired peers:
```json
{
  "type": "offer",
  "role": "host",
  "deviceId": "win-laptop-9f82c",
  "payload": {
    "type": "offer",
    "sdp": "v=0\r\no=- 4208082404 2 IN IP4 127.0.0.1..."
  }
}
```

```json
{
  "type": "answer",
  "role": "client",
  "deviceId": "win-laptop-9f82c",
  "payload": {
    "type": "answer",
    "sdp": "v=0\r\no=- 8109841029 2 IN IP4 127.0.0.1..."
  }
}
```

### 2.4 ICE Candidate Exchange (`candidate`)
```json
{
  "type": "candidate",
  "role": "host",
  "deviceId": "win-laptop-9f82c",
  "payload": {
    "candidate": "candidate:842163049 1 udp 1677729535 192.168.1.100 54120 typ srflx raddr 192.168.1.100 rport 54120 generation 0 ufrag n7K2 network-id 1",
    "sdpMid": "0",
    "sdpMLineIndex": 0
  }
}
```

### 2.5 Heartbeat (`ping` / `pong`)
Clients send `{"type": "ping"}` periodically to verify liveliness; server immediately replies `{"type": "pong"}`.
