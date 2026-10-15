package app.melyxar.tv.look

import android.graphics.Matrix
import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.CacheDrawScope
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.LinearGradientShader
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.RadialGradientShader
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.SweepGradientShader
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.DrawStyle
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp
import app.melyxar.theme.Palette
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/*
 * What is drawn behind the pages, as the web draws it (`backdrop.tsx`): one
 * of the paintings of light (`styles/lights.css`) or the drawn shelf of the
 * door (`door-background.tsx`). Everything is laid out once per size and
 * palette and never moves: the dust is a handful of point batches, one per
 * strength of its mask, rather than a circle per grain.
 */

/** What an account chose to draw behind the pages: "light" (one of the paintings of light, numbered from 1), "library" (the drawn shelf) or "none". */
@Composable
fun PageBackdrop(backdrop: String, light: Int, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    when (backdrop) {
        "light" -> {
            val painting = PAINTINGS[lightNumber(light) - 1]
            Spacer(modifier.drawWithCache { drawn(paintingOf(painting, palette)) })
        }
        "library" -> Spacer(modifier.drawWithCache { drawn(shelfOf(palette)) })
    }
}

/** The painting an account's number names, the first for one there is not. */
internal fun lightNumber(light: Int) = if (light in 1..PAINTINGS.size) light else 1

/** How much of a band of dust its mask lets through, at a distance from the
 *  mask's centre counted in radii of its ellipse: fully at the centre, a
 *  share [held] at [middle], none from [end] on. */
internal fun fadeAt(distance: Float, middle: Float, held: Float, end: Float): Float = when {
    distance <= middle -> 1f - (1f - held) * distance / middle
    distance < end -> held * (end - distance) / (end - middle)
    else -> 0f
}

/** The two ends of CSS's `linear-gradient(<angle>)` across a box, as x, y,
 *  x, y: through its centre, long enough for its corners to be the ends. */
internal fun gradientLine(degrees: Float, width: Float, height: Float): FloatArray {
    val turn = Math.toRadians(degrees.toDouble())
    val acrossX = sin(turn).toFloat()
    val acrossY = -cos(turn).toFloat()
    val half = (abs(width * acrossX) + abs(height * acrossY)) / 2
    return floatArrayOf(
        width / 2 - acrossX * half,
        height / 2 - acrossY * half,
        width / 2 + acrossX * half,
        height / 2 + acrossY * half,
    )
}

private fun CacheDrawScope.drawn(paint: DrawScope.() -> Unit) = onDrawBehind { clipRect { paint() } }

/* ---------------------------------------------------------------------------
 * The paintings of light
 * ------------------------------------------------------------------------ */

private enum class Tone { Accent, Dim, Bright, Ground, Dust }

/** A colour stop as the stylesheet writes it: a tone at a strength in
 *  percent, or transparent, at a place in percent (in degrees for a fan). */
private class Stop(val tone: Tone?, val strength: Float, val at: Float)

private fun clear(at: Float) = Stop(null, 0f, at)
private fun accent(strength: Float, at: Float = 0f) = Stop(Tone.Accent, strength, at)
private fun dim(strength: Float, at: Float = 0f) = Stop(Tone.Dim, strength, at)
private fun bright(strength: Float, at: Float = 0f) = Stop(Tone.Bright, strength, at)
private fun ground(at: Float) = Stop(Tone.Ground, 100f, at)
private fun dust(strength: Float, at: Float) = Stop(Tone.Dust, strength, at)

private sealed interface Layer

/** `radial-gradient(W% H% at X% Y%, ...)`. */
private class Pool(val wide: Float, val high: Float, val x: Float, val y: Float, val stops: List<Stop>) : Layer

/** `conic-gradient(from F at X% Y%, ...)`. */
private class Fan(val from: Float, val x: Float, val y: Float, val stops: List<Stop>) : Layer

/** `linear-gradient(Adeg, ...)`. */
private class Streak(val degrees: Float, val stops: List<Stop>) : Layer

/** `radial-gradient(circle Rpx at X% Y%, ...)`. */
private class Spark(val radius: Float, val x: Float, val y: Float, val stops: List<Stop>) : Layer

private fun pool(wide: Float, high: Float, x: Float, y: Float, vararg stops: Stop) = Pool(wide, high, x, y, stops.toList())

/** The mask of a band of dust: see [fadeAt]. */
private class Fade(val middle: Float, val held: Float, val end: Float)

private val NEAR_FADE = Fade(0.46f, 0.45f, 0.78f)
private val FAR_FADE = Fade(0.42f, 0.40f, 0.74f)

/** One band of dust: a dot repeated every [step] points, cut by an ellipse
 *  [wide] by [high] percent of the box at [x], [y]. */
private class Band(
    val step: Float,
    val dot: Float,
    val force: Float,
    val wide: Float,
    val high: Float,
    val x: Float,
    val y: Float,
    val fade: Fade,
)

private fun near(wide: Float, high: Float, x: Float, y: Float, step: Float = 17f, dot: Float = 1.1f, force: Float = 0.62f) =
    Band(step, dot, force, wide, high, x, y, NEAR_FADE)

private fun far(wide: Float, high: Float, x: Float, y: Float, step: Float = 29f, dot: Float = 1.1f, force: Float = 0.38f) =
    Band(step, dot, force, wide, high, x, y, FAR_FADE)

/** Its layers as the stylesheet lists them, the first on top. */
private class Painting(val layers: List<Layer>, val near: Band, val far: Band)

/** The points of the constellation in the stylesheet's order, as x, y in
 *  percent and size in points. The larger ones are brighter and wear a glow
 *  five times their size. */
private val STARS = floatArrayOf(
    33f, 18f, 3.4f, 8f, 80f, 1.6f, 51f, 7f, 2.2f, 42f, 26f, 2.8f, 43f, 80f, 1.6f,
    92f, 62f, 2.8f, 40f, 94f, 1.6f, 42f, 54f, 2.8f, 32f, 79f, 1.8f, 13f, 57f, 1.8f,
    9f, 9f, 1.8f, 50f, 53f, 2f, 20f, 76f, 1.6f, 85f, 71f, 2f, 60f, 11f, 2.8f,
    75f, 57f, 2f, 35f, 36f, 2.2f, 58f, 46f, 1.6f, 92f, 48f, 3.4f, 32f, 57f, 3.4f,
    80f, 30f, 2.2f, 24f, 30f, 3.4f, 85f, 11f, 2.2f, 16f, 44f, 2.8f, 93f, 18f, 1.8f,
    85f, 92f, 3.4f, 9f, 23f, 1.8f, 53f, 91f, 2.8f, 53f, 76f, 2f, 63f, 60f, 1.8f,
)

private fun constellation(): List<Layer> {
    val layers = mutableListOf<Layer>()
    for (star in 0 until STARS.size / 3) {
        val x = STARS[star * 3]
        val y = STARS[star * 3 + 1]
        val radius = STARS[star * 3 + 2]
        val lit = radius >= 2.8f
        layers += Spark(radius, x, y, listOf(dust(if (lit) 85f else 65f, 55f), clear(100f)))
        if (lit) layers += Spark(radius * 5, x, y, listOf(accent(22f), clear(100f)))
    }
    layers += pool(44f, 50f, 84f, 16f, accent(18f), clear(74f))
    layers += pool(46f, 44f, 14f, 90f, dim(20f), clear(74f))
    layers += pool(50f, 46f, 46f, 50f, bright(8f), clear(76f))
    return layers
}

private val PAINTINGS = listOf(
    // The one long arc.
    Painting(
        listOf(
            pool(48f, 75f, 18f, 29f, clear(95f), accent(17f, 100f), clear(106f)),
            pool(33f, 41f, 53f, 106f, dim(17f), clear(76f)),
            pool(34f, 40f, 88f, 91f, bright(20f), clear(74f)),
            pool(35f, 43f, 11f, 38f, accent(26f), clear(72f)),
        ),
        near(46f, 34f, 20f, 52f),
        far(44f, 44f, 80f, 34f),
    ),
    // Horizon.
    Painting(
        listOf(
            pool(96f, 44f, 50f, 116f, clear(94f), bright(30f, 98.5f), accent(10f, 101f), clear(108f)),
            pool(70f, 30f, 50f, 76f, accent(24f), clear(72f)),
            pool(96f, 44f, 50f, 116f, dim(14f, 60f), clear(95f)),
            pool(32f, 38f, 88f, 6f, dim(16f), clear(72f)),
        ),
        near(62f, 20f, 50f, 58f),
        far(36f, 34f, 78f, 22f),
    ),
    // Eclipse.
    Painting(
        listOf(
            pool(15f, 27f, 72f, 40f, clear(88f), bright(40f, 100f), accent(16f, 108f), clear(150f)),
            pool(38f, 64f, 72f, 40f, accent(18f), clear(70f)),
            pool(40f, 46f, 10f, 90f, dim(20f), clear(72f)),
        ),
        near(30f, 46f, 72f, 40f),
        far(40f, 40f, 18f, 70f),
    ),
    // Rays.
    Painting(
        listOf(
            pool(30f, 40f, 0f, 0f, bright(30f), clear(72f)),
            pool(36f, 34f, 84f, 92f, accent(18f), clear(74f)),
            pool(130f, 150f, 0f, 0f, clear(40f), ground(104f)),
            Fan(
                90f, 0f, 0f,
                listOf(
                    clear(0f), clear(12f), accent(26f, 17f), clear(23f), clear(30f), bright(30f, 36f), clear(41f),
                    clear(47f), accent(20f, 50f), clear(54f), clear(60f), dim(28f, 67f), clear(75f),
                ),
            ),
        ),
        near(40f, 34f, 34f, 38f),
        far(30f, 36f, 76f, 70f),
    ),
    // Orbits.
    Painting(
        listOf(
            pool(44f, 52f, 96f, 104f, clear(94f), bright(26f, 100f), clear(106f)),
            pool(68f, 86f, 96f, 104f, clear(95f), accent(18f, 100f), clear(105f)),
            pool(96f, 126f, 96f, 104f, clear(96f), accent(11f, 100f), clear(104f)),
            pool(34f, 44f, 96f, 104f, accent(26f), clear(76f)),
            pool(30f, 36f, 6f, 8f, dim(16f), clear(72f)),
        ),
        near(36f, 30f, 56f, 56f),
        far(34f, 40f, 22f, 24f),
    ),
    // Nebula.
    Painting(
        listOf(
            pool(26f, 30f, 48f, 24f, bright(18f), clear(72f)),
            pool(30f, 36f, 64f, 58f, bright(16f), clear(74f)),
            pool(38f, 50f, 30f, 42f, accent(24f), clear(72f)),
            pool(44f, 30f, 80f, 90f, dim(18f), clear(74f)),
            pool(24f, 28f, 10f, 82f, dim(16f), clear(72f)),
        ),
        near(40f, 42f, 44f, 44f, step = 13f),
        far(56f, 52f, 60f, 56f, step = 23f),
    ),
    // Crossing.
    Painting(
        listOf(
            pool(22f, 30f, 52f, 50f, bright(22f), clear(72f)),
            pool(70f, 90f, 8f, 120f, clear(95f), accent(20f, 100f), clear(105f)),
            pool(70f, 90f, 96f, -22f, clear(95f), bright(18f, 100f), clear(105f)),
            pool(34f, 40f, 92f, 88f, dim(20f), clear(74f)),
            pool(30f, 36f, 6f, 10f, accent(16f), clear(72f)),
        ),
        near(30f, 30f, 52f, 50f),
        far(30f, 40f, 14f, 22f),
    ),
    // Spotlight.
    Painting(
        listOf(
            pool(40f, 16f, 50f, 98f, accent(28f), clear(72f)),
            pool(20f, 14f, 50f, 0f, bright(30f), clear(72f)),
            pool(90f, 140f, 50f, -12f, clear(50f), ground(108f)),
            Fan(
                158f, 50f, -12f,
                listOf(clear(0f), accent(24f, 12f), bright(32f, 22f), accent(24f, 32f), clear(44f)),
            ),
        ),
        near(16f, 44f, 50f, 46f),
        far(30f, 30f, 50f, 30f),
    ),
    // Comet.
    Painting(
        listOf(
            pool(16f, 24f, 72f, 30f, bright(34f), clear(72f)),
            pool(36f, 32f, 12f, 94f, dim(16f), clear(72f)),
            pool(76f, 110f, 72f, 30f, clear(18f), ground(100f)),
            Streak(152f, listOf(clear(36f), accent(16f, 45f), bright(28f, 49f), accent(16f, 53f), clear(62f))),
        ),
        near(34f, 22f, 56f, 44f),
        far(30f, 36f, 22f, 20f),
    ),
    // Constellation.
    Painting(
        constellation(),
        near(62f, 16f, 48f, 46f, step = 11f, dot = 0.8f, force = 0.55f),
        far(54f, 24f, 56f, 50f, step = 19f, dot = 0.9f, force = 0.26f),
    ),
)

/** The tones of a palette, turned down on a white screen as the stylesheet
 *  turns them down, with ink rather than light for the dust. */
private class Tones(private val palette: Palette) {
    private val force = if (palette.light) 0.45f else 1f
    private val dustForce = if (palette.light) 0.56f else 1f
    val dust = if (palette.light) palette.text else palette.accentBright
    val dustStrength = 0.62f * dustForce

    fun of(stop: Stop): Color = when (stop.tone) {
        null -> Color.Transparent
        Tone.Accent -> palette.accent.copy(alpha = stop.strength / 100 * force)
        Tone.Dim -> palette.accentDim.copy(alpha = stop.strength / 100 * force)
        Tone.Bright -> palette.accentBright.copy(alpha = stop.strength / 100 * force)
        Tone.Ground -> palette.ground.copy(alpha = stop.strength / 100)
        Tone.Dust -> dust.copy(alpha = stop.strength / 100 * dustForce)
    }
}

/** A gradient laid out once, and the part of the box it covers. */
private class Wash(val brush: Brush, val topLeft: Offset, val size: Size)

/** A band of dust at one strength of its mask: its dots, and how they are drawn. */
private class Dots(val points: FloatArray, val paint: android.graphics.Paint)

/** How many strengths of its mask a band of dust is drawn at. */
private const val SHADES = 16

private fun CacheDrawScope.paintingOf(painting: Painting, palette: Palette): DrawScope.() -> Unit {
    val tones = Tones(palette)
    val washes = painting.layers.asReversed().map { washOf(it, tones) }.toTypedArray()
    val dust = (dotsOf(painting.near, tones) + dotsOf(painting.far, tones)).toTypedArray()
    return {
        for (wash in washes) drawRect(wash.brush, wash.topLeft, wash.size)
        val canvas = drawContext.canvas.nativeCanvas
        for (dots in dust) canvas.drawPoints(dots.points, dots.paint)
    }
}

private fun CacheDrawScope.washOf(layer: Layer, tones: Tones): Wash {
    val width = size.width
    val height = size.height
    return when (layer) {
        is Pool -> {
            val center = Offset(layer.x / 100 * width, layer.y / 100 * height)
            val radiusX = layer.wide / 100 * width
            // Stops past the ellipse's edge widen the gradient to reach them.
            val reach = max(1f, layer.stops.last().at / 100)
            val shader = RadialGradientShader(
                center,
                radiusX * reach,
                layer.stops.map(tones::of),
                layer.stops.map { it.at / 100 / reach },
            )
            shader.setLocalMatrix(Matrix().apply { setScale(1f, layer.high / 100 * height / radiusX, center.x, center.y) })
            Wash(ShaderBrush(shader), Offset.Zero, size)
        }
        is Fan -> {
            val center = Offset(layer.x / 100 * width, layer.y / 100 * height)
            val shader = SweepGradientShader(center, layer.stops.map(tones::of), layer.stops.map { it.at / 360 })
            // CSS turns from the top, a sweep from the right.
            shader.setLocalMatrix(Matrix().apply { setRotate(layer.from - 90f, center.x, center.y) })
            Wash(ShaderBrush(shader), Offset.Zero, size)
        }
        is Streak -> {
            val line = gradientLine(layer.degrees, width, height)
            val shader = LinearGradientShader(
                Offset(line[0], line[1]),
                Offset(line[2], line[3]),
                layer.stops.map(tones::of),
                layer.stops.map { it.at / 100 },
            )
            Wash(ShaderBrush(shader), Offset.Zero, size)
        }
        is Spark -> {
            val center = Offset(layer.x / 100 * width, layer.y / 100 * height)
            val radius = layer.radius.dp.toPx()
            val shader = RadialGradientShader(center, radius, layer.stops.map(tones::of), layer.stops.map { it.at / 100 })
            Wash(ShaderBrush(shader), center - Offset(radius, radius), Size(radius * 2, radius * 2))
        }
    }
}

private fun CacheDrawScope.dotsOf(band: Band, tones: Tones): List<Dots> {
    val step = band.step.dp.toPx()
    val centerX = band.x / 100 * size.width
    val centerY = band.y / 100 * size.height
    val radiusX = band.wide / 100 * size.width
    val radiusY = band.high / 100 * size.height
    val shades = Array(SHADES) { mutableListOf<Float>() }
    for (row in 0 until (size.height / step).toInt() + 1) {
        val y = step / 2 + row * step
        for (column in 0 until (size.width / step).toInt() + 1) {
            val x = step / 2 + column * step
            val distance = hypot((x - centerX) / radiusX, (y - centerY) / radiusY)
            val through = fadeAt(distance, band.fade.middle, band.fade.held, band.fade.end)
            if (through > 0f) {
                shades[min(SHADES - 1, (through * SHADES).toInt())].apply {
                    add(x)
                    add(y)
                }
            }
        }
    }
    // The grain is solid to its size and gone a tenth of a point further.
    val grain = (band.dot + 0.05f).dp.toPx() * 2
    return shades.withIndex().filter { it.value.isNotEmpty() }.map { (shade, points) ->
        val strength = tones.dustStrength * band.force * (shade + 0.5f) / SHADES
        val paint = Paint().apply {
            color = tones.dust.copy(alpha = strength)
            strokeWidth = grain
            strokeCap = StrokeCap.Round
        }
        Dots(points.toFloatArray(), paint.asFrameworkPaint())
    }
}

/* ---------------------------------------------------------------------------
 * The shelf
 * ------------------------------------------------------------------------ */

/** A shape of a drawing, in the units of its square. */
private fun interface Shape {
    fun path(): Path
}

private fun box(x: Float, y: Float, width: Float, height: Float, corner: Float) =
    Shape { Path().apply { addRoundRect(RoundRect(x, y, x + width, y + height, corner, corner)) } }

private fun oval(x: Float, y: Float, radiusX: Float, radiusY: Float = radiusX) =
    Shape { Path().apply { addOval(Rect(x - radiusX, y - radiusY, x + radiusX, y + radiusY)) } }

private fun trace(data: String) = Shape { PathParser().parsePathString(data).toPath() }

/** One shape, outlined or filled, at a share of the drawing's strength,
 *  turned about a point of its square. */
private class Ink(val shape: Shape, val filled: Boolean, val alpha: Float, val turn: Float, val turnX: Float, val turnY: Float)

private fun line(shape: Shape, turn: Float = 0f, turnX: Float = 0f, turnY: Float = 0f) = Ink(shape, false, 1f, turn, turnX, turnY)

private fun fill(shape: Shape, alpha: Float = 1f, turn: Float = 0f, turnX: Float = 0f, turnY: Float = 0f) =
    Ink(shape, true, alpha, turn, turnX, turnY)

/** A drawing and the square it is drawn in. */
private class Drawing(val wide: Float, val tall: Float, val inks: List<Ink>)

private val POSTER = Drawing(
    60f, 90f,
    listOf(
        line(box(0.9f, 0.9f, 58.2f, 88.2f, 4.5f)),
        line(box(6f, 6f, 48f, 52f, 2.6f)),
        line(oval(41f, 19f, 5f)),
        fill(trace("M6 58 L21 33 L33 49 L41 40 L54 58 Z"), 0.5f),
        fill(box(6f, 66f, 42f, 3f, 1.5f)),
        fill(box(6f, 74f, 30f, 3f, 1.5f), 0.7f),
        fill(box(6f, 82f, 19f, 3f, 1.5f), 0.7f),
    ),
)

private val ALBUM = Drawing(
    64f, 64f,
    listOf(
        line(box(0.9f, 0.9f, 62.2f, 62.2f, 4.5f)),
        line(oval(32f, 27f, 15f)),
        line(oval(32f, 27f, 5.4f)),
        fill(oval(32f, 27f, 1.2f)),
        fill(box(11f, 50f, 30f, 3f, 1.5f)),
        fill(box(11f, 57f, 18f, 3f, 1.5f), 0.7f),
    ),
)

private val EPISODE = Drawing(
    96f, 74f,
    listOf(
        line(box(0.9f, 0.9f, 94.2f, 52.2f, 4.5f)),
        line(oval(48f, 27f, 10.5f)),
        fill(trace("M44.8 21.8 L54 27 L44.8 32.2 Z")),
        fill(box(1f, 60f, 56f, 3.2f, 1.6f)),
        fill(box(1f, 68f, 33f, 3.2f, 1.6f), 0.7f),
    ),
)

private val STRIP = Drawing(
    36f, 100f,
    listOf(line(box(0.9f, 0.9f, 34.2f, 98.2f, 3f))) +
        listOf(6f, 18f, 30f, 42f, 54f, 66f, 78f, 90f).flatMap { y ->
            listOf(fill(box(3.4f, y, 4.2f, 5f, 1.2f), 0.6f), fill(box(28.4f, y, 4.2f, 5f, 1.2f), 0.6f))
        } +
        listOf(5f, 29f, 53f, 77f).map { y -> line(box(10.5f, y, 15f, 18f, 1.6f)) },
)

private val BADGE = Drawing(
    48f, 48f,
    listOf(
        line(oval(24f, 24f, 22.6f)),
        fill(trace("M19 15.4 L34 24 L19 32.6 Z")),
    ),
)

private val BAR = Drawing(
    168f, 30f,
    listOf(
        line(box(0.9f, 0.9f, 166.2f, 28.2f, 14.1f)),
        fill(trace("M13 9.6 L23 15 L13 20.4 Z")),
        fill(box(32f, 13.8f, 104f, 2.4f, 1.2f), 0.4f),
        fill(box(32f, 13.8f, 44f, 2.4f, 1.2f)),
        fill(oval(76f, 15f, 4.2f)),
        fill(trace("M145 11.4h3l4-3.4v14l-4-3.4h-3Z"), 0.8f),
        line(trace("M155.6 11.8a3 3 0 0 1 0 6.4")),
    ),
)

private val NOTE = Drawing(
    40f, 48f,
    listOf(
        fill(oval(9f, 38.5f, 6.2f, 4.8f), turn = -18f, turnX = 9f, turnY = 38.5f),
        fill(oval(30f, 33.5f, 6.2f, 4.8f), turn = -18f, turnX = 30f, turnY = 33.5f),
        line(trace("M15 38.5V8M36 33.5V3")),
        fill(trace("M15 8 L36 3 L36 9.6 L15 14.6 Z")),
    ),
)

private val WAVE = Drawing(
    96f, 40f,
    listOf(10f, 22f, 34f, 16f, 28f, 38f, 24f, 12f, 30f, 20f, 34f, 14f).mapIndexed { at, tall ->
        fill(box(at * 8.2f, 20f - tall / 2, 4.2f, tall, 2.1f), 0.5f + tall / 38f * 0.5f)
    },
)

private val CLAPPER = Drawing(
    84f, 64f,
    listOf(
        line(box(1.4f, 24f, 81.2f, 38.6f, 3.4f)),
        fill(box(11f, 36f, 44f, 3.2f, 1.6f), 0.7f),
        fill(box(11f, 45f, 27f, 3.2f, 1.6f), 0.5f),
        line(box(3f, 7f, 78f, 13f, 2.4f), turn = -6f, turnX = 42f, turnY = 14f),
    ) + listOf(10, 30, 50, 70).map { x ->
        fill(trace("M$x 7 h6 l-5 13 h-6 Z"), 0.6f, turn = -6f, turnX = 42f, turnY = 14f)
    },
)

/** One drawing on the shelf: where it stands in percent of the box, its
 *  width in points, its turn, the width in points the box needs before it is
 *  drawn at all, and whether it carries the accent rather than faint grey. */
private class Piece(
    val drawing: Drawing,
    val x: Float,
    val y: Float,
    val wide: Float,
    val turn: Float,
    val needs: Float = 0f,
    val accent: Boolean = false,
)

private val SHELF = listOf(
    Piece(POSTER, 1f, 13f, 112f, -7f),
    Piece(ALBUM, 3f, 70f, 92f, -5f),
    Piece(EPISODE, 2f, 52f, 170f, 4f, needs = 900f),
    Piece(NOTE, 23f, 12f, 40f, 6f, needs = 900f),
    Piece(STRIP, 21f, 56f, 62f, 9f, needs = 900f),
    Piece(WAVE, 8f, 38f, 124f, -2f, needs = 1100f, accent = true),
    Piece(BADGE, 15f, 44f, 52f, 0f, needs = 1100f, accent = true),
    Piece(POSTER, 81f, 10f, 120f, 6f),
    Piece(CLAPPER, 82f, 62f, 96f, -8f),
    Piece(ALBUM, 72f, 18f, 88f, -4f, needs = 1100f),
    Piece(EPISODE, 86f, 44f, 150f, 5f, needs = 900f),
    Piece(NOTE, 94f, 76f, 36f, -10f, needs = 900f),
    Piece(WAVE, 90f, 33f, 104f, 3f, needs = 1100f),
    Piece(BAR, 69f, 82f, 236f, -3f, needs = 1100f),
)

/** No shelf on a box this narrow or narrower: half a poster cut off by the
 *  edge is not a background. */
private const val SHELF_FROM = 700f

private fun CacheDrawScope.shelfOf(palette: Palette): DrawScope.() -> Unit {
    val across = size.width.toDp().value
    val pieces = if (across <= SHELF_FROM) emptyList() else SHELF.filter { it.needs <= across }
    // The line is held to the screen rather than to each drawing's square.
    val outline = Stroke(1.4.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
    val paths = mutableListOf<Path>()
    val colours = mutableListOf<Color>()
    val styles = mutableListOf<DrawStyle>()
    for (piece in pieces) {
        val drawing = piece.drawing
        val scale = piece.wide.dp.toPx() / drawing.wide
        val left = piece.x / 100 * size.width
        val top = piece.y / 100 * size.height
        val placed = Matrix().apply {
            setScale(scale, scale)
            postTranslate(left, top)
            postRotate(piece.turn, left + drawing.wide * scale / 2, top + drawing.tall * scale / 2)
        }
        val colour = if (piece.accent) palette.accent.copy(alpha = 0.5f) else palette.textFaint.copy(alpha = 0.4f)
        for (ink in drawing.inks) {
            val path = ink.shape.path()
            path.asAndroidPath().transform(Matrix(placed).apply { preRotate(ink.turn, ink.turnX, ink.turnY) })
            paths += path
            colours += colour.copy(alpha = colour.alpha * ink.alpha)
            styles += if (ink.filled) Fill else outline
        }
    }
    val drawnPaths = paths.toTypedArray()
    val drawnColours = colours.toTypedArray()
    val drawnStyles = styles.toTypedArray()
    return {
        for (at in drawnPaths.indices) drawPath(drawnPaths[at], drawnColours[at], style = drawnStyles[at])
    }
}
