package main

import "encoding/json"

// MessageType represents the type of a signaling message.
type MessageType string

const (
	TypeRegister  MessageType = "register"  // Host registers its device ID & token
	TypeJoin      MessageType = "join"      // Client requests to connect to a host
	TypeOffer     MessageType = "offer"     // WebRTC SDP offer
	TypeAnswer    MessageType = "answer"    // WebRTC SDP answer
	TypeCandidate MessageType = "candidate" // ICE candidate
	TypeStatus    MessageType = "status"    // Online/offline/state notification
	TypePing      MessageType = "ping"      // Heartbeat ping
	TypePong      MessageType = "pong"      // Heartbeat pong
	TypeError     MessageType = "error"     // Error response
	TypeBye       MessageType = "bye"       // Session teardown
)

// SignalingMessage represents the standard JSON envelope exchanged over WebSocket.
type SignalingMessage struct {
	Type     MessageType     `json:"type"`
	Role     string          `json:"role,omitempty"`     // "host" or "client"
	DeviceID string          `json:"deviceId,omitempty"` // Target or originating device ID
	Token    string          `json:"token,omitempty"`    // Pairing token / auth key
	Payload  json.RawMessage `json:"payload,omitempty"`  // SDP, ICE candidate, or custom metadata
	Error    string          `json:"error,omitempty"`    // Error description if Type == TypeError
}

// ConfigResponse represents the response from GET /config, providing STUN/TURN servers to clients.
type ConfigResponse struct {
	Status     string       `json:"status"`
	Version    string       `json:"version"`
	IceServers []IceServer  `json:"iceServers"`
}

// IceServer defines a STUN or TURN server configuration for WebRTC peer connections.
type IceServer struct {
	URLs       []string `json:"urls"`
	Username   string   `json:"username,omitempty"`
	Credential string   `json:"credential,omitempty"`
}
