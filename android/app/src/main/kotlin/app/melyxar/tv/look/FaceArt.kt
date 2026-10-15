package app.melyxar.tv.look

import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.PathParser
import app.melyxar.theme.FaceDrawing
import app.melyxar.theme.FaceTints
import app.melyxar.theme.faceArtOf
import app.melyxar.theme.faceTints
import kotlin.math.max

/*
 * The paintings an account without a picture wears, drawn as the web draws
 * them (`face-art.tsx`) on its 100 by 100 canvas, scaled to cover the square
 * they are given. Every gradient is laid out once per painting and tint, so
 * drawing one again costs no allocation. A gradient the web spans over a
 * shape's box is placed here over the same box, worked out by hand.
 */

/** The painting an account without a picture wears, in the account's accent. */
@Composable
fun FacePainting(name: String, modifier: Modifier = Modifier) {
    val accent = LocalPalette.current.accent.toArgb()
    val art = remember(name) { faceArtOf(name) }
    val paint = remember(art, accent) { paintingOf(art.drawing, Tints(faceTints(accent, art.turn))) }
    Spacer(
        modifier.drawBehind {
            val fit = max(size.width, size.height) / VIEW
            withTransform({
                translate((size.width - VIEW * fit) / 2, (size.height - VIEW * fit) / 2)
                scale(fit, fit, pivot = Offset.Zero)
            }) { paint() }
        },
    )
}

private const val VIEW = 100f
private val WHOLE = Size(VIEW, VIEW)

private class Tints(tints: FaceTints) {
    val night = Color(tints.night)
    val deep = Color(tints.deep)
    val base = Color(tints.base)
    val glow = Color(tints.glow)
    val light = Color(tints.light)
}

private fun Color.at(opacity: Float) = copy(alpha = opacity)

/** A radial gradient filling a circle, from its centre to its edge. */
private fun centred(x: Float, y: Float, radius: Float, vararg stops: Pair<Float, Color>) =
    Brush.radialGradient(*stops, center = Offset(x, y), radius = radius)

/** A radial gradient over a square box, its centre and radius as shares of
 *  the box's side. */
private fun radialIn(
    left: Float,
    top: Float,
    side: Float,
    x: Float,
    y: Float,
    radius: Float,
    vararg stops: Pair<Float, Color>,
) = Brush.radialGradient(*stops, center = Offset(left + x * side, top + y * side), radius = radius * side)

/** A linear gradient over a box, its ends as shares of the box: in a box
 *  that is not square the bands lean as the web's do. */
private fun linearIn(
    left: Float,
    top: Float,
    width: Float,
    height: Float,
    from: Offset,
    to: Offset,
    vararg stops: Pair<Float, Color>,
): Brush {
    val along = to - from
    val length = along.x * along.x + along.y * along.y
    val acrossX = along.x / (width * length)
    val acrossY = along.y / (height * length)
    val reach = acrossX * acrossX + acrossY * acrossY
    val start = Offset(left + from.x * width, top + from.y * height)
    return Brush.linearGradient(*stops, start = start, end = start + Offset(acrossX / reach, acrossY / reach))
}

private fun pathOf(data: String) = PathParser().parsePathString(data).toPath()

/** A few points of light, as x, y and size. */
private val SKY = floatArrayOf(
    14f, 18f, 0.7f,
    31f, 9f, 0.45f,
    46f, 24f, 0.5f,
    9f, 41f, 0.4f,
    62f, 12f, 0.6f,
    24f, 31f, 0.35f,
    83f, 21f, 0.4f,
)

private fun sky(from: Int = 0, until: Int = SKY.size / 3) = SKY.copyOfRange(from * 3, until * 3)

private fun DrawScope.drawStars(stars: FloatArray, light: Color) {
    repeat(stars.size / 3) { star ->
        val size = stars[star * 3 + 2]
        drawCircle(light, size, Offset(stars[star * 3], stars[star * 3 + 1]), alpha = 0.35f + size * 0.5f)
    }
}

/** The ground every painting starts from and the darkened edges it ends on,
 *  around the drawing itself. */
private fun paintingOf(drawing: FaceDrawing, tints: Tints): DrawScope.() -> Unit {
    val ground = radialIn(0f, 0f, VIEW, 0.3f, 0.2f, 1f, 0f to tints.deep.at(0.55f), 0.55f to tints.night)
    val vignette = radialIn(0f, 0f, VIEW, 0.5f, 0.5f, 0.75f, 0.6f to tints.night.at(0f), 1f to tints.night.at(0.7f))
    val ink = when (drawing) {
        FaceDrawing.Planet -> planet(tints)
        FaceDrawing.Eclipse -> eclipse(tints)
        FaceDrawing.Rings -> rings(tints)
        FaceDrawing.Silk -> silk(tints)
        FaceDrawing.Horizon -> horizon(tints)
        FaceDrawing.Record -> record(tints)
        FaceDrawing.Nebula -> nebula(tints)
        FaceDrawing.Orbits -> orbits(tints)
    }
    return {
        drawRect(tints.night, size = WHOLE)
        drawRect(ground, size = WHOLE)
        ink()
        drawRect(vignette, size = WHOLE)
    }
}

/** A planet seen from its night side, a thin crescent of daylight on it. */
private fun planet(tints: Tints): DrawScope.() -> Unit {
    val dawn = radialIn(0f, 0f, VIEW, 0.18f, 0.2f, 0.6f, 0f to tints.glow.at(0.4f), 1f to tints.glow.at(0f))
    val air = centred(
        76f, 80f, 74f,
        0.84f to tints.glow.at(0.5f), 0.9f to tints.glow.at(0.18f), 1f to tints.glow.at(0f),
    )
    val body = radialIn(
        14f, 18f, 124f, 0.7f, 0.72f, 0.74f,
        0f to tints.night, 0.74f to tints.night, 0.88f to tints.deep, 0.96f to tints.base, 1f to tints.light,
    )
    val stars = sky(until = 5)
    return {
        drawRect(dawn, size = WHOLE)
        drawStars(stars, tints.light)
        drawCircle(air, 74f, Offset(76f, 80f))
        drawCircle(body, 62f, Offset(76f, 80f))
    }
}

/** A disc held against a light: its ring, and one bright point. */
private fun eclipse(tints: Tints): DrawScope.() -> Unit {
    val corona = centred(
        50f, 52f, 46f,
        0.5f to tints.glow.at(0.9f), 0.62f to tints.base.at(0.45f), 1f to tints.base.at(0f),
    )
    val rim = Brush.linearGradient(
        0f to tints.light.at(0.15f), 0.7f to tints.light.at(0.9f), 1f to tints.light,
        start = Offset(27f, 75f),
        end = Offset(73f, 29f),
    )
    val spark = centred(
        65.5f, 35.5f, 9f,
        0f to tints.light, 0.25f to tints.light.at(0.7f), 1f to tints.glow.at(0f),
    )
    val stars = sky(from = 2)
    val rimLine = Stroke(0.9f)
    return {
        drawStars(stars, tints.light)
        drawCircle(corona, 46f, Offset(50f, 52f))
        drawCircle(tints.night, 23f, Offset(50f, 52f))
        drawCircle(rim, 23f, Offset(50f, 52f), style = rimLine)
        drawCircle(spark, 9f, Offset(65.5f, 35.5f))
    }
}

/** A ringed world, the rings passing behind it and in front. */
private fun rings(tints: Tints): DrawScope.() -> Unit {
    val body = radialIn(
        29f, 33f, 42f, 0.32f, 0.28f, 0.85f,
        0f to tints.light, 0.25f to tints.base, 0.6f to tints.deep, 1f to tints.night,
    )
    val ringStops = arrayOf(
        0f to tints.base.at(0.1f),
        0.3f to tints.light.at(0.85f),
        0.7f to tints.base.at(0.6f),
        1f to tints.base.at(0.1f),
    )
    val outer = Brush.horizontalGradient(*ringStops, startX = 7f, endX = 93f)
    val inner = Brush.horizontalGradient(*ringStops, startX = 15f, endX = 85f)
    val halo = centred(50f, 54f, 38f, 0.4f to tints.glow.at(0.35f), 1f to tints.glow.at(0f))
    val outerLine = Stroke(3.2f)
    val innerLine = Stroke(1f)
    val centre = Offset(50f, 54f)
    val band: DrawScope.() -> Unit = {
        drawOval(outer, Offset(7f, 43.5f), Size(86f, 21f), style = outerLine)
        drawOval(inner, Offset(15f, 46f), Size(70f, 16f), alpha = 0.6f, style = innerLine)
    }
    return {
        drawStars(SKY, tints.light)
        drawCircle(halo, 38f, centre)
        rotate(-16f, centre) { band() }
        drawCircle(body, 21f, centre)
        rotate(-16f, centre) {
            clipRect(0f, 54f, VIEW, VIEW) { band() }
        }
    }
}

/** Light falling through the dark in three long folds. */
private fun silk(tints: Tints): DrawScope.() -> Unit {
    val three = pathOf("M-5 78 C 20 50, 48 96, 105 46 L105 60 C 50 104, 22 62, -5 92 Z")
    val two = pathOf("M-5 62 C 26 30, 52 82, 105 28 L105 36 C 54 88, 26 44, -5 70 Z")
    val one = pathOf("M-5 50 C 30 22, 50 66, 105 16 L105 20 C 52 70, 30 30, -5 54 Z")
    // The boxes of the three folds, which their gradients span.
    val threeInk = linearIn(
        -5f, 46f, 110f, 46f, Offset(0f, 0f), Offset(1f, 0f),
        0f to tints.deep.at(0f), 0.35f to tints.base.at(0.55f), 1f to tints.deep.at(0f),
    )
    val twoInk = linearIn(
        -5f, 28f, 110f, 42f, Offset(0f, 0.4f), Offset(1f, 0f),
        0f to tints.glow.at(0f), 0.6f to tints.glow.at(0.7f), 1f to tints.deep.at(0f),
    )
    val oneInk = linearIn(
        -5f, 16f, 110f, 38f, Offset(0f, 0f), Offset(1f, 0.3f),
        0f to tints.base.at(0f), 0.45f to tints.light.at(0.9f), 1f to tints.base.at(0f),
    )
    val stars = floatArrayOf(18f, 14f, 0.5f, 70f, 74f, 0.4f, 86f, 86f, 0.6f, 40f, 8f, 0.35f)
    return {
        drawPath(three, threeInk)
        drawPath(two, twoInk)
        drawPath(one, oneInk)
        drawStars(stars, tints.light)
    }
}

/** Daybreak over the curve of a world. */
private fun horizon(tints: Tints): DrawScope.() -> Unit {
    val sun = radialIn(
        0f, 0f, VIEW, 0.5f, 0.66f, 0.62f,
        0f to tints.light.at(0.95f), 0.18f to tints.glow.at(0.7f), 0.55f to tints.base.at(0.2f), 1f to tints.base.at(0f),
    )
    val land = Brush.verticalGradient(0f to tints.deep.at(0.85f), 0.12f to tints.night, startY = 66f, endY = 270f)
    val edge = Brush.horizontalGradient(
        0f to tints.light.at(0f), 0.5f to tints.light, 1f to tints.light.at(0f),
        startX = -52f,
        endX = 152f,
    )
    val edgeLine = Stroke(1.1f)
    return {
        drawStars(SKY, tints.light)
        drawRect(sun, size = WHOLE)
        drawCircle(land, 102f, Offset(50f, 168f))
        drawCircle(edge, 102f, Offset(50f, 168f), style = edgeLine)
    }
}

/** A record, turning, the grooves catching the light in two wedges. */
private fun record(tints: Tints): DrawScope.() -> Unit {
    val centre = Offset(50f, 50f)
    val halo = centred(50f, 50f, 49f, 0.7f to tints.glow.at(0.4f), 1f to tints.glow.at(0f))
    val disc = centred(50f, 50f, 39f, 0f to tints.deep, 0.4f to tints.night, 1f to tints.night)
    val wedges = pathOf("M50 50 L22 22 A39 39 0 0 1 36 13.6 Z M50 50 L78 78 A39 39 0 0 1 64 86.4 Z")
    // The box of the two wedges, which the sheen spans.
    val sheen = linearIn(
        22f, 13.6f, 56f, 72.8f, Offset(0f, 0f), Offset(1f, 1f),
        0f to tints.light.at(0f), 0.5f to tints.light.at(0.5f), 1f to tints.light.at(0f),
    )
    val label = radialIn(38f, 38f, 24f, 0.35f, 0.3f, 0.8f, 0f to tints.light, 0.4f to tints.base, 1f to tints.deep)
    val grooves = floatArrayOf(36f, 33f, 30f, 27.5f, 25f, 22.5f, 20f)
    val grooveLine = Stroke(0.35f)
    val rimLine = Stroke(0.6f)
    return {
        drawCircle(halo, 49f, centre)
        drawCircle(disc, 39f, centre)
        for (radius in grooves) {
            drawCircle(tints.light, radius, centre, alpha = 0.14f, style = grooveLine)
        }
        drawPath(wedges, sheen, alpha = 0.55f)
        drawCircle(tints.base, 39f, centre, alpha = 0.6f, style = rimLine)
        drawCircle(label, 12f, centre)
        drawCircle(tints.night, 1.6f, centre)
    }
}

/** Clouds of coloured gas around one bright star. */
private fun nebula(tints: Tints): DrawScope.() -> Unit {
    val three = centred(30f, 68f, 46f, 0f to tints.deep.at(0.9f), 1f to tints.deep.at(0f))
    val one = centred(66f, 40f, 40f, 0f to tints.base.at(0.85f), 1f to tints.base.at(0f))
    val two = centred(44f, 54f, 26f, 0f to tints.glow.at(0.7f), 1f to tints.glow.at(0f))
    val star = centred(57f, 47f, 11f, 0f to tints.light, 0.2f to tints.light.at(0.8f), 1f to tints.glow.at(0f))
    val stars = SKY + floatArrayOf(78f, 70f, 0.5f, 20f, 86f, 0.4f, 90f, 52f, 0.35f)
    // Each cloud is a circle whose gradient fills it, flattened into its
    // ellipse so the gradient follows the ellipse's shape as the web's does.
    fun DrawScope.cloud(brush: Brush, x: Float, y: Float, across: Float, high: Float, turn: Float) {
        val centre = Offset(x, y)
        withTransform({
            rotate(turn, centre)
            scale(1f, high / across, centre)
        }) { drawCircle(brush, across, centre) }
    }
    return {
        cloud(three, 30f, 68f, 46f, 34f, 0f)
        cloud(one, 66f, 40f, 40f, 30f, -25f)
        cloud(two, 44f, 54f, 26f, 18f, 20f)
        drawStars(stars, tints.light)
        drawCircle(star, 11f, Offset(57f, 47f))
        drawOval(tints.light, Offset(41f, 46.55f), Size(32f, 0.9f))
        drawOval(tints.light, Offset(56.55f, 35f), Size(0.9f, 24f))
    }
}

/** A small sun and the paths of what turns around it. */
private fun orbits(tints: Tints): DrawScope.() -> Unit {
    val sun = centred(50f, 52f, 18f, 0f to tints.light, 0.3f to tints.glow.at(0.9f), 1f to tints.base.at(0f))
    val centre = Offset(50f, 52f)
    // Each path's gradient spans its own box, before the paths are tilted.
    fun path(across: Float, high: Float) = linearIn(
        50f - across, 52f - high, across * 2, high * 2, Offset(0f, 0f), Offset(1f, 1f),
        0f to tints.light.at(0.65f), 1f to tints.base.at(0.1f),
    )
    val near = path(20f, 12f)
    val middle = path(33f, 20f)
    val far = path(46f, 28f)
    val nearLine = Stroke(0.7f)
    val middleLine = Stroke(0.6f)
    val farLine = Stroke(0.5f)
    val bigMoon = radialIn(75.5f, 34.5f, 8f, 0.35f, 0.3f, 0.8f, 0f to tints.light, 0.5f to tints.base, 1f to tints.deep)
    val smallMoon = radialIn(23f, 69f, 4f, 0.35f, 0.3f, 0.8f, 0f to tints.light, 0.5f to tints.base, 1f to tints.deep)
    val stars = sky(from = 1)
    return {
        drawStars(stars, tints.light)
        rotate(-20f, centre) {
            drawOval(near, Offset(30f, 40f), Size(40f, 24f), style = nearLine)
            drawOval(middle, Offset(17f, 32f), Size(66f, 40f), style = middleLine)
            drawOval(far, Offset(4f, 24f), Size(92f, 56f), alpha = 0.7f, style = farLine)
        }
        drawCircle(sun, 18f, centre)
        drawCircle(tints.light, 4.5f, centre)
        drawCircle(bigMoon, 4f, Offset(79.5f, 38.5f))
        drawCircle(smallMoon, 2f, Offset(25f, 71f))
    }
}
