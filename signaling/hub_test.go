package main

import (
	"encoding/json"
	"sync"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func createMockClient(hub *Hub, role, deviceID, token string) *Client {
	c := NewClient(hub, nil) // production queue capacities (64/32/1)
	c.role = role
	c.deviceID = deviceID
	c.token = token
	return c
}

func recvCtrl(t *testing.T, c *Client, what string) SignalingMessage {
	t.Helper()
	select {
	case m := <-c.ctrlSend:
		var out SignalingMessage
		if err := json.Unmarshal(m.data, &out); err != nil {
			t.Fatalf("%s: bad JSON: %v", what, err)
		}
		return out
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for %s", what)
	}
	return SignalingMessage{}
}

func statusOf(t *testing.T, m SignalingMessage) string {
	t.Helper()
	var p struct {
		Status string `json:"status"`
	}
	_ = json.Unmarshal(m.Payload, &p)
	return p.Status
}

func register(t *testing.T, hub *Hub, id, token string) *Client {
	t.Helper()
	host := createMockClient(hub, "", "", "")
	msg, _ := json.Marshal(SignalingMessage{Type: TypeRegister, DeviceID: id, Token: token})
	hub.ProcessMessage(host, msg)
	if m := recvCtrl(t, host, "register ack"); statusOf(t, m) != "registered" {
		t.Fatalf("expected registered, got %+v", m)
	}
	return host
}

func join(t *testing.T, hub *Hub, id, token string) *Client {
	t.Helper()
	c := createMockClient(hub, "", "", "")
	msg, _ := json.Marshal(SignalingMessage{Type: TypeJoin, DeviceID: id, Token: token})
	hub.ProcessMessage(c, msg)
	return c
}

func TestRegisterHost(t *testing.T) {
	hub := NewHub()
	host := register(t, hub, "laptop-123", "secret-pass-456")

	hub.mu.RLock()
	registered, exists := hub.hosts["laptop-123"]
	hub.mu.RUnlock()
	if !exists || registered != host || registered.token != "secret-pass-456" {
		t.Fatalf("host not registered correctly")
	}
}

func TestJoinAuthentication(t *testing.T) {
	hub := NewHub()
	host := register(t, hub, "laptop-abc", "correct-token")

	bad := join(t, hub, "laptop-abc", "wrong-token")
	if m := recvCtrl(t, bad, "bad join error"); m.Type != TypeError {
		t.Fatalf("expected error for bad token, got %s", m.Type)
	}

	good := join(t, hub, "laptop-abc", "correct-token")
	if m := recvCtrl(t, good, "paired ack"); statusOf(t, m) != "paired" {
		t.Fatalf("expected paired, got %+v", m)
	}
	if m := recvCtrl(t, host, "client_joined"); statusOf(t, m) != "client_joined" {
		t.Fatalf("expected client_joined, got %+v", m)
	}
}

func TestDoubleAuthRejected(t *testing.T) {
	hub := NewHub()
	register(t, hub, "dev", "tok")
	c := join(t, hub, "dev", "tok")
	recvCtrl(t, c, "paired ack")

	msg, _ := json.Marshal(SignalingMessage{Type: TypeJoin, DeviceID: "dev", Token: "tok"})
	hub.ProcessMessage(c, msg)
	if m := recvCtrl(t, c, "second join"); m.Type != TypeError {
		t.Fatalf("expected error on duplicate join")
	}
	hub.mu.RLock()
	n := len(hub.clients["dev"])
	hub.mu.RUnlock()
	if n != 1 {
		t.Fatalf("client registered %d times", n)
	}
}

func TestRouteOfferAnswerAndTextFocus(t *testing.T) {
	hub := NewHub()
	host := createMockClient(hub, "host", "laptop-xyz", "token123")
	client := createMockClient(hub, "client", "laptop-xyz", "token123")
	hub.mu.Lock()
	hub.hosts["laptop-xyz"] = host
	hub.clients["laptop-xyz"] = []*Client{client}
	hub.mu.Unlock()

	offer, _ := json.Marshal(SignalingMessage{Type: TypeOffer, Payload: json.RawMessage(`{"sdp":"x"}`)})
	hub.ProcessMessage(host, offer)
	if m := recvCtrl(t, client, "offer"); m.Type != TypeOffer {
		t.Fatalf("client did not get offer: %s", m.Type)
	}

	answer, _ := json.Marshal(SignalingMessage{Type: TypeAnswer, Payload: json.RawMessage(`{"sdp":"y"}`)})
	hub.ProcessMessage(client, answer)
	if m := recvCtrl(t, host, "answer"); m.Type != TypeAnswer {
		t.Fatalf("host did not get answer: %s", m.Type)
	}

	tf, _ := json.Marshal(SignalingMessage{Type: TypeTextFocus})
	hub.ProcessMessage(host, tf)
	if m := recvCtrl(t, client, "text_focus"); m.Type != TypeTextFocus {
		t.Fatalf("text_focus not routed: %s", m.Type)
	}
}

func TestInputBurstNeverDroppedWhileVideoOverflows(t *testing.T) {
	hub := NewHub()
	host := createMockClient(hub, "host", "d", "t")
	client := createMockClient(hub, "client", "d", "t")
	hub.mu.Lock()
	hub.hosts["d"] = host
	hub.clients["d"] = []*Client{client}
	hub.mu.Unlock()

	// Flood the client with video frames while the host receives an input burst.
	jpeg := append([]byte{0xFF, 0xD8}, make([]byte, 4000)...)
	for i := 0; i < 500; i++ {
		hub.ProcessBinary(host, jpeg)
	}
	var big []byte
	big = append(big, 0x0E)
	big = append(big, make([]byte, 5000)...) // >1024-byte clipboard-style packet
	packets := [][]byte{{0x02, 1}, {0x03, 1}, {0x04, 0x41}, {0x05, 0x41}, big}
	for _, p := range packets {
		hub.ProcessBinary(client, p)
	}
	for i := range packets {
		select {
		case m := <-host.ctrlSend:
			if len(m.data) != len(packets[i]) {
				t.Fatalf("packet %d reordered or truncated", i)
			}
		default:
			t.Fatalf("input packet %d was dropped", i)
		}
	}
	if len(client.videoSend) != 1 {
		t.Fatalf("expected exactly 1 queued video frame, got %d", len(client.videoSend))
	}
}

func TestAudioAndVideoDoNotStarveEachOther(t *testing.T) {
	hub := NewHub()
	host := createMockClient(hub, "host", "d", "t")
	client := createMockClient(hub, "client", "d", "t")
	hub.mu.Lock()
	hub.hosts["d"] = host
	hub.clients["d"] = []*Client{client}
	hub.mu.Unlock()

	audio := append([]byte("SWAU"), make([]byte, 4000)...)
	jpeg := append([]byte{0xFF, 0xD8}, make([]byte, 4000)...)
	for i := 0; i < 10; i++ {
		hub.ProcessBinary(host, audio)
		hub.ProcessBinary(host, jpeg)
	}
	if len(client.audioSend) == 0 || len(client.videoSend) == 0 {
		t.Fatalf("audio=%d video=%d: both queues must retain data", len(client.audioSend), len(client.videoSend))
	}
	if len(client.ctrlSend) != 0 {
		t.Fatalf("media leaked into control queue")
	}
}

func TestMultiClientLeaveAndHostReconnect(t *testing.T) {
	hub := NewHub()
	host := register(t, hub, "dev", "tok")
	a := join(t, hub, "dev", "tok")
	b := join(t, hub, "dev", "tok")
	recvCtrl(t, a, "a paired")
	recvCtrl(t, b, "b paired")
	recvCtrl(t, host, "joined a")
	recvCtrl(t, host, "joined b")

	hub.handleDisconnect(a)
	select {
	case m := <-host.ctrlSend:
		t.Fatalf("host told client_left while another viewer remains: %s", string(m.data))
	default:
	}

	hub.handleDisconnect(b)
	if m := recvCtrl(t, host, "client_left"); statusOf(t, m) != "client_left" {
		t.Fatalf("expected client_left, got %+v", m)
	}

	// New viewer waits, host drops and re-registers: it must get client_joined.
	c := join(t, hub, "dev", "tok")
	recvCtrl(t, c, "c paired")
	recvCtrl(t, host, "joined c")
	hub.handleDisconnect(host)
	recvCtrl(t, c, "host offline")

	host2 := createMockClient(hub, "", "", "")
	msg, _ := json.Marshal(SignalingMessage{Type: TypeRegister, DeviceID: "dev", Token: "tok"})
	hub.ProcessMessage(host2, msg)
	recvCtrl(t, host2, "registered")
	if m := recvCtrl(t, host2, "client_joined on reconnect"); statusOf(t, m) != "client_joined" {
		t.Fatalf("expected client_joined on reconnect, got %+v", m)
	}
}

func TestConcurrentStress(t *testing.T) {
	hub := NewHub()
	host := createMockClient(hub, "host", "d", "t")
	hub.mu.Lock()
	hub.hosts["d"] = host
	hub.mu.Unlock()

	var wg sync.WaitGroup
	jpeg := append([]byte{0xFF, 0xD8}, make([]byte, 2000)...)
	audio := append([]byte("SWAU"), make([]byte, 2000)...)

	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			c := createMockClient(hub, "", "", "")
			j, _ := json.Marshal(SignalingMessage{Type: TypeJoin, DeviceID: "d", Token: "t"})
			hub.ProcessMessage(c, j)
			for k := 0; k < 200; k++ {
				hub.ProcessBinary(host, jpeg)
				hub.ProcessBinary(host, audio)
				hub.ProcessBinary(c, []byte{0x02, 1})
				ping, _ := json.Marshal(SignalingMessage{Type: TypePing})
				hub.ProcessMessage(c, ping)
			}
			hub.handleDisconnect(c)
		}()
	}
	// Drain host queue concurrently.
	stop := make(chan struct{})
	go func() {
		for {
			select {
			case <-host.ctrlSend:
			case <-stop:
				return
			}
		}
	}()
	wg.Wait()
	close(stop)
	_ = websocket.TextMessage
}
