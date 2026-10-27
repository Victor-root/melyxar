package app.melyxar.tv.play

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.dp
import app.melyxar.tv.look.LocalPalette
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/*
 * The figures of Material's expressive circular indicator, as the web's
 * `spinner-shape.ts` holds them: the wavy edge is the rounded star of
 * Android's shapes library, `RoundedPolygon.star(13, innerRadius = 0.75,
 * rounding = 0.35 smoothed 0.4, innerRounding = 0.5)` normalised to a square
 * of one, thirteen waves for a ring 64 across with a stroke of 4. One wave,
 * centred on the middle of the square: down from its crest on the angle
 * nought, round the bend, and up to the next crest.
 */
private val DOWN = floatArrayOf(0.491694f, 0f, 0.491694f, 0.019832f, 0.482374f, 0.039663f, 0.463733f, 0.051974f)
private val BEND = floatArrayOf(0.463733f, 0.051974f, 0.425022f, 0.077537f, 0.412448f, 0.128553f, 0.434844f, 0.169179f)
private val UP = floatArrayOf(0.463733f, -0.051973f, 0.482374f, -0.039663f, 0.491694f, -0.019832f, 0.491694f, 0f)
private const val WAVES = 13

/** The radius of the plain circle the track is drawn on, in a square of one. */
private const val ROUND = 0.493962f

/** The ring's square, the width of both lines and the space between them. */
private const val BOX = 64f
private const val STROKE = 4f
private const val GAP = 4f

/** The wavy ring's curves, begun at the crest on the angle nought and going
 *  round clockwise, each as four points. */
private val STAR: List<FloatArray> = buildList {
    val step = 2 * PI / WAVES
    for (wave in 0 until WAVES) {
        add(turned(DOWN, wave * step))
        add(turned(BEND, wave * step))
        add(turned(UP, (wave + 1) * step))
    }
}

/** The same curves, every point pulled out onto the circle at its angle. */
private val FLAT: List<FloatArray> = STAR.map { curve ->
    FloatArray(8) { at ->
        val angle = atan2(curve[at or 1], curve[at and 6])
        ROUND * if (at % 2 == 0) cos(angle) else sin(angle)
    }
}

private fun turned(curve: FloatArray, angle: Double): FloatArray {
    val (c, s) = cos(angle).toFloat() to sin(angle).toFloat()
    return FloatArray(8) { at ->
        val (x, y) = curve[at and 6] to curve[at or 1]
        if (at % 2 == 0) x * c - y * s else x * s + y * c
    }
}

private val STANDARD = CubicBezierEasing(0.2f, 0f, 0f, 1f)
private val EMPHASIZED_ACCELERATE = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f)

/**
 * The ring that turns while a film is on its way, as the web player's:
 * Material's expressive indicator, the one Omnify shows on its first load. A
 * lit line with a wavy edge, the wave running along it one wave a second, and
 * a track finishing the circle with a gap at each end. Told how far the wait
 * has got, [percent], it is lit that far round from the top, flat below a
 * tenth and from ninety-five on, as Material's is. Only drawn again as it
 * turns.
 */
@Composable
fun Spinner(modifier: Modifier = Modifier, percent: () -> Float? = { null }) {
    val palette = LocalPalette.current
    val known by remember { derivedStateOf { percent() != null } }
    val wavy by remember { derivedStateOf { percent()?.let { it > 10f && it < 95f } ?: true } }
    val amplitude by animateFloatAsState(
        targetValue = if (wavy) 1f else 0f,
        animationSpec = tween(500, easing = if (wavy) STANDARD else EMPHASIZED_ACCELERATE),
        label = "amplitude",
    )
    val turning = rememberInfiniteTransition(label = "spinner")
    // Three turns every six seconds, and a quick quarter turn more every
    // second and a half.
    val turn = turning.animateFloat(0f, 1080f, infiniteRepeatable(tween(6_000, easing = LinearEasing)), label = "turn")
    val step = turning.animateFloat(
        0f,
        360f,
        infiniteRepeatable(
            keyframes {
                durationMillis = 6_000
                90f at 300
                90f at 1_500
                180f at 1_800
                180f at 3_000
                270f at 3_300
                270f at 4_500
                360f at 4_800
            },
        ),
        label = "step",
    )
    // The lit share grows evenly for three seconds and draws back in on the
    // standard curve.
    val sweep = turning.animateFloat(
        0.1f,
        0.1f,
        infiniteRepeatable(
            keyframes {
                durationMillis = 6_000
                0.87f at 3_000 using STANDARD
            },
        ),
        label = "sweep",
    )
    // One wave a second along thirteen waves.
    val run = turning.animateFloat(0f, 1f, infiniteRepeatable(tween(13_000, easing = LinearEasing)), label = "run")
    Box(
        modifier.size(64.dp).drawWithCache {
            val scale = (size.minDimension / BOX) * (BOX - STROKE)
            val middle = Offset(size.width / 2, size.height / 2)
            val line = Stroke(STROKE * size.minDimension / BOX, cap = StrokeCap.Round)
            val trackRadius = ROUND * scale
            val trackLength = 2 * PI.toFloat() * trackRadius
            val track = Color.White.copy(alpha = 0.14f)
            val whole = Path()
            val measure = PathMeasure()
            val shown = Path()
            var drawnAt = -1f
            var length = 0f

            // The ring at an amplitude, gone round twice so a piece of it can
            // be taken past its start as the wave runs.
            fun shapeAt(wave: Float) {
                if (wave == drawnAt) return
                drawnAt = wave
                whole.reset()
                repeat(2) {
                    STAR.forEachIndexed { at, curve ->
                        val flat = FLAT[at]
                        fun point(i: Int) = flat[i] + (curve[i] - flat[i]) * wave
                        fun x(i: Int) = middle.x + point(i) * scale
                        fun y(i: Int) = middle.y + point(i + 1) * scale
                        if (it == 0 && at == 0) whole.moveTo(x(0), y(0))
                        whole.cubicTo(x(2), y(2), x(4), y(4), x(6), y(6))
                    }
                }
                measure.setPath(whole, false)
                length = measure.length / 2
            }

            onDrawBehind {
                val wave = if (known) amplitude else 1f
                shapeAt(wave)
                val lit = percent()?.coerceIn(0f, 100f)?.div(100f) ?: sweep.value
                val angle = if (known) -90f else turn.value + step.value
                val running = if (wave > 0f) run.value else 0f
                rotate(angle, middle) {
                    val litLength = lit * length
                    val apart = min(litLength, GAP * size.minDimension / BOX) +
                        2 * min(litLength, STROKE / 2 * size.minDimension / BOX)
                    val from = lit * trackLength + apart
                    val to = trackLength - apart
                    if (to > from) {
                        drawArc(
                            track,
                            startAngle = from / trackLength * 360f,
                            sweepAngle = (to - from) / trackLength * 360f,
                            useCenter = false,
                            topLeft = Offset(middle.x - trackRadius, middle.y - trackRadius),
                            size = Size(2 * trackRadius, 2 * trackRadius),
                            style = line,
                        )
                    }
                    if (litLength > 0f) {
                        // The piece is taken further along and turned back by
                        // as much, so the wave runs while the piece stays.
                        shown.reset()
                        val shift = running * length
                        measure.getSegment(shift, shift + litLength, shown, true)
                        rotate(-running * 360f, middle) {
                            drawPath(shown, palette.accentBright, style = line)
                        }
                    }
                }
            }
        },
    )
}
