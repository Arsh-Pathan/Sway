package com.sway.app.model

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class SignalingEnvelope(
    val type: String,
    val role: String? = null,
    val deviceId: String? = null,
    val token: String? = null,
    val payload: JsonElement? = null,
    val error: String? = null
)

@Serializable
data class IceServerConfig(
    val urls: List<String>,
    val username: String? = null,
    val credential: String? = null
)

@Serializable
data class ConfigPayload(
    val status: String,
    val version: String,
    val iceServers: List<IceServerConfig>
)

@Serializable
data class SdpPayload(
    val type: String,
    val sdp: String
)

@Serializable
data class CandidatePayload(
    val candidate: String,
    val sdpMid: String? = null,
    val sdpMLineIndex: Int = 0
)
