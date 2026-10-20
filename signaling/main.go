package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  64 * 1024,
	WriteBufferSize: 64 * 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

var startTime = time.Now()

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	hub := NewHub()
	go hub.Run()

	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		hub.mu.RLock()
		hostCount := len(hub.hosts)
		clientCount := 0
		for _, list := range hub.clients {
			clientCount += len(list)
		}
		hub.mu.RUnlock()

		_ = json.NewEncoder(w).Encode(map[string]interface{}{
			"status":        "healthy",
			"uptime_seconds": time.Since(startTime).Seconds(),
			"active_hosts":   hostCount,
			"active_clients": clientCount,
			"timestamp":      time.Now().UTC().Format(time.RFC3339),
		})
	})

	http.HandleFunc("/config", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")

		stunURLs := os.Getenv("STUN_SERVER")
		if stunURLs == "" {
			stunURLs = "stun:stun.l.google.com:19302,stun:stun1.l.google.com:19302"
		}

		iceServers := []IceServer{
			{
				URLs: strings.Split(stunURLs, ","),
			},
		}

		turnServer := os.Getenv("TURN_SERVER")
		turnUser := os.Getenv("TURN_USERNAME")
		turnPass := os.Getenv("TURN_CREDENTIAL")
		if turnServer != "" {
			iceServers = append(iceServers, IceServer{
				URLs:       strings.Split(turnServer, ","),
				Username:   turnUser,
				Credential: turnPass,
			})
		}

		_ = json.NewEncoder(w).Encode(ConfigResponse{
			Status:     "ok",
			Version:    "1.0.0",
			IceServers: iceServers,
		})
	})

	http.HandleFunc("/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("[WebSocket] Upgrade failed: %v", err)
			return
		}

		client := &Client{
			hub:  hub,
			conn: conn,
			send: make(chan WSMessage, 512),
		}

		hub.register <- client

		go client.writePump()
		go client.readPump()
	})

	addr := fmt.Sprintf(":%s", port)
	log.Printf("[Sway Signaling] Server starting on %s", addr)
	if err := http.ListenAndServe(addr, nil); err != nil {
		log.Fatalf("[Sway Signaling] Server error: %v", err)
	}
}

func (c *Client) readPump() {
	defer func() {
		c.hub.unregister <- c
		c.conn.Close()
	}()

	c.conn.SetReadLimit(maxMessageSize)
	_ = c.conn.SetReadDeadline(time.Now().Add(pongWait))
	c.conn.SetPongHandler(func(string) error {
		_ = c.conn.SetReadDeadline(time.Now().Add(pongWait))
		return nil
	})

	for {
		msgType, message, err := c.conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				log.Printf("[Client] Read error: %v", err)
			}
			break
		}

		if msgType == websocket.BinaryMessage {
			c.hub.ProcessBinary(c, message)
		} else {
			c.hub.ProcessMessage(c, message)
		}
	}
}

func (c *Client) writePump() {
	ticker := time.NewTicker(pingPeriod)
	defer func() {
		ticker.Stop()
		c.conn.Close()
	}()

	for {
		select {
		case msg, ok := <-c.send:
			_ = c.conn.SetWriteDeadline(time.Now().Add(writeWait))
			if !ok {
				_ = c.conn.WriteMessage(websocket.CloseMessage, []byte{})
				return
			}

			if err := c.conn.WriteMessage(msg.msgType, msg.data); err != nil {
				return
			}

		case <-ticker.C:
			_ = c.conn.SetWriteDeadline(time.Now().Add(writeWait))
			if err := c.conn.WriteMessage(websocket.PingMessage, nil); err != nil {
				return
			}
		}
	}
}
