package app.melyxar.tv.look

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** How large a list of choices is drawn: small over a film, where it must
 *  leave the picture seen, large as a window of its own over a screen of
 *  settings, read from across a room. */
private class Scale(
    val narrow: Dp,
    val wide: Dp,
    val tallest: Dp,
    val rim: Dp,
    val title: TextUnit,
    val name: TextUnit,
    val value: TextUnit,
    val note: TextUnit,
    val glyph: Dp,
    val across: Dp,
    val down: Dp,
)

private val SMALL = Scale(320.dp, 420.dp, 360.dp, 8.dp, 18.sp, 14.sp, 13.sp, 13.sp, 18.dp, 10.dp, 9.dp)
private val LARGE = Scale(620.dp, 760.dp, 560.dp, 10.dp, 22.sp, 19.sp, 17.sp, 16.sp, 24.dp, 20.dp, 16.dp)

/**
 * A list of [choices] over the screen, under its [title], the one in force
 * ticked, as the web's menus: the remote starts on the one in force and
 * stays in the list until it chooses, which closes it, or until the back
 * key does.
 *
 * A list of settings rather than of choices says what each is on in
 * [values], none for a line among them that is a choice, starts where the remote last left it, [startAt], and stays open
 * when one is chosen, [leadsOn], since choosing it opens its own list. A
 * [note] says what the choices are for; a list with none says only that.
 * Colours to choose from show each its own in [swatches], and a menu its
 * choices' icons in [icons].
 */
@Composable
fun ChoiceList(
    title: String,
    choices: List<String>,
    chosen: Int?,
    onChoose: (Int) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    values: List<String?> = emptyList(),
    note: String? = null,
    startAt: Int = chosen ?: 0,
    leadsOn: Boolean = false,
    swatches: List<Color> = emptyList(),
    icons: List<ImageVector?> = emptyList(),
    large: Boolean = false,
) {
    val palette = LocalPalette.current
    val scale = if (large) LARGE else SMALL
    val start = remember { FocusRequester() }
    val first = startAt.coerceIn(0, (choices.size - 1).coerceAtLeast(0))
    BackHandler(onBack = onClose)
    Column(
        modifier = modifier
            .width(if (values.all { it == null }) scale.narrow else scale.wide)
            .focusProperties { onExit = { cancelFocusChange() } }
            .then(if (choices.isEmpty()) Modifier.focusRequester(start).focusable() else Modifier.focusGroup())
            .window()
            .padding(scale.rim),
    ) {
        Text(
            title,
            fontSize = scale.title,
            fontWeight = FontWeight.Bold,
            color = palette.text,
            modifier = Modifier.padding(horizontal = scale.across, vertical = scale.down * 0.9f),
        )
        note?.let {
            Text(
                it,
                fontSize = scale.note,
                lineHeight = scale.note * 1.45f,
                color = palette.textSoft,
                modifier = Modifier.padding(horizontal = scale.across, vertical = scale.down),
            )
        }
        LazyColumn(
            state = rememberLazyListState(initialFirstVisibleItemIndex = first),
            modifier = Modifier.heightIn(max = scale.tallest),
        ) {
            itemsIndexed(choices) { at, name ->
                Choice(
                    name,
                    value = values.getOrNull(at),
                    swatch = swatches.getOrNull(at),
                    icon = icons.getOrNull(at),
                    chosen = at == chosen,
                    scale = scale,
                    onClick = {
                        onChoose(at)
                        if (!leadsOn) onClose()
                    },
                    modifier = if (at == first) Modifier.focusRequester(start) else Modifier,
                )
            }
        }
    }
    LaunchedEffect(Unit) { start.requestFocus() }
}

@Composable
private fun Choice(
    name: String,
    value: String?,
    swatch: Color?,
    icon: ImageVector?,
    chosen: Boolean,
    scale: Scale,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val bright = litPast({ lit })
    val shape = RoundedCornerShape(8.dp)
    Row(
        modifier = modifier
            .padding(top = 2.dp)
            .fillMaxWidth()
            .clip(shape)
            .drawBehind { drawRect(palette.accent.copy(alpha = 0.26f * lit)) }
            .border(1.dp, palette.accent.copy(alpha = 0.45f * lit), shape)
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = scale.across, vertical = scale.down),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // A setting says what it is on rather than being ticked.
        if (value == null) {
            Box(Modifier.width(scale.glyph + 8.dp), contentAlignment = Alignment.CenterStart) {
                when {
                    chosen -> Glyph(Icons.Tick, palette.accentBright, scale.glyph)
                    icon != null -> Glyph(icon, if (bright) palette.text else palette.textSoft, scale.glyph)
                }
            }
        }
        swatch?.let {
            Box(
                Modifier
                    .padding(end = 10.dp)
                    .size(scale.glyph - 2.dp)
                    .clip(CircleShape)
                    .background(it)
                    .border(1.dp, palette.text.copy(alpha = 0.3f), CircleShape),
            )
        }
        Text(
            name,
            fontSize = scale.name,
            color = palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        value?.let {
            Text(
                it,
                fontSize = scale.value,
                color = palette.textFaint,
                maxLines = 1,
                modifier = Modifier.padding(start = 12.dp),
            )
            Glyph(Icons.ChevronRight, palette.textFaint, scale.glyph - 2.dp, Modifier.padding(start = 4.dp))
        }
    }
}

/**
 * A list of choices as a window of its own, for a screen of settings: the
 * screen behind it darkened, the list large in its middle.
 */
@Composable
fun ChoiceWindow(
    title: String,
    choices: List<String>,
    chosen: Int?,
    onChoose: (Int) -> Unit,
    onClose: () -> Unit,
    swatches: List<Color> = emptyList(),
) {
    val palette = LocalPalette.current
    Box(Modifier.fillMaxSize().background(palette.ground.copy(alpha = 0.72f)), contentAlignment = Alignment.Center) {
        ChoiceList(title, choices, chosen, onChoose, onClose, swatches = swatches, large = true)
    }
}
