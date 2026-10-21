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

object NotificationHelper {
    const val CHANNEL_ID_STATUS = "sway_status_channel"
    const val NOTIFICATION_ID_SESSION = 1002

    fun initChannels(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val statusChannel = NotificationChannel(
                CHANNEL_ID_STATUS,
                "Sway Remote Session",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Displays active remote desktop session connection"
                enableVibration(false)
                setShowBadge(false)
            }

            val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(statusChannel)
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
            .setSmallIcon(R.drawable.ic_notification)
            .setColor(0xFF111111.toInt())
            .setContentTitle("Sway Active")
            .setContentText("Connected to $hostName")
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setSilent(true)
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
}
