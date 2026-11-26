package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  64 * 1024,
	WriteBufferSize: 256 * 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

var startTime = time.Now()

const authDeadline = 10 * time.Second

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	hub := NewHub()
	go hub.Run()

	mux := http.NewServeMux()

	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		hub.mu.RLock()
		hostCount := len(hub.hosts)
		clientCount := 0
		for _, list := range hub.clients {
			clientCount += len(list)
		}
		hub.mu.RUnlock()

		_ = json.NewEncoder(w).Encode(map[string]interface{}{
			"status":         "healthy",
			"uptime_seconds": time.Since(startTime).Seconds(),
			"active_hosts":   hostCount,
			"active_clients": clientCount,
			"timestamp":      time.Now().UTC().Format(time.RFC3339),
		})
	})

	mux.HandleFunc("/config", func(w http.ResponseWriter, r *http.Request) {
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

	mux.HandleFunc("/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("[WebSocket] Upgrade failed: %v", err)
			return
		}

		if tcpConn, ok := conn.UnderlyingConn().(*net.TCPConn); ok {
			_ = tcpConn.SetNoDelay(true)
			// Moderate kernel buffers: large ones just add seconds of stale video on WAN links.
			_ = tcpConn.SetWriteBuffer(128 * 1024)
			_ = tcpConn.SetReadBuffer(256 * 1024)
		}

		client := NewClient(hub, conn)
		hub.register <- client

		// Drop sockets that never register/join.
		time.AfterFunc(authDeadline, func() {
			hub.mu.RLock()
			role := client.role
			hub.mu.RUnlock()
			if role == "" {
				client.Close()
			}
		})

		go client.writePump()
		go client.readPump()
	})

	addr := fmt.Sprintf(":%s", port)
	srv := &http.Server{
		Addr:              addr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
		IdleTimeout:       60 * time.Second,
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	go func() {
		log.Printf("[Sway Signaling] Server starting on %s", addr)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("[Sway Signaling] Server error: %v", err)
		}
	}()

	<-ctx.Done()
	log.Printf("[Sway Signaling] Shutting down...")
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	_ = srv.Shutdown(shutdownCtx)
}

func (c *Client) readPump() {
	defer func() {
		c.hub.unregister <- c
		c.Close()
	}()

	c.conn.SetReadLimit(preAuthMessageSize)
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
		// Any traffic proves the peer is alive.
		_ = c.conn.SetReadDeadline(time.Now().Add(pongWait))

		if msgType == websocket.BinaryMessage {
			c.hub.ProcessBinary(c, message)
		} else {
			c.hub.ProcessMessage(c, message)
		}
	}
}

func (c *Client) write(msg WSMessage) error {
	_ = c.conn.SetWriteDeadline(time.Now().Add(writeWait))
	return c.conn.WriteMessage(msg.msgType, msg.data)
}

func (c *Client) writePump() {
	ticker := time.NewTicker(pingPeriod)
	defer func() {
		ticker.Stop()
		c.Close()
	}()

	for {
		// Control/input traffic always goes first.
		select {
		case msg := <-c.ctrlSend:
			if c.write(msg) != nil {
				return
			}
			continue
		default:
		}

		select {
		case <-c.done:
			_ = c.conn.SetWriteDeadline(time.Now().Add(time.Second))
			_ = c.conn.WriteMessage(websocket.CloseMessage, []byte{})
			return

		case msg := <-c.ctrlSend:
			if c.write(msg) != nil {
				return
			}

		case msg := <-c.audioSend:
			if c.write(msg) != nil {
				return
			}

		case msg := <-c.videoSend:
			if c.write(msg) != nil {
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
