package com.sway.app.ui.theme

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}

private val MonochromeColorScheme = darkColorScheme(
    primary = AccentWhite,
    onPrimary = Color.Black,
    primaryContainer = SurfaceElevated,
    onPrimaryContainer = AccentWhite,
    secondary = AccentZinc,
    onSecondary = Color.Black,
    secondaryContainer = SurfaceCard,
    onSecondaryContainer = AccentWhite,
    background = BackgroundDark,
    onBackground = TextPrimary,
    surface = SurfaceDark,
    onSurface = TextPrimary,
    surfaceVariant = SurfaceCard,
    onSurfaceVariant = TextSecondary,
    outline = SurfaceCardBorder,
    outlineVariant = Color(0xFF3F3F46),
    error = Color(0xFFE4E4E7),
    onError = Color.Black
)

@Composable
fun SwayTheme(content: @Composable () -> Unit) {
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val activity = view.context.findActivity() ?: return@SideEffect
            val window = activity.window
            val controller = WindowCompat.getInsetsController(window, view)
            controller.isAppearanceLightStatusBars = false
            controller.isAppearanceLightNavigationBars = false
        }
    }

    MaterialTheme(
        colorScheme = MonochromeColorScheme,
        typography = Typography,
        content = content
    )
}
