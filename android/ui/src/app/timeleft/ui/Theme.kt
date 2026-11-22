package app.timeleft.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/** "In Time" glow green. */
val Glow = Color(0xFF3DFF7A)
val GlowDim = Color(0xFF1E8C45)
val Background = Color(0xFF040805)
val SurfaceDark = Color(0xFF0B140D)
val Loss = Color(0xFFFF6B5E)

private val scheme = darkColorScheme(
    primary = Glow,
    onPrimary = Color(0xFF002910),
    secondary = GlowDim,
    secondaryContainer = Color(0xFF14432A),
    onSecondaryContainer = Glow,
    background = Background,
    onBackground = Color(0xFFD7F5DE),
    surface = SurfaceDark,
    onSurface = Color(0xFFD7F5DE),
    surfaceVariant = Color(0xFF13221A),
    onSurfaceVariant = Color(0xFF9DBFA6),
    outline = Color(0xFF2F5A3B),
    error = Loss,
)

@Composable
fun TimeLeftTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = scheme, content = content)
}

/** Big glowing digits of the countdown. */
fun glowDigits(size: Int) = TextStyle(
    color = Glow,
    fontFamily = FontFamily.Monospace,
    fontWeight = FontWeight.Bold,
    fontSize = size.sp,
    shadow = Shadow(color = Glow.copy(alpha = 0.85f), offset = Offset.Zero, blurRadius = size * 0.6f),
)

fun deltaColor(ms: Long): Color = if (ms < 0) Loss else Glow
