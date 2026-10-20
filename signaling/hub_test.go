package main

import (
	"encoding/json"
	"testing"
	"time"
)

func createMockClient(role, deviceID, token string) *Client {
	return &Client{
		send:     make(chan WSMessage, 100),
		role:     role,
		deviceID: deviceID,
		token:    token,
	}
}

func TestRegisterHost(t *testing.T) {
	hub := NewHub()
	host := createMockClient("", "", "")
	host.hub = hub

	regMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeRegister,
		DeviceID: "laptop-123",
		Token:    "secret-pass-456",
	})

	hub.ProcessMessage(host, regMsg)

	hub.mu.RLock()
	registered, exists := hub.hosts["laptop-123"]
	hub.mu.RUnlock()

	if !exists {
		t.Fatalf("expected host to be registered")
	}
	if registered.token != "secret-pass-456" {
		t.Fatalf("expected token 'secret-pass-456', got %s", registered.token)
	}

	select {
	case resp := <-host.send:
		var ack SignalingMessage
		if err := json.Unmarshal(resp.data, &ack); err != nil {
			t.Fatalf("failed to parse ack: %v", err)
		}
		if ack.Type != TypeStatus {
			t.Fatalf("expected TypeStatus, got %s", ack.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for register ack")
	}
}

func TestJoinAuthentication(t *testing.T) {
	hub := NewHub()

	// Register host
	host := createMockClient("", "", "")
	host.hub = hub
	regMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeRegister,
		DeviceID: "laptop-abc",
		Token:    "correct-token",
	})
	hub.ProcessMessage(host, regMsg)
	<-host.send // Drain register ack

	// Mobile joins with WRONG token
	clientBad := createMockClient("", "", "")
	clientBad.hub = hub
	badJoinMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeJoin,
		DeviceID: "laptop-abc",
		Token:    "wrong-token",
	})
	hub.ProcessMessage(clientBad, badJoinMsg)

	select {
	case resp := <-clientBad.send:
		var errMsg SignalingMessage
		_ = json.Unmarshal(resp.data, &errMsg)
		if errMsg.Type != TypeError {
			t.Fatalf("expected TypeError for bad token, got %s", errMsg.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for error response")
	}

	// Mobile joins with CORRECT token
	clientGood := createMockClient("", "", "")
	clientGood.hub = hub
	goodJoinMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeJoin,
		DeviceID: "laptop-abc",
		Token:    "correct-token",
	})
	hub.ProcessMessage(clientGood, goodJoinMsg)

	select {
	case resp := <-clientGood.send:
		var ack SignalingMessage
		_ = json.Unmarshal(resp.data, &ack)
		if ack.Type != TypeStatus {
			t.Fatalf("expected TypeStatus for successful join, got %s", ack.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for good join ack")
	}

	// Host should also receive client_joined notification
	select {
	case hostNotif := <-host.send:
		var notif SignalingMessage
		_ = json.Unmarshal(hostNotif.data, &notif)
		if notif.Type != TypeStatus {
			t.Fatalf("expected TypeStatus on host, got %s", notif.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("host was not notified of client join")
	}
}

func TestRouteOfferAndAnswer(t *testing.T) {
	hub := NewHub()

	// Setup paired host and client
	host := createMockClient("host", "laptop-xyz", "token123")
	client := createMockClient("client", "laptop-xyz", "token123")
	host.hub = hub
	client.hub = hub

	hub.mu.Lock()
	hub.hosts["laptop-xyz"] = host
	hub.clients["laptop-xyz"] = []*Client{client}
	hub.mu.Unlock()

	// Host sends SDP Offer
	offerMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeOffer,
		Role:     "host",
		DeviceID: "laptop-xyz",
		Payload:  json.RawMessage(`{"sdp":"v=0..."}`),
	})
	hub.ProcessMessage(host, offerMsg)

	select {
	case received := <-client.send:
		var forwarded SignalingMessage
		_ = json.Unmarshal(received.data, &forwarded)
		if forwarded.Type != TypeOffer {
			t.Fatalf("client did not receive offer, got %s", forwarded.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for offer routing to client")
	}

	// Client sends SDP Answer
	answerMsg, _ := json.Marshal(SignalingMessage{
		Type:     TypeAnswer,
		Role:     "client",
		DeviceID: "laptop-xyz",
		Payload:  json.RawMessage(`{"sdp":"v=0-answer..."}`),
	})
	hub.ProcessMessage(client, answerMsg)

	select {
	case received := <-host.send:
		var forwarded SignalingMessage
		_ = json.Unmarshal(received.data, &forwarded)
		if forwarded.Type != TypeAnswer {
			t.Fatalf("host did not receive answer, got %s", forwarded.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Fatalf("timed out waiting for answer routing to host")
	}
}
