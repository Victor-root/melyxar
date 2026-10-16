package app.melyxar.tv.look

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.addOutline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp

/*
 * What the web does on hover, done when the remote comes to a piece: it
 * grows a little and its rim lights in the accent; a card also wears the
 * web's halo (`Shadows.kt`). Every one of these reads how lit the piece is
 * while it draws, so the easing in and out never composes the piece again.
 */

/** How lit a piece is, from 0 at rest to 1 when the remote is on it, eased
 *  the way the web eases its hover. */
@Composable
fun litBy(interaction: InteractionSource): State<Float> {
    val focused by interaction.collectIsFocusedAsState()
    return animateFloatAsState(if (focused) 1f else 0f, calm(), label = "lit")
}

/** Grows by [growth] when lit. */
fun Modifier.grows(lit: () -> Float, growth: Float = FOCUS_GROWTH) = graphicsLayer {
    val scale = 1f + (growth - 1f) * lit()
    scaleX = scale
    scaleY = scale
}

/** How far past the edge a rim reaches, in pixels: the one the edge's
 *  smoothing shares with what is under the piece. */
private const val BLEED = 1f

/** A rim drawn inside the piece's edge, as the web's inset ring: the edge
 *  itself stroked twice as wide and cut a pixel outside it, so it follows
 *  every curve of the shape exactly and covers the edge's last pixels,
 *  where the piece's own ground would otherwise show through as a thin
 *  line round it. */
fun Modifier.rim(shape: Shape, color: Color, width: Dp, lit: () -> Float = { 1f }) = drawWithCache {
    val outline = shape.createOutline(size, layoutDirection, this)
    val edge = Path().apply {
        addOutline(shape.createOutline(Size(size.width + 2 * BLEED, size.height + 2 * BLEED), layoutDirection, this@drawWithCache))
        translate(Offset(-BLEED, -BLEED))
    }
    val stroke = Stroke(width.toPx() * 2)
    onDrawWithContent {
        drawContent()
        val shown = lit()
        if (shown > 0f) {
            clipPath(edge) { drawOutline(outline, color, alpha = shown, style = stroke) }
        }
    }
}
