package app.melyxar.tv.look

import android.graphics.BlurMaskFilter
import android.graphics.RectF
import android.os.Build
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.graphics.withSave
import app.melyxar.theme.Palette
import kotlin.math.max
import kotlin.math.roundToInt

/*
 * The web's box-shadow, value for value: a CSS pixel is a point of the
 * television. Drawn straight onto the screen's own canvas, where the
 * graphics card works the blur of a rounded box out itself, rather than
 * painted into a picture on the thread that draws the screen, as Compose's
 * shadows are.
 */

/** One shadow of CSS's `box-shadow: x y blur spread color`. */
@Immutable
class BoxShadow(val color: Color, val blur: Dp, val spread: Dp = 0.dp, val x: Dp = 0.dp, val y: Dp = 0.dp)

/** The ink of the light theme's shadows, `rgb(20, 22, 27)`. */
private val LightInk = Color(0xFF14161B)

/** `--shadow-card`, in each theme. */
val Palette.shadowCard: BoxShadow
    get() = if (light) {
        BoxShadow(LightInk.copy(alpha = 0.14f), blur = 14.dp, y = 4.dp)
    } else {
        BoxShadow(Color.Black.copy(alpha = 0.35f), blur = 18.dp, y = 6.dp)
    }

/** `--shadow-raised`, in each theme. */
val Palette.shadowRaised: BoxShadow
    get() = if (light) {
        BoxShadow(LightInk.copy(alpha = 0.18f), blur = 32.dp, y = 12.dp)
    } else {
        BoxShadow(Color.Black.copy(alpha = 0.45f), blur = 40.dp, y = 14.dp)
    }

/** `--shadow-tile`, in each theme: what a card rests on. */
val Palette.shadowTile: Array<BoxShadow>
    get() = if (light) {
        arrayOf(BoxShadow(LightInk.copy(alpha = 0.2f), blur = 3.dp, y = 1.dp))
    } else {
        arrayOf(
            BoxShadow(Color.White.copy(alpha = 0.06f), blur = 0.dp, spread = 1.dp),
            BoxShadow(Color.Black.copy(alpha = 0.45f), blur = 3.dp, y = 1.dp),
        )
    }

/**
 * [shadows] round a piece of [shape], as strong as [shown] says, drawn
 * outside the piece only, as CSS draws them. Below Android 9 the graphics
 * card cannot blur, and they are left out rather than drawn hard.
 */
fun Modifier.boxShadow(shape: Shape, vararg shadows: BoxShadow, shown: () -> Float = { 1f }): Modifier {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
        return this
    }
    return drawWithCache {
        val outline = shape.createOutline(size, layoutDirection, this)
        val corner = (outline as? Outline.Rounded)?.roundRect?.topLeftCornerRadius?.x ?: 0f
        val piece = android.graphics.Path().apply {
            addRoundRect(0f, 0f, size.width, size.height, corner, corner, android.graphics.Path.Direction.CW)
        }
        val layers = shadows.map { shadow ->
            val spread = shadow.spread.toPx()
            val x = shadow.x.toPx()
            val y = shadow.y.toPx()
            DrawnShadow(
                box = RectF(x - spread, y - spread, size.width + x + spread, size.height + y + spread),
                corner = max(0f, corner + spread),
                alpha = shadow.color.alpha,
                paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                    color = shadow.color.toArgb()
                    blurOf(shadow.blur.toPx())?.let { maskFilter = BlurMaskFilter(it, BlurMaskFilter.Blur.NORMAL) }
                },
            )
        }
        onDrawBehind {
            val strength = shown()
            if (strength <= 0f) {
                return@onDrawBehind
            }
            drawIntoCanvas { canvas ->
                canvas.nativeCanvas.withSave {
                    clipOutPath(piece)
                    layers.forEach { layer ->
                        layer.paint.alpha = (layer.alpha * strength * 255).roundToInt()
                        drawRoundRect(layer.box, layer.corner, layer.corner, layer.paint)
                    }
                }
            }
        }
    }
}

private class DrawnShadow(val box: RectF, val corner: Float, val alpha: Float, val paint: android.graphics.Paint)

/**
 * The radius Android's blur takes for a CSS blur of [cssBlur] pixels, or
 * nothing for one too small to show. CSS blurs by a deviation of half its
 * radius; Android turns a radius r into a deviation of 0.57735 r + 0.5.
 */
internal fun blurOf(cssBlur: Float): Float? {
    val deviation = cssBlur / 2
    return if (deviation <= 0.5f) null else (deviation - 0.5f) / 0.57735f
}

/** The web's halo of a card on hover, `--shadow-raised` under it. */
fun Modifier.halo(shape: Shape, palette: Palette, lit: () -> Float) =
    boxShadow(shape, palette.shadowRaised, accentAir(palette.accent), accentBloom(palette.accent), shown = lit)

/** The two blooms of the accent round a lit card or tile:
 *  `0 0 8px accent 55%` and `0 0 26px 2px accent 30%`. */
fun accentAir(accent: Color) = BoxShadow(accent.copy(alpha = 0.55f), blur = 8.dp)

fun accentBloom(accent: Color) = BoxShadow(accent.copy(alpha = 0.30f), blur = 26.dp, spread = 2.dp)
