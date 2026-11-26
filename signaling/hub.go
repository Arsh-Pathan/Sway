package main

import (
	"crypto/subtle"
	"encoding/json"
	"log"
	"sync"
	"sync/atomic"
	"time"

	"github.com/gorilla/websocket"
)

const (
	writeWait  = 10 * time.Second
	pongWait   = 60 * time.Second
	pingPeriod = 25 * time.Second

	// Pre-authentication read limit; raised once the peer registers/joins.
	preAuthMessageSize = 64 * 1024
	clientMessageSize  = 256 * 1024
	maxMessageSize     = 10 * 1024 * 1024 // 10 MB for high-resolution desktop frames (host only)
)

// WSMessage encapsulates a message type (Text or Binary) and its payload.
type WSMessage struct {
	msgType int
	data    []byte
}

// Client represents a connected WebSocket peer (either a Host or an Android Client).
//
// Outbound traffic is split into three queues so that media can never evict
// control/input traffic and audio and video never evict each other:
//   - ctrlSend:  JSON signaling and binary input packets (never dropped on overflow of media)
//   - audioSend: "SWAU" PCM chunks (drop-oldest)
//   - videoSend: JPEG frames (capacity 1, drop-stale)
type Client struct {
	hub       *Hub
	conn      *websocket.Conn
	ctrlSend  chan WSMessage
	audioSend chan WSMessage
	videoSend chan WSMessage
	done      chan struct{}
	closeOnce sync.Once
	closed    atomic.Bool

	// role / deviceID / token are protected by Hub.mu.
	role     string // "host" or "client"
	deviceID string
	token    string
}

// NewClient builds a client with production queue capacities.
func NewClient(hub *Hub, conn *websocket.Conn) *Client {
	return &Client{
		hub:       hub,
		conn:      conn,
		ctrlSend:  make(chan WSMessage, 64),
		audioSend: make(chan WSMessage, 32),
		videoSend: make(chan WSMessage, 1),
		done:      make(chan struct{}),
	}
}

// Close marks the client closed and tears down the socket. The send channels are
// intentionally never closed, which removes any send-vs-close race.
func (c *Client) Close() {
	c.closeOnce.Do(func() {
		c.closed.Store(true)
		close(c.done)
		if c.conn != nil {
			_ = c.conn.Close()
		}
	})
}

// Hub maintains the set of active clients and handles message routing.
type Hub struct {
	mu sync.RWMutex

	// hosts maps deviceID to the active host client
	hosts map[string]*Client

	// clients maps deviceID to a list of connected mobile clients
	clients map[string][]*Client

	register   chan *Client
	unregister chan *Client
}

// NewHub creates a new Hub instance.
func NewHub() *Hub {
	return &Hub{
		hosts:      make(map[string]*Client),
		clients:    make(map[string][]*Client),
		register:   make(chan *Client),
		unregister: make(chan *Client),
	}
}

// Run starts the hub event loop.
func (h *Hub) Run() {
	for {
		select {
		case client := <-h.register:
			if client.conn != nil {
				log.Printf("[Hub] New socket connected from %s", client.conn.RemoteAddr())
			}

		case client := <-h.unregister:
			h.handleDisconnect(client)
		}
	}
}

func statusMsg(role, deviceID, status string) []byte {
	b, _ := json.Marshal(SignalingMessage{
		Type:     TypeStatus,
		Role:     role,
		DeviceID: deviceID,
		Payload:  json.RawMessage(`{"status":"` + status + `"}`),
	})
	return b
}

// handleDisconnect removes the client and notifies remaining peers.
func (h *Hub) handleDisconnect(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()

	switch client.role {
	case "host":
		if currentHost, exists := h.hosts[client.deviceID]; exists && currentHost == client {
			delete(h.hosts, client.deviceID)
			log.Printf("[Hub] Host disconnected for device: %s", client.deviceID)

			offline := statusMsg("host", client.deviceID, "offline")
			for _, c := range h.clients[client.deviceID] {
				c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: offline})
			}
		}

	case "client":
		list := h.clients[client.deviceID]
		for i, c := range list {
			if c != client {
				continue
			}
			last := len(list) - 1
			copy(list[i:], list[i+1:])
			list[last] = nil // do not pin the disconnected client in the backing array
			list = list[:last]
			break
		}
		if len(list) == 0 {
			delete(h.clients, client.deviceID)
			log.Printf("[Hub] Last mobile client disconnected for device: %s", client.deviceID)
			if host, ok := h.hosts[client.deviceID]; ok {
				host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: statusMsg("client", client.deviceID, "client_left")})
			}
		} else {
			h.clients[client.deviceID] = list
		}
	}

	client.Close()
}

// ProcessBinary relays binary frames (screen captures, audio and input events) between host and mobile.
func (h *Hub) ProcessBinary(client *Client, raw []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	switch client.role {
	case "host":
		for _, c := range h.clients[client.deviceID] {
			c.sendSafe(WSMessage{msgType: websocket.BinaryMessage, data: raw})
		}
	case "client":
		if host, exists := h.hosts[client.deviceID]; exists {
			host.sendSafe(WSMessage{msgType: websocket.BinaryMessage, data: raw})
		}
	}
}

// ProcessMessage routes a parsed signaling message.
func (h *Hub) ProcessMessage(client *Client, raw []byte) {
	var msg SignalingMessage
	if err := json.Unmarshal(raw, &msg); err != nil {
		client.sendError("invalid JSON payload")
		return
	}

	switch msg.Type {
	case TypePing:
		pong, _ := json.Marshal(SignalingMessage{
			Type:    TypePong,
			Payload: msg.Payload,
		})
		client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: pong})

	case TypeRegister:
		h.handleRegister(client, msg)

	case TypeJoin:
		h.handleJoin(client, msg)

	case TypeOffer, TypeAnswer, TypeCandidate, TypeTextFocus:
		h.routePeerMessage(client, msg, raw)

	case TypeBye:
		h.mu.RLock()
		role, id := client.role, client.deviceID
		h.mu.RUnlock()
		log.Printf("[Hub] Session bye received from %s (%s)", id, role)

	default:
		client.sendError("unrecognized message type: " + string(msg.Type))
	}
}

// handleRegister registers a Windows host.
func (h *Hub) handleRegister(client *Client, msg SignalingMessage) {
	if msg.DeviceID == "" || msg.Token == "" {
		client.sendError("registration requires deviceId and token")
		return
	}
	if len(msg.DeviceID) > 64 || len(msg.Token) > 128 {
		client.sendError("deviceId or token exceeds maximum allowed length")
		return
	}

	h.mu.Lock()
	defer h.mu.Unlock()

	if client.role != "" {
		client.sendError("already authenticated")
		return
	}

	if existing, exists := h.hosts[msg.DeviceID]; exists {
		if subtle.ConstantTimeCompare([]byte(existing.token), []byte(msg.Token)) != 1 {
			client.sendError("deviceId already registered with different token")
			return
		}
		existing.Close()
	}

	client.role = "host"
	client.deviceID = msg.DeviceID
	client.token = msg.Token
	h.hosts[msg.DeviceID] = client
	if client.conn != nil {
		client.conn.SetReadLimit(maxMessageSize)
	}

	log.Printf("[Hub] Host successfully registered: device=%s", msg.DeviceID)

	client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: statusMsg("server", msg.DeviceID, "registered")})

	// If viewers are already waiting (host reconnected), tell the host so streaming resumes.
	if len(h.clients[msg.DeviceID]) > 0 {
		client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: statusMsg("client", msg.DeviceID, "client_joined")})
	}
}

// handleJoin authenticates and connects a mobile client to a host.
func (h *Hub) handleJoin(client *Client, msg SignalingMessage) {
	if msg.DeviceID == "" || msg.Token == "" {
		client.sendError("join requires deviceId and token")
		return
	}
	if len(msg.DeviceID) > 64 || len(msg.Token) > 128 {
		client.sendError("deviceId or token exceeds maximum allowed length")
		return
	}

	h.mu.Lock()
	defer h.mu.Unlock()

	if client.role != "" {
		client.sendError("already authenticated")
		return
	}

	host, exists := h.hosts[msg.DeviceID]
	if !exists {
		client.sendError("host device not online")
		return
	}

	if subtle.ConstantTimeCompare([]byte(host.token), []byte(msg.Token)) != 1 {
		client.sendError("authentication failed: invalid pairing token")
		return
	}

	client.role = "client"
	client.deviceID = msg.DeviceID
	client.token = msg.Token
	h.clients[msg.DeviceID] = append(h.clients[msg.DeviceID], client)
	if client.conn != nil {
		client.conn.SetReadLimit(clientMessageSize)
	}

	log.Printf("[Hub] Mobile client paired with device=%s", msg.DeviceID)

	client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: statusMsg("server", msg.DeviceID, "paired")})
	host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: statusMsg("client", msg.DeviceID, "client_joined")})
}

// routePeerMessage relays SDP offers, answers, ICE candidates and text-focus hints between paired peers.
func (h *Hub) routePeerMessage(client *Client, msg SignalingMessage, raw []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	switch client.role {
	case "host":
		clients := h.clients[client.deviceID]
		if len(clients) == 0 {
			return
		}
		for _, c := range clients {
			c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: raw})
		}
	case "client":
		host, exists := h.hosts[client.deviceID]
		if !exists {
			client.sendError("target host not online")
			return
		}
		host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: raw})
	default:
		client.sendError("client must register or join before sending peer messages")
	}
}

func isAudioPacket(d []byte) bool {
	return len(d) >= 4 && d[0] == 'S' && d[1] == 'W' && d[2] == 'A' && d[3] == 'U'
}

func isJPEGFrame(d []byte) bool {
	return len(d) >= 2 && d[0] == 0xFF && d[1] == 0xD8
}

// sendSafe enqueues a message without ever blocking the caller (callers may hold Hub.mu).
func (c *Client) sendSafe(msg WSMessage) {
	if c.closed.Load() {
		return
	}

	if msg.msgType == websocket.BinaryMessage {
		switch {
		case isAudioPacket(msg.data):
			select {
			case c.audioSend <- msg:
				return
			default:
			}
			// Full: drop the oldest chunk and retry once.
			select {
			case <-c.audioSend:
			default:
			}
			select {
			case c.audioSend <- msg:
			default:
			}
			return

		case isJPEGFrame(msg.data):
			select {
			case c.videoSend <- msg:
				return
			default:
			}
			// Stale frame queued: replace it with the newest one.
			select {
			case <-c.videoSend:
			default:
			}
			select {
			case c.videoSend <- msg:
			default:
			}
			return
		}
	}

	// JSON signaling and binary input packets (of any size) take the control queue.
	select {
	case c.ctrlSend <- msg:
	default:
		log.Printf("[Hub] control queue full for device=%s, dropping message", c.deviceID)
	}
}

func (c *Client) sendError(message string) {
	errMsg, _ := json.Marshal(SignalingMessage{
		Type:  TypeError,
		Error: message,
	})
	c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: errMsg})
}
