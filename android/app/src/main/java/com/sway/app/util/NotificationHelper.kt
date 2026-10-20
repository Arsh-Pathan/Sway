package com.sway.app.util

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.sway.app.MainActivity
import com.sway.app.R
import com.sway.app.network.DiscoveredHost

object NotificationHelper {
    const val CHANNEL_ID_ALERTS = "sway_alerts_channel"
    const val CHANNEL_ID_STATUS = "sway_status_channel"

    const val NOTIFICATION_ID_DISCOVERY = 1001
    const val NOTIFICATION_ID_SESSION = 1002

    fun initChannels(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val alertChannel = NotificationChannel(
                CHANNEL_ID_ALERTS,
                "Sway Host Alerts",
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Alerts when desktop hosts are discovered on your network"
                enableVibration(true)
            }

            val statusChannel = NotificationChannel(
                CHANNEL_ID_STATUS,
                "Sway Session Status",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows active desktop streaming session status"
            }

            val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(alertChannel)
            notificationManager.createNotificationChannel(statusChannel)
        }
    }

    fun notifyHostDiscovered(context: Context, host: DiscoveredHost) {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            putExtra("signaling_url", "${host.ipAddress}:${host.port}")
            putExtra("device_id", host.deviceId)
            putExtra("token", host.token)
        }

        val pendingIntent = PendingIntent.getActivity(
            context,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(context, CHANNEL_ID_ALERTS)
            .setSmallIcon(R.drawable.ic_launcher)
            .setContentTitle("🖥️ Sway Host Ready")
            .setContentText("Found '${host.name}' on local Wi-Fi. Tap to connect!")
            .setStyle(
                NotificationCompat.BigTextStyle()
                    .bigText("Host '${host.name}' is online and available for remote control at ${host.ipAddress}:${host.port}. Tap to connect instantly.")
            )
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .setContentIntent(pendingIntent)
            .build()

        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID_DISCOVERY, notification)
        } catch (_: SecurityException) {
            // Handled when notification permission is pending or denied
        }
    }

    fun notifySessionConnected(context: Context, hostName: String) {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }

        val pendingIntent = PendingIntent.getActivity(
            context,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(context, CHANNEL_ID_STATUS)
            .setSmallIcon(R.drawable.ic_launcher)
            .setContentTitle("🟢 Sway Connected")
            .setContentText("Streaming desktop from $hostName (60 FPS)")
            .setOngoing(true)
            .setContentIntent(pendingIntent)
            .build()

        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID_SESSION, notification)
        } catch (_: SecurityException) {}
    }

    fun cancelSessionNotification(context: Context) {
        try {
            NotificationManagerCompat.from(context).cancel(NOTIFICATION_ID_SESSION)
        } catch (_: Exception) {}
    }

    fun notifySessionEnded(context: Context, hostName: String) {
        cancelSessionNotification(context)

        val notification = NotificationCompat.Builder(context, CHANNEL_ID_STATUS)
            .setSmallIcon(R.drawable.ic_launcher)
            .setContentTitle("Sway Session Ended")
            .setContentText("Disconnected from $hostName")
            .setAutoCancel(true)
            .build()

        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID_SESSION + 1, notification)
        } catch (_: SecurityException) {}
    }
}
