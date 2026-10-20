package com.sway.app

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.lifecycle.lifecycleScope
import com.sway.app.network.SignalingClient
import com.sway.app.ui.screens.PairingScreen
import com.sway.app.ui.screens.SessionScreen
import com.sway.app.ui.theme.BackgroundDark
import com.sway.app.ui.theme.SwayTheme
import com.sway.app.webrtc.WebRtcManager
import org.webrtc.PeerConnection

class MainActivity : ComponentActivity() {

    private var signalingClient: SignalingClient? = null
    private var webRtcManager: WebRtcManager? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        setContent {
            SwayTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = BackgroundDark
                ) {
                    var inSession by remember { mutableStateOf(false) }

                    if (inSession && webRtcManager != null) {
                        SessionScreen(
                            webRtcManager = webRtcManager!!,
                            onDisconnect = {
                                cleanupSession()
                                inSession = false
                            }
                        )
                    } else {
                        PairingScreen(
                            onConnect = { serverUrl, deviceId, token ->
                                connectToHost(serverUrl, deviceId, token)
                                inSession = true
                            }
                        )
                    }
                }
            }
        }
    }

    private fun connectToHost(serverUrl: String, deviceId: String, token: String) {
        val client = SignalingClient(
            serverUrl = serverUrl,
            deviceId = deviceId,
            token = token,
            scope = lifecycleScope
        )
        this.signalingClient = client

        val rtc = WebRtcManager(
            context = applicationContext,
            scope = lifecycleScope,
            signalingClient = client
        )
        this.webRtcManager = rtc

        // Connect WebSocket and initialize WebRTC session with default Google STUN
        client.connect()

        val defaultIceServers = listOf(
            PeerConnection.IceServer.builder("stun:stun.l.google.com:19302").createIceServer(),
            PeerConnection.IceServer.builder("stun:stun1.l.google.com:19302").createIceServer()
        )
        rtc.startSession(defaultIceServers)
    }

    private fun cleanupSession() {
        webRtcManager?.release()
        webRtcManager = null
        signalingClient?.disconnect()
        signalingClient = null
    }

    override fun onDestroy() {
        super.onDestroy()
        cleanupSession()
    }
}
