package app.melyxar.tv.look

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/*
 * The controls of the web interface (`styles/controls.css`, `work.css`), made
 * for the remote: what the web shows on hover is what the remote shows on the
 * piece it is on.
 */

/** How much a button grows under the remote, as `.button:hover`. */
private const val BUTTON_GROWTH = 1.04f

/** The large accent button, the one the hand goes to: Play, Resume. */
@Composable
fun AccentButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = Icons.Play,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = modifier
            .grows({ lit }, BUTTON_GROWTH)
            // `0 4px 16px var(--accent-glow)`, lit or not.
            .boxShadow(ControlShape, BoxShadow(palette.accentGlow, blur = 16.dp, y = 4.dp))
            .clip(ControlShape)
            .background(palette.accent)
            .rim(ControlShape, palette.accentContrast, 2.5.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 26.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        icon?.let { Glyph(it, palette.accentContrast, 20.dp) }
        Text(text, color = palette.accentContrast, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

/** The large plain button beside it. */
@Composable
fun PlainButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = modifier
            .grows({ lit }, BUTTON_GROWTH)
            .clip(ControlShape)
            .background(if (lit > 0.5f) palette.surfaceHover else palette.surfaceRaised)
            .border(1.dp, palette.border, ControlShape)
            .rim(ControlShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 26.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        icon?.let { Glyph(it, if (lit > 0.5f) palette.accentBright else palette.text, 20.dp) }
        Text(text, color = palette.text, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

/** A round mark of a work: watched, favourite, watch later. */
@Composable
fun RoundMark(
    icon: ImageVector,
    /** What it does, said by the system for somebody who does not see it. */
    label: String,
    on: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Box(
        modifier = modifier
            .size(48.dp)
            .grows({ lit }, 1.06f)
            .clip(CircleShape)
            .background(palette.surfaceRaised.copy(alpha = 0.74f))
            .border(1.dp, if (on) palette.accent.copy(alpha = 0.6f) else palette.border, CircleShape)
            .rim(CircleShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Glyph(icon, if (on || lit > 0.5f) palette.accentBright else palette.text, 22.dp)
    }
}

/** A word on what a file holds: 4K, HDR10, Dolby Atmos. */
@Composable
fun Badge(text: String, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(5.dp)
    Text(
        text = text,
        color = palette.textSoft,
        fontSize = 12.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = 0.04.em,
        maxLines = 1,
        overflow = TextOverflow.Clip,
        modifier = modifier
            .clip(shape)
            .background(palette.ground.copy(alpha = 0.45f))
            .border(1.dp, palette.text.copy(alpha = 0.3f), shape)
            .padding(horizontal = 8.dp, vertical = 2.dp),
    )
}

/** The glass of the bar's pieces: the accent at 22% over the raised surface
 *  at 55%, a rim of the accent and its halo always on. */
@Composable
fun Modifier.glassPiece(shape: Shape = PillShape): Modifier {
    val palette = LocalPalette.current
    return this
        .boxShadow(shape, palette.shadowCard, BoxShadow(palette.accentGlow, blur = 14.dp, spread = 2.dp))
        .clip(shape)
        .background(palette.surfaceRaised.copy(alpha = 0.55f))
        .background(palette.accent.copy(alpha = 0.22f))
        .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
}
