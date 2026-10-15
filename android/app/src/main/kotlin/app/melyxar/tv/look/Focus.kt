package app.melyxar.tv.look

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.dropShadow
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
 * The web's halo of a card: `0 0 8px accent 55%, 0 0 26px 2px accent 30%`,
 * with the raised shadow under it.
 */
fun Modifier.halo(shape: Shape, accent: Color, lit: () -> Float) = this
    .dropShadow(shape) {
        radius = 40.dp.toPx()
        offset = androidx.compose.ui.geometry.Offset(0f, 14.dp.toPx())
        color = Color.Black
        alpha = 0.45f * lit()
    }
    .dropShadow(shape) {
        radius = 26.dp.toPx()
        spread = 2.dp.toPx()
        color = accent
        alpha = 0.30f * lit()
    }
    .dropShadow(shape) {
        radius = 8.dp.toPx()
        color = accent
        alpha = 0.55f * lit()
    }

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
