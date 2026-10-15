package app.melyxar.tv.look

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.Stroke
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
 * A glow of [color] round the piece, as high as [elevation] times
 * [strength]: drawn by the graphics card from the piece's outline, as the
 * system draws its shadows, so nothing is blurred on the thread that draws
 * the screen and nothing is drawn again while the strength eases. A blurred
 * shadow is painted into a picture of its own on that thread, three for each
 * piece the remote came to, which a television's processor cannot keep up
 * with.
 */
fun Modifier.glow(shape: Shape, color: Color, elevation: Dp, strength: () -> Float = { 1f }) = graphicsLayer {
    shadowElevation = elevation.toPx() * strength()
    this.shape = shape
    clip = false
    ambientShadowColor = color
    spotShadowColor = color
}

/** The web's halo of a card, in the accent, as lit as the piece is. */
fun Modifier.halo(shape: Shape, accent: Color, lit: () -> Float) = glow(shape, accent, HALO_HEIGHT, lit)

/** How high the halo of a lit piece stands. */
private val HALO_HEIGHT = 22.dp

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
