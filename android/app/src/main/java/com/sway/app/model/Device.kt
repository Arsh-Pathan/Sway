package com.sway.app.model

import kotlinx.serialization.Serializable

@Serializable
data class PairedDevice(
    val id: String,
    val name: String,
    val token: String,
    val signalingUrl: String,
    val lastConnected: Long = System.currentTimeMillis()
)
