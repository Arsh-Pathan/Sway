package com.sway.app.network

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress

@Serializable
data class DiscoveredHost(
    val name: String,
    val deviceId: String,
    val token: String,
    val port: Int,
    val ipAddress: String
)

class NetworkDiscovery {
    private val tag = "NetworkDiscovery"
    private val json = Json { ignoreUnknownKeys = true }

    private val _discoveredHost = MutableStateFlow<DiscoveredHost?>(null)
    val discoveredHost = _discoveredHost.asStateFlow()

    suspend fun discover() = withContext(Dispatchers.IO) {
        var socket: DatagramSocket? = null
        try {
            socket = DatagramSocket().apply {
                broadcast = true
                soTimeout = 2000
            }

            val pingData = "SWAY_DISCOVER".toByteArray(Charsets.UTF_8)

            // Broadcast on all active network interfaces to find exact subnet broadcast
            try {
                val interfaces = java.net.NetworkInterface.getNetworkInterfaces()
                while (interfaces.hasMoreElements()) {
                    val networkInterface = interfaces.nextElement()
                    if (networkInterface.isLoopback || !networkInterface.isUp) continue
                    for (interfaceAddress in networkInterface.interfaceAddresses) {
                        val broadcast = interfaceAddress.broadcast ?: continue
                        try {
                            socket.send(DatagramPacket(pingData, pingData.size, broadcast, 8088))
                        } catch (_: Exception) {}
                    }
                }
            } catch (_: Exception) {}

            // Global Wi-Fi broadcast
            try {
                val broadcastPacket = DatagramPacket(
                    pingData,
                    pingData.size,
                    InetAddress.getByName("255.255.255.255"),
                    8088
                )
                socket.send(broadcastPacket)
            } catch (e: Exception) {
                Log.d(tag, "Broadcast send failed: ${e.message}")
            }

            // Also probe USB / localhost
            try {
                val localPacket = DatagramPacket(
                    pingData,
                    pingData.size,
                    InetAddress.getByName("127.0.0.1"),
                    8088
                )
                socket.send(localPacket)
            } catch (_: Exception) {}

            val recvBuf = ByteArray(1024)
            val recvPacket = DatagramPacket(recvBuf, recvBuf.size)
            socket.receive(recvPacket)

            val text = String(recvPacket.data, 0, recvPacket.length, Charsets.UTF_8)
            Log.i(tag, "Received discovery reply: $text from ${recvPacket.address.hostAddress}")

            if (text.contains("sway_beacon")) {
                val host = json.decodeFromString<DiscoveredHostJson>(text)
                val rawIp = recvPacket.address.hostAddress ?: "127.0.0.1"
                val ip = if (rawIp == "0.0.0.0" || rawIp == "::" || rawIp == "0:0:0:0:0:0:0:1") "127.0.0.1" else rawIp
                _discoveredHost.value = DiscoveredHost(
                    name = host.name,
                    deviceId = host.deviceId,
                    token = host.token,
                    port = host.port,
                    ipAddress = ip
                )
            }
        } catch (e: Exception) {
            Log.d(tag, "UDP discovery completed/timeout: ${e.message}")
        } finally {
            socket?.close()
        }
    }
}

@Serializable
private data class DiscoveredHostJson(
    val name: String,
    val deviceId: String,
    val token: String,
    val port: Int
)
