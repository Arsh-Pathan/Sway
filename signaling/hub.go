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
	maxMessageSize = 512 * 1024 // 512 KB for large SDP payloads
)

// Client represents a connected WebSocket peer (either a Host or an Android Client).
type Client struct {
	hub      *Hub
	conn     *websocket.Conn
	send     chan []byte
	role     string // "host" or "client"
	deviceID string
	token    string
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
				select {
				case c.send <- offlineMsg:
				default:
				}
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
	}

	close(client.send)
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
		pong, _ := json.Marshal(SignalingMessage{Type: TypePong})
		client.sendSafe(pong)

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

	// If a host already exists for this device ID, reject or replace if same token
	if existing, exists := h.hosts[msg.DeviceID]; exists {
		if subtle.ConstantTimeCompare([]byte(existing.token), []byte(msg.Token)) != 1 {
			client.sendError("deviceId already registered with different token")
			return
		}
		// Replace previous connection
		close(existing.send)
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
	client.sendSafe(ack)
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

	// Verify constant-time token comparison
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
	client.sendSafe(ack)

	// Notify host that a mobile client has joined
	notifyHost, _ := json.Marshal(SignalingMessage{
		Type:     TypeStatus,
		Role:     "client",
		DeviceID: msg.DeviceID,
		Payload:  json.RawMessage(`{"status":"client_joined"}`),
	})
	host.sendSafe(notifyHost)
}

// routePeerMessage relays SDP offers, answers, and ICE candidates between paired host and client.
func (h *Hub) routePeerMessage(client *Client, msg SignalingMessage, raw []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	if client.role == "host" {
		// Forward to mobile client(s)
		clients, exists := h.clients[client.deviceID]
		if !exists || len(clients) == 0 {
			log.Printf("[Hub] No active mobile clients for device: %s", client.deviceID)
			return
		}
		for _, c := range clients {
			c.sendSafe(raw)
		}
	} else if client.role == "client" {
		// Forward to host
		host, exists := h.hosts[client.deviceID]
		if !exists {
			client.sendError("target host not online")
			return
		}
		host.sendSafe(raw)
	} else {
		client.sendError("client must register or join before sending peer messages")
	}
}

func (c *Client) sendSafe(data []byte) {
	select {
	case c.send <- data:
	default:
		log.Printf("[Client] Send buffer full, dropping message")
	}
}

func (c *Client) sendError(message string) {
	errMsg, _ := json.Marshal(SignalingMessage{
		Type:  TypeError,
		Error: message,
	})
	c.sendSafe(errMsg)
}
