package com.sway.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.sway.app.network.SignalingClient
import com.sway.app.ui.screens.PairingScreen
import com.sway.app.ui.screens.SessionScreen
import com.sway.app.ui.theme.BackgroundDark
import com.sway.app.ui.theme.SwayTheme
import com.sway.app.util.NotificationHelper
import com.sway.app.webrtc.WebRtcManager
import org.webrtc.PeerConnection

class MainActivity : ComponentActivity() {

    private var signalingClient: SignalingClient? = null
    private var webRtcManager: WebRtcManager? = null
    private var currentHostName: String = "Desktop"

    private val inSessionState = mutableStateOf(false)

    private val notificationPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { _ -> }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        // Request notification permission for Android 13+ (API 33)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }

        handleIncomingIntent(intent)

        setContent {
            SwayTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = BackgroundDark
                ) {
                    val inSession by inSessionState

                    if (inSession && webRtcManager != null) {
                        SessionScreen(
                            webRtcManager = webRtcManager!!,
                            onDisconnect = {
                                cleanupSession()
                                inSessionState.value = false
                            }
                        )
                    } else {
                        PairingScreen(
                            onConnect = { serverUrl, deviceId, token ->
                                connectToHost(serverUrl, deviceId, token)
                                inSessionState.value = true
                            }
                        )
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIncomingIntent(intent)
    }

    private fun handleIncomingIntent(intent: Intent?) {
        val serverUrl = intent?.getStringExtra("signaling_url")
        val deviceId = intent?.getStringExtra("device_id")
        val token = intent?.getStringExtra("token")

        if (!serverUrl.isNullOrEmpty() && !deviceId.isNullOrEmpty() && !token.isNullOrEmpty()) {
            connectToHost(serverUrl, deviceId, token)
            inSessionState.value = true
        }
    }

    private fun connectToHost(serverUrl: String, deviceId: String, token: String) {
        currentHostName = deviceId
        cleanupSession()

        // Sway host streams optimized frames and binary input directly over WebSocket for both local Wi-Fi and cloud tunnels
        val isDirectLocal = true

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
            signalingClient = client,
            isDirectLocal = isDirectLocal
        )
        this.webRtcManager = rtc

        // Connect WebSocket and initialize session
        client.connect()

        NotificationHelper.notifySessionConnected(this, deviceId)
    }

    private fun cleanupSession() {
        webRtcManager?.release()
        webRtcManager = null
        signalingClient?.disconnect()
        signalingClient = null

        NotificationHelper.cancelSessionNotification(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        cleanupSession()
    }
}
