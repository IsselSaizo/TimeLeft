package app.timeleft.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import app.timeleft.core.Countdown
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Analog dial for the rest of the current day. Hour hand: 24-hour scale (one turn = one
 * day); minute and second hands: 60-unit scale. Angles shrink as time runs out, so all
 * hands travel counter-clockwise.
 */
@Composable
fun CountdownDial(c: Countdown, modifier: Modifier = Modifier) {
    Canvas(modifier.aspectRatio(1f)) {
        val r = size.minDimension / 2f
        val center = Offset(size.width / 2f, size.height / 2f)

        // Outer glow rings.
        drawCircle(Glow.copy(alpha = 0.06f), r * 0.98f, center, style = Stroke(r * 0.08f))
        drawCircle(Glow.copy(alpha = 0.55f), r * 0.94f, center, style = Stroke(r * 0.012f))

        // 60 minute ticks.
        for (i in 0 until 60) {
            val major = i % 5 == 0
            val outer = r * 0.9f
            val inner = if (major) r * 0.8f else r * 0.86f
            radial(center, i * 6f, inner, outer, Glow.copy(alpha = if (major) 0.9f else 0.35f), if (major) r * 0.018f else r * 0.008f)
        }
        // 24 hour dots on an inner ring (hour hand scale).
        for (i in 0 until 24) {
            val p = polar(center, i * 15f, r * 0.62f)
            drawCircle(Glow.copy(alpha = if (i % 6 == 0) 0.9f else 0.3f), if (i % 6 == 0) r * 0.022f else r * 0.011f, p)
        }

        val s = c.seconds.toFloat() + c.millis.toFloat() / 1000f
        val m = c.minutes.toFloat() + s / 60f
        val h = c.hours.toFloat() + m / 60f
        if (c.expired) return@Canvas
        hand(center, h * 15f, r * 0.5f, r * 0.035f)
        hand(center, m * 6f, r * 0.76f, r * 0.022f)
        hand(center, s * 6f, r * 0.86f, r * 0.009f, Color(0xFFB8FFD0))
        drawCircle(Glow, r * 0.035f, center)
        drawCircle(Background, r * 0.015f, center)
    }
}

private fun polar(center: Offset, deg: Float, radius: Float): Offset {
    val a = deg / 180f * PI.toFloat()
    return Offset(center.x + sin(a) * radius, center.y - cos(a) * radius)
}

private fun DrawScope.radial(center: Offset, deg: Float, from: Float, to: Float, color: Color, width: Float) {
    drawLine(color, polar(center, deg, from), polar(center, deg, to), width, StrokeCap.Round)
}

private fun DrawScope.hand(center: Offset, deg: Float, length: Float, width: Float, color: Color = Glow) {
    val tail = polar(center, deg + 180f, length * 0.12f)
    val tip = polar(center, deg, length)
    drawLine(color.copy(alpha = 0.18f), tail, tip, width * 4f, StrokeCap.Round)
    drawLine(color, tail, tip, width, StrokeCap.Round)
}
