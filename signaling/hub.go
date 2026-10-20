package main

import (
	"crypto/subtle"
	"encoding/json"
	"log"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

const (
	writeWait      = 10 * time.Second
	pongWait       = 60 * time.Second
	pingPeriod     = (pongWait * 9) / 10
	maxMessageSize = 10 * 1024 * 1024 // 10 MB for high-resolution desktop frames
)

// WSMessage encapsulates a message type (Text or Binary) and its payload.
type WSMessage struct {
	msgType int
	data    []byte
}

// Client represents a connected WebSocket peer (either a Host or an Android Client).
type Client struct {
	hub       *Hub
	conn      *websocket.Conn
	send      chan WSMessage
	closeOnce sync.Once
	closed    bool
	role      string // "host" or "client"
	deviceID  string
	token     string
}

func (c *Client) Close() {
	c.closeOnce.Do(func() {
		c.closed = true
		close(c.send)
	})
}

// Hub maintains the set of active clients and handles message routing.
type Hub struct {
	mu sync.RWMutex

	// hosts maps deviceID to the active host client
	hosts map[string]*Client

	// clients maps deviceID to a list of connected mobile clients
	clients map[string][]*Client

	// register channel for new client connections
	register chan *Client

	// unregister channel for dropped connections
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
			log.Printf("[Hub] New socket connected from %s", client.conn.RemoteAddr())

		case client := <-h.unregister:
			h.handleDisconnect(client)
		}
	}
}

// handleDisconnect removes the client and notifies remaining peers.
func (h *Hub) handleDisconnect(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if client.role == "host" {
		if currentHost, exists := h.hosts[client.deviceID]; exists && currentHost == client {
			delete(h.hosts, client.deviceID)
			log.Printf("[Hub] Host disconnected for device: %s", client.deviceID)

			// Notify all connected mobile clients that the host is offline
			offlineMsg, _ := json.Marshal(SignalingMessage{
				Type:     TypeStatus,
				Role:     "host",
				DeviceID: client.deviceID,
				Payload:  json.RawMessage(`{"status":"offline"}`),
			})

			for _, c := range h.clients[client.deviceID] {
				c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: offlineMsg})
			}
		}
	} else if client.role == "client" {
		list := h.clients[client.deviceID]
		for i, c := range list {
			if c == client {
				h.clients[client.deviceID] = append(list[:i], list[i+1:]...)
				log.Printf("[Hub] Mobile client disconnected from device: %s", client.deviceID)
				break
			}
		}

		// If no more mobile clients remain for this host, notify host
		if len(h.clients[client.deviceID]) == 0 {
			if host, exists := h.hosts[client.deviceID]; exists {
				notifyHost, _ := json.Marshal(SignalingMessage{
					Type:     TypeStatus,
					Role:     "client",
					DeviceID: client.deviceID,
					Payload:  json.RawMessage(`{"status":"client_left"}`),
				})
				host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: notifyHost})
			}
		}
	}

	client.Close()
}

// ProcessBinary relays binary frames (screen captures and input events) directly between host and mobile.
func (h *Hub) ProcessBinary(client *Client, raw []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	if client.role == "host" {
		// Relay desktop screen frame to paired mobile client(s)
		for _, c := range h.clients[client.deviceID] {
			c.sendSafe(WSMessage{msgType: websocket.BinaryMessage, data: raw})
		}
	} else if client.role == "client" {
		// Relay mouse/keyboard input event to target host
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

	case TypeOffer, TypeAnswer, TypeCandidate:
		h.routePeerMessage(client, msg, raw)

	case TypeBye:
		log.Printf("[Hub] Session bye received from %s (%s)", client.deviceID, client.role)

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

	h.mu.Lock()
	defer h.mu.Unlock()

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

	log.Printf("[Hub] Host successfully registered: device=%s", msg.DeviceID)

	ack, _ := json.Marshal(SignalingMessage{
		Type:     TypeStatus,
		Role:     "server",
		DeviceID: msg.DeviceID,
		Payload:  json.RawMessage(`{"status":"registered"}`),
	})
	client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: ack})
}

// handleJoin authenticates and connects a mobile client to a host.
func (h *Hub) handleJoin(client *Client, msg SignalingMessage) {
	if msg.DeviceID == "" || msg.Token == "" {
		client.sendError("join requires deviceId and token")
		return
	}

	h.mu.Lock()
	defer h.mu.Unlock()

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

	log.Printf("[Hub] Mobile client paired with device=%s", msg.DeviceID)

	ack, _ := json.Marshal(SignalingMessage{
		Type:     TypeStatus,
		Role:     "server",
		DeviceID: msg.DeviceID,
		Payload:  json.RawMessage(`{"status":"paired"}`),
	})
	client.sendSafe(WSMessage{msgType: websocket.TextMessage, data: ack})

	// Notify host that a mobile client has joined
	notifyHost, _ := json.Marshal(SignalingMessage{
		Type:     TypeStatus,
		Role:     "client",
		DeviceID: msg.DeviceID,
		Payload:  json.RawMessage(`{"status":"client_joined"}`),
	})
	host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: notifyHost})
}

// routePeerMessage relays SDP offers, answers, and ICE candidates between paired host and client.
func (h *Hub) routePeerMessage(client *Client, msg SignalingMessage, raw []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	if client.role == "host" {
		clients, exists := h.clients[client.deviceID]
		if !exists || len(clients) == 0 {
			log.Printf("[Hub] No active mobile clients for device: %s", client.deviceID)
			return
		}
		for _, c := range clients {
			c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: raw})
		}
	} else if client.role == "client" {
		host, exists := h.hosts[client.deviceID]
		if !exists {
			client.sendError("target host not online")
			return
		}
		host.sendSafe(WSMessage{msgType: websocket.TextMessage, data: raw})
	} else {
		client.sendError("client must register or join before sending peer messages")
	}
}

func (c *Client) sendSafe(msg WSMessage) {
	if c.closed {
		return
	}
	defer func() {
		_ = recover()
	}()
	select {
	case c.send <- msg:
	default:
		// Send buffer full, drop frame (standard for live video)
	}
}

func (c *Client) sendError(message string) {
	errMsg, _ := json.Marshal(SignalingMessage{
		Type:  TypeError,
		Error: message,
	})
	c.sendSafe(WSMessage{msgType: websocket.TextMessage, data: errMsg})
}
