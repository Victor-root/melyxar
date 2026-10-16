package app.melyxar.tv.look

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/*
 * The web's `.browse-piece`: how a grid is read, on one piece of the glass
 * the bar at the top is made of, its choices set apart by hairlines.
 */

/** The height of what is pressed on a piece of glass. */
private val PRESS = 38.dp

/** A piece of glass holding presses side by side. */
@Composable
fun GlassBar(modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    Row(
        modifier = modifier.height(46.dp).glassPiece().padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        content = content,
    )
}

/** The hairline between two choices of a piece. */
@Composable
fun GlassRule() {
    Box(Modifier.padding(horizontal = 6.dp).width(1.dp).height(24.dp).background(LocalPalette.current.border))
}

/** What every press on the glass shares: tinted by the accent and ringed
 *  by it as the remote comes to it, filled with it when [on] says it is
 *  the one in force. */
@Composable
private fun Modifier.glassPress(shape: Shape, on: Boolean, interaction: MutableInteractionSource, lit: () -> Float, onClick: () -> Unit): Modifier {
    val palette = LocalPalette.current
    return this
        .height(PRESS)
        .clip(shape)
        .drawBehind { drawRect(if (on) palette.accent else palette.accent.copy(alpha = 0.22f * lit())) }
        .rim(shape, palette.accentBright, 2.dp, lit)
        .clickable(interaction, indication = null, onClick = onClick)
}

/** A choice of the piece: its name, faint, and what it is on, opening the
 *  list of what it can be; in the accent when [narrows] says it narrows
 *  what is shown. */
@Composable
fun GlassField(name: String, value: String, onClick: () -> Unit, modifier: Modifier = Modifier, narrows: Boolean = false) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = modifier.glassPress(PillShape, on = false, interaction, { lit }, onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(name, fontSize = 13.sp, color = palette.textFaint)
        val strong = if (narrows || lit > 0.5f) palette.accentBright else palette.text
        Text(value, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = strong, maxLines = 1)
        Glyph(Icons.ChevronDown, strong, 16.dp)
    }
}

/** A tab of the piece, filled with the accent when it is the one shown. */
@Composable
fun GlassTab(text: String, on: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Box(
        modifier = modifier.glassPress(PillShape, on, interaction, { lit }, onClick).padding(horizontal = 16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            fontSize = 15.sp,
            fontWeight = FontWeight.SemiBold,
            color = when {
                on -> palette.accentContrast
                lit > 0.5f -> palette.accentBright
                else -> palette.textSoft
            },
            maxLines = 1,
        )
    }
}

/** A round press of the piece, said by [label] to whoever does not see it. */
@Composable
fun GlassIcon(icon: ImageVector, label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Box(
        modifier = modifier
            .size(PRESS)
            .glassPress(CircleShape, on = false, interaction, { lit }, onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Glyph(icon, if (lit > 0.5f) palette.accentBright else palette.text, 20.dp)
    }
}
