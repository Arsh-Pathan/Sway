# Multi-stage build for minimal image size (~15MB)
FROM golang:1.22-alpine AS builder

WORKDIR /app

# Install git and ca-certificates
RUN apk add --no-cache git ca-certificates

COPY go.mod go.sum* ./
RUN go mod download || true

COPY . .

# Build static binary
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o sway-signaling .

# Minimal scratch/alpine runtime
FROM alpine:3.19

WORKDIR /app
RUN apk --no-cache add ca-certificates tzdata

COPY --from=builder /app/sway-signaling /app/sway-signaling

EXPOSE 8080

ENV PORT=8080
ENV STUN_SERVER="stun:stun.l.google.com:19302,stun:stun1.l.google.com:19302"

ENTRYPOINT ["/app/sway-signaling"]
