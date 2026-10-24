package com.sway.app.ui.screens

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.*
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Smartphone
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.ui.res.painterResource
import com.sway.app.R
import com.sway.app.model.PairedDevice
import com.sway.app.network.NetworkDiscovery
import com.sway.app.ui.theme.*
import kotlinx.serialization.json.Json
import org.json.JSONObject
import java.util.concurrent.Executors

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairingScreen(
    onConnect: (signalingUrl: String, deviceId: String, token: String) -> Unit
) {
    val context = LocalContext.current
    val prefs = remember { context.getSharedPreferences("sway_prefs", Context.MODE_PRIVATE) }
    var selectedTab by remember { mutableIntStateOf(0) } // 0: QR Scan, 1: Manual

    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
        )
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { isGranted ->
        hasCameraPermission = isGranted
    }

    val discovery = remember { NetworkDiscovery() }
    val discoveredHost by discovery.discoveredHost.collectAsState()

    LaunchedEffect(Unit) {
        while (true) {
            discovery.discover()
            kotlinx.coroutines.delay(2500)
        }
    }

    // Default to the current live laptop credentials if not saved yet
    val defaultUrl = "192.168.11.154:8089"
    val defaultDeviceId = "sway-laptop-b18602ec"
    val defaultToken = "50f0232b-ea9"
    val cloudflareTunnelUrl = "https://framework-governments-introducing-pregnant.trycloudflare.com"

    val isCellular = remember {
        val cm = context.getSystemService(android.content.Context.CONNECTIVITY_SERVICE) as? android.net.ConnectivityManager
        val cap = cm?.getNetworkCapabilities(cm.activeNetwork)
        cap?.hasTransport(android.net.NetworkCapabilities.TRANSPORT_CELLULAR) == true &&
            !(cap.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI))
    }

    // Manual Form & Saved State: Auto-select Cloudflare tunnel if phone is on mobile cellular network
    var signalingUrl by remember {
        val saved = prefs.getString("saved_signaling_url", null)
        val initial = if (isCellular && (saved == null || saved.contains("192.168.") || saved.contains("10.") || saved.contains("172.") || saved.contains("127.0.0.1"))) {
            cloudflareTunnelUrl
        } else {
            saved ?: defaultUrl
        }
        mutableStateOf(initial)
    }
    var deviceId by remember {
        mutableStateOf(prefs.getString("saved_device_id", null) ?: defaultDeviceId)
    }
    var token by remember {
        mutableStateOf(prefs.getString("saved_token", null) ?: defaultToken)
    }

    val connectAndSave = { url: String, devId: String, tok: String ->
        prefs.edit()
            .putString("saved_signaling_url", url)
            .putString("saved_device_id", devId)
            .putString("saved_token", tok)
            .apply()
        onConnect(url, devId, tok)
    }

    // Auto-populate manual fields when a host is discovered on local Wi-Fi
    LaunchedEffect(discoveredHost) {
        discoveredHost?.let { host ->
            deviceId = host.deviceId
            token = host.token
            signalingUrl = "${host.ipAddress}:${host.port}"
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BackgroundDark)
            .padding(20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Spacer(modifier = Modifier.height(24.dp))

        // App Header
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Image(
                painter = painterResource(id = R.drawable.ic_sway_logo),
                contentDescription = "Sway Logo",
                modifier = Modifier
                    .size(46.dp)
                    .clip(RoundedCornerShape(12.dp))
            )
            Column {
                Text(
                    text = "Sway",
                    style = Typography.headlineLarge,
                    color = TextPrimary
                )
                Text(
                    text = "Remote Laptop Controller",
                    style = Typography.bodyMedium,
                    color = TextMuted
                )
            }
        }

        // Auto-Discovery Card or Saved Connection Card
        discoveredHost?.let { host ->
            Surface(
                modifier = Modifier.fillMaxWidth(),
                color = SurfaceCard,
                shape = RoundedCornerShape(16.dp),
                border = BorderStroke(1.dp, SurfaceCardBorder)
            ) {
                Row(
                    modifier = Modifier.padding(14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            "Discovered Laptop",
                            color = AccentWhite,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold
                        )
                        Text(
                            host.name,
                            color = TextPrimary,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 15.sp
                        )
                        Text(
                            "${host.ipAddress}:${host.port} (Direct Wi-Fi / 1ms)",
                            color = TextMuted,
                            fontSize = 12.sp
                        )
                    }
                    Button(
                        onClick = {
                            connectAndSave("${host.ipAddress}:${host.port}", host.deviceId, host.token)
                        },
                        colors = ButtonDefaults.buttonColors(
                            containerColor = androidx.compose.ui.graphics.Color.White,
                            contentColor = androidx.compose.ui.graphics.Color.Black
                        ),
                        shape = RoundedCornerShape(10.dp)
                    ) {
                        Text("Auto-Connect", fontWeight = FontWeight.Bold)
                    }
                }
            }
        } ?: run {
            if (deviceId.isNotBlank() && token.isNotBlank()) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = SurfaceCard,
                    shape = RoundedCornerShape(16.dp),
                    border = BorderStroke(1.dp, SurfaceCardBorder)
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                "SAVED CONNECTION",
                                color = TextMuted,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                letterSpacing = 1.sp
                            )
                            Text(
                                deviceId,
                                color = TextPrimary,
                                fontWeight = FontWeight.SemiBold,
                                fontSize = 15.sp
                            )
                            Text(
                                signalingUrl,
                                color = TextMuted,
                                fontSize = 12.sp
                            )
                        }
                        Button(
                            onClick = {
                                connectAndSave(signalingUrl, deviceId, token)
                            },
                            colors = ButtonDefaults.buttonColors(
                                containerColor = androidx.compose.ui.graphics.Color.White,
                                contentColor = androidx.compose.ui.graphics.Color.Black
                            ),
                            shape = RoundedCornerShape(10.dp)
                        ) {
                            Text("Connect", fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }
        }

        Spacer(modifier = Modifier.height(4.dp))

        // Tab Selector
        PrimaryTabRow(
            selectedTabIndex = selectedTab,
            containerColor = SurfaceDark,
            contentColor = AccentIndigoBright,
            modifier = Modifier.clip(RoundedCornerShape(12.dp))
        ) {
            Tab(
                selected = selectedTab == 0,
                onClick = { selectedTab = 0 },
                text = { Text("Scan QR Code", fontWeight = FontWeight.SemiBold) }
            )
            Tab(
                selected = selectedTab == 1,
                onClick = { selectedTab = 1 },
                text = { Text("Manual Input", fontWeight = FontWeight.SemiBold) }
            )
        }

        if (selectedTab == 0) {
            // ==================== QR SCANNER TAB ====================
            if (hasCameraPermission) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .clip(RoundedCornerShape(20.dp))
                        .background(SurfaceDark),
                    contentAlignment = Alignment.Center
                ) {
                    CameraQrScannerView { scannedJson ->
                        try {
                            val obj = JSONObject(scannedJson)
                            var scannedUrl = obj.optString("server", signalingUrl)
                            val scannedId = obj.getString("deviceId")
                            val scannedToken = obj.getString("token")

                            // If on Mobile Data (Cellular), auto-route local Wi-Fi IPs to the live Cloudflare Remote Tunnel
                            if (isCellular && (scannedUrl.contains("192.168.") || scannedUrl.contains("10.") || scannedUrl.contains("172.") || scannedUrl.contains("127.0.0.1"))) {
                                scannedUrl = cloudflareTunnelUrl
                                Toast.makeText(context, "Mobile Data detected: Connecting via Remote Tunnel", Toast.LENGTH_SHORT).show()
                            }
                            connectAndSave(scannedUrl, scannedId, scannedToken)
                        } catch (e: Exception) {
                            // Raw format: deviceId:token
                            if (scannedJson.contains(":")) {
                                val parts = scannedJson.split(":")
                                val targetUrl = if (isCellular) cloudflareTunnelUrl else signalingUrl
                                connectAndSave(targetUrl, parts[0], parts[1])
                            }
                        }
                    }

                    // Target Overlay Frame
                    Box(
                        modifier = Modifier
                            .size(240.dp)
                            .clip(RoundedCornerShape(16.dp))
                            .background(AccentIndigoGlow)
                    )
                }
            } else {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .clip(RoundedCornerShape(20.dp))
                        .background(SurfaceDark)
                        .padding(24.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        Icon(
                            Icons.Default.QrCodeScanner,
                            contentDescription = null,
                            tint = AccentIndigo,
                            modifier = Modifier.size(56.dp)
                        )
                        Text(
                            text = "Camera access is required to scan the pairing QR code from your laptop.",
                            style = Typography.bodyLarge,
                            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                            color = TextSecondary
                        )
                        Button(
                            onClick = { permissionLauncher.launch(Manifest.permission.CAMERA) },
                            colors = ButtonDefaults.buttonColors(containerColor = AccentIndigo),
                            shape = RoundedCornerShape(12.dp)
                        ) {
                            Text("Grant Camera Permission", fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }
        } else {
            // ==================== MANUAL INPUT TAB ====================
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .clip(RoundedCornerShape(20.dp))
                    .background(SurfaceDark)
                    .padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                // Connection Mode Selector
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    FilterChip(
                        selected = signalingUrl.contains("8089"),
                        onClick = {
                            discoveredHost?.let {
                                signalingUrl = "${it.ipAddress}:${it.port}"
                                deviceId = it.deviceId
                                token = it.token
                            } ?: run {
                                signalingUrl = "192.168.11.229:8089"
                            }
                        },
                        label = { Text("Local Wi-Fi", fontSize = 11.sp) },
                        modifier = Modifier.weight(1f),
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = androidx.compose.ui.graphics.Color.White,
                            selectedLabelColor = androidx.compose.ui.graphics.Color.Black
                        )
                    )
                    FilterChip(
                        selected = signalingUrl.contains("trycloudflare.com") || (!signalingUrl.contains("8089") && !signalingUrl.contains("8080") && signalingUrl.contains(".")),
                        onClick = {
                            signalingUrl = "https://framework-governments-introducing-pregnant.trycloudflare.com"
                        },
                        label = { Text("Mobile 4G/5G", fontSize = 11.sp) },
                        modifier = Modifier.weight(1.1f),
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = androidx.compose.ui.graphics.Color.White,
                            selectedLabelColor = androidx.compose.ui.graphics.Color.Black
                        )
                    )
                    FilterChip(
                        selected = signalingUrl.contains("8080"),
                        onClick = {
                            signalingUrl = "127.0.0.1:8080"
                        },
                        label = { Text("Relay", fontSize = 11.sp) },
                        modifier = Modifier.weight(0.8f),
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = androidx.compose.ui.graphics.Color.White,
                            selectedLabelColor = androidx.compose.ui.graphics.Color.Black
                        )
                    )
                }

                OutlinedTextField(
                    value = signalingUrl,
                    onValueChange = { signalingUrl = it },
                    label = { Text("Server / Host Address") },
                    placeholder = { Text("192.168.x.x:8089 or vps:8080") },
                    modifier = Modifier.fillMaxWidth(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = AccentWhite,
                        unfocusedBorderColor = SurfaceCardBorder,
                        focusedTextColor = TextPrimary,
                        unfocusedTextColor = TextPrimary
                    ),
                    shape = RoundedCornerShape(12.dp)
                )

                OutlinedTextField(
                    value = deviceId,
                    onValueChange = { deviceId = it },
                    label = { Text("Laptop Device ID") },
                    placeholder = { Text("sway-laptop-XXXX") },
                    modifier = Modifier.fillMaxWidth(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = AccentWhite,
                        unfocusedBorderColor = SurfaceCardBorder,
                        focusedTextColor = TextPrimary,
                        unfocusedTextColor = TextPrimary
                    ),
                    shape = RoundedCornerShape(12.dp)
                )

                OutlinedTextField(
                    value = token,
                    onValueChange = { token = it },
                    label = { Text("Pairing Secret / Token") },
                    placeholder = { Text("Enter secret token") },
                    modifier = Modifier.fillMaxWidth(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = AccentWhite,
                        unfocusedBorderColor = SurfaceCardBorder,
                        focusedTextColor = TextPrimary,
                        unfocusedTextColor = TextPrimary
                    ),
                    shape = RoundedCornerShape(12.dp)
                )

                Spacer(modifier = Modifier.weight(1f))

                Button(
                    onClick = {
                        if (deviceId.isNotBlank() && token.isNotBlank()) {
                            connectAndSave(signalingUrl, deviceId.trim(), token.trim())
                        }
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(50.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = androidx.compose.ui.graphics.Color.White,
                        contentColor = androidx.compose.ui.graphics.Color.Black,
                        disabledContainerColor = SurfaceCard,
                        disabledContentColor = TextMuted
                    ),
                    shape = RoundedCornerShape(12.dp),
                    enabled = deviceId.isNotBlank() && token.isNotBlank()
                ) {
                    Text("Connect to Laptop", fontWeight = FontWeight.Bold, fontSize = 16.sp)
                }
            }
        }
    }
}

@androidx.annotation.OptIn(androidx.camera.core.ExperimentalGetImage::class)
@Composable
fun CameraQrScannerView(onQrCodeDetected: (String) -> Unit) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val cameraExecutor = remember { Executors.newSingleThreadExecutor() }
    val barcodeScanner = remember { BarcodeScanning.getClient() }

    var hasScanned by remember { mutableStateOf(false) }

    AndroidView(
        factory = { ctx ->
            val previewView = PreviewView(ctx)
            val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)

            cameraProviderFuture.addListener({
                val cameraProvider = cameraProviderFuture.get()
                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(previewView.surfaceProvider)
                }

                val imageAnalysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()

                imageAnalysis.setAnalyzer(cameraExecutor) { imageProxy ->
                    val mediaImage = imageProxy.image
                    if (mediaImage != null && !hasScanned) {
                        val image = InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
                        barcodeScanner.process(image)
                            .addOnSuccessListener { barcodes ->
                                for (barcode in barcodes) {
                                    barcode.rawValue?.let { raw ->
                                        hasScanned = true
                                        onQrCodeDetected(raw)
                                        return@addOnSuccessListener
                                    }
                                }
                            }
                            .addOnCompleteListener {
                                imageProxy.close()
                            }
                    } else {
                        imageProxy.close()
                    }
                }

                val cameraSelector = CameraSelector.DEFAULT_BACK_CAMERA
                try {
                    cameraProvider.unbindAll()
                    cameraProvider.bindToLifecycle(lifecycleOwner, cameraSelector, preview, imageAnalysis)
                } catch (e: Exception) {
                    e.printStackTrace()
                }
            }, ContextCompat.getMainExecutor(ctx))

            previewView
        },
        modifier = Modifier.fillMaxSize()
    )
}
