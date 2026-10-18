package app.melyxar.tv.look

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.addOutline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.InspectorInfo
import androidx.compose.ui.unit.Dp
import kotlinx.coroutines.launch

/*
 * What the web does on hover, done when the remote comes to a piece: it
 * grows a little and its rim lights in the accent; a card also wears the
 * web's halo (`Shadows.kt`). Every one of these reads how lit the piece is
 * while it draws, so the easing in and out never composes the piece again.
 */

/** How lit a piece is, from 0 at rest to 1 when the remote is on it, eased
 *  the way the web eases its hover. The remote coming or going starts the
 *  easing straight from [interaction], without composing the piece again
 *  first. */
@Composable
fun litBy(interaction: InteractionSource): State<Float> {
    val lit = remember(interaction) { Animatable(0f) }
    LaunchedEffect(interaction) {
        val focuses = mutableListOf<FocusInteraction.Focus>()
        interaction.interactions.collect { said ->
            when (said) {
                is FocusInteraction.Focus -> focuses += said
                is FocusInteraction.Unfocus -> focuses -= said.focus
                else -> return@collect
            }
            val target = if (focuses.isEmpty()) 0f else 1f
            launch { lit.animateTo(target, calm()) }
        }
    }
    return lit.asState()
}

/** Whether [lit] is past [over]: what reads it is composed again only as
 *  the piece crosses it, not on every frame of the easing. */
@Composable
fun litPast(lit: () -> Float, over: Float = 0.5f): Boolean {
    val past by remember(lit, over) { derivedStateOf { lit() > over } }
    return past
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
 *  line round it. Its paths are kept while the piece keeps its size. */
fun Modifier.rim(shape: Shape, color: Color, width: Dp, lit: () -> Float = Whole) = this then RimElement(shape, color, width, lit)

private data class RimElement(val shape: Shape, val color: Color, val width: Dp, val lit: () -> Float) : ModifierNodeElement<RimNode>() {
    override fun create() = RimNode(shape, color, width, lit)

    override fun update(node: RimNode) = node.update(shape, color, width, lit)

    override fun InspectorInfo.inspectableProperties() {
        name = "rim"
    }
}

private class RimNode(
    private var shape: Shape,
    private var color: Color,
    private var width: Dp,
    private var lit: () -> Float,
) : Modifier.Node(), DrawModifierNode {
    private val made = MadeFor()
    private var outline: Outline? = null
    private val edge = Path()
    private var stroke = Stroke()

    fun update(shape: Shape, color: Color, width: Dp, lit: () -> Float) {
        if (shape != this.shape || width != this.width) {
            this.shape = shape
            this.width = width
            made.forget()
        }
        this.color = color
        this.lit = lit
        invalidateDraw()
    }

    override fun ContentDrawScope.draw() {
        if (made.isStale(this)) {
            outline = shape.createOutline(size, layoutDirection, this)
            edge.rewind()
            edge.addOutline(shape.createOutline(Size(size.width + 2 * BLEED, size.height + 2 * BLEED), layoutDirection, this))
            edge.translate(Offset(-BLEED, -BLEED))
            stroke = Stroke(width.toPx() * 2)
        }
        drawContent()
        val shown = lit()
        if (shown > 0f) {
            clipPath(edge) { drawOutline(outline!!, color, alpha = shown, style = stroke) }
        }
    }
}
