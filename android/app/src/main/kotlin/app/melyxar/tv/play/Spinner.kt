package app.melyxar.tv.play

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.dp
import app.melyxar.tv.look.LocalPalette
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/** The ring, against the 48 points it is drawn in on the web: how far out,
 *  how far the wave rises off it, and how many waves go round. */
private const val BOX = 48f
private const val RADIUS = 19f
private const val AMPLITUDE = 1.5f
private const val WAVES = 8

/** How many straight pieces make one wave. */
private const val PIECES = 16

/** The web's `--ease` of the sweep. */
private val SWEEP = CubicBezierEasing(0.4f, 0f, 0.2f, 1f)

/**
 * The ring that turns while a film is on its way, as the web player's: a
 * faint track, and over it the accent running round on a wavy edge, growing
 * out of nothing and drawn back in as it turns. Told how far the wait has
 * got, [percent], it stands still and is lit that far round from the top.
 * Only drawn again as it turns.
 */
@Composable
fun Spinner(modifier: Modifier = Modifier, percent: () -> Float? = { null }) {
    val palette = LocalPalette.current
    val turning = rememberInfiniteTransition(label = "spinner")
    val turn = turning.animateFloat(0f, 360f, infiniteRepeatable(tween(2_400, easing = LinearEasing)), label = "turn")
    val lit = turning.animateFloat(
        0.2f,
        0.2f,
        infiniteRepeatable(
            keyframes {
                durationMillis = 1_600
                0.2f at 0 using SWEEP
                0.68f at 800 using SWEEP
            },
            RepeatMode.Restart,
        ),
        label = "lit",
    )
    val from = turning.animateFloat(
        0f,
        1f,
        infiniteRepeatable(
            keyframes {
                durationMillis = 1_600
                0f at 0 using SWEEP
                0.24f at 800 using SWEEP
            },
            RepeatMode.Restart,
        ),
        label = "from",
    )
    Box(
        modifier.size(52.dp).drawWithCache {
            val scale = size.minDimension / BOX
            val middle = Offset(size.width / 2, size.height / 2)
            val wave = Path().apply {
                val steps = WAVES * PIECES
                for (step in 0 until steps) {
                    val angle = step.toDouble() / steps * 2 * PI
                    val out = (RADIUS + AMPLITUDE * sin(WAVES * angle)).toFloat() * scale
                    val x = middle.x + out * cos(angle - PI / 2).toFloat()
                    val y = middle.y + out * sin(angle - PI / 2).toFloat()
                    if (step == 0) moveTo(x, y) else lineTo(x, y)
                }
                close()
            }
            val measure = PathMeasure().apply { setPath(wave, true) }
            val length = measure.length
            val shown = Path()
            val track = Stroke(2 * scale)
            val line = Stroke(4 * scale, cap = StrokeCap.Round)
            onDrawBehind {
                drawCircle(Color.White.copy(alpha = 0.14f), RADIUS * scale, middle, style = track)
                val known = percent()
                if (known != null) {
                    shown.reset()
                    measure.getSegment(0f, length * known.coerceIn(0f, 100f) / 100f, shown, true)
                    drawPath(shown, palette.accentBright, style = line)
                    return@onDrawBehind
                }
                rotate(turn.value) {
                    val start = (from.value % 1f) * length
                    val end = start + lit.value * length
                    shown.reset()
                    measure.getSegment(start, minOf(end, length), shown, true)
                    if (end > length) measure.getSegment(0f, end - length, shown, true)
                    drawPath(shown, palette.accentBright, style = line)
                }
            }
        },
    )
}
