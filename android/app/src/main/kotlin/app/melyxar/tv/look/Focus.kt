package app.melyxar.tv.look

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/*
 * What the web does on hover, done when the remote comes to a piece: it
 * grows a little, its rim lights in the accent and a halo of the accent
 * shines around it. Every one of these reads how lit the piece is while it
 * draws, so the easing in and out never composes the piece again.
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

/**
 * A soft light of [color] round the piece, [reach] wide, as strong as [lit]
 * says, as the web's halo on hover: rings of the piece's own shape, each a
 * little wider and faint, drawn over one another so the light thickens
 * towards the edge. Laid out once per size, drawn by the graphics card, and
 * nothing at all while unlit; never a blurred shadow, which is painted into
 * a picture of its own on the thread that draws the screen.
 */
fun Modifier.halo(shape: Shape, color: Color, reach: Dp = HALO_REACH, lit: () -> Float = { 1f }) = drawWithCache {
    val rings = (1..HALO_RINGS).map { ring ->
        val spread = reach.toPx() * ring / HALO_RINGS
        spread to shape.createOutline(Size(size.width + 2 * spread, size.height + 2 * spread), layoutDirection, this)
    }
    onDrawBehind {
        val shown = lit()
        if (shown > 0f) {
            rings.forEach { (spread, outline) ->
                translate(-spread, -spread) { drawOutline(outline, color, alpha = HALO_RING_ALPHA * shown) }
            }
        }
    }
}

/** How far the halo of a lit piece reaches, in how many rings, each how
 *  strong: about half the colour at the piece's edge, fading out from it. */
private val HALO_REACH = 16.dp
private const val HALO_RINGS = 8
private const val HALO_RING_ALPHA = 0.075f

/** A rim drawn inside the piece's edge, as the web's inset ring. */
fun Modifier.rim(shape: Shape, color: Color, width: Dp, lit: () -> Float = { 1f }) = drawWithContent {
    drawContent()
    val shown = lit()
    if (shown > 0f) {
        val inset = width.toPx() / 2
        val outline = shape.createOutline(
            size.copy(width = size.width - 2 * inset, height = size.height - 2 * inset),
            layoutDirection,
            this,
        )
        drawContext.transform.translate(inset, inset)
        drawOutline(outline, color, alpha = shown, style = Stroke(width.toPx()))
        drawContext.transform.translate(-inset, -inset)
    }
}
