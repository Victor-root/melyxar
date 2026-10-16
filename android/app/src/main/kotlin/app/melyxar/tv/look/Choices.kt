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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The tallest a list of choices grows before it scrolls. */
private val TALLEST = 360.dp

/**
 * A list of [choices] over the screen, under its [title], the one in force
 * ticked, as the web's menus: the remote starts on the one in force and
 * stays in the list until it chooses, which closes it, or until the back
 * key does.
 *
 * A list of settings rather than of choices says what each is on in
 * [values], starts where the remote last left it, [startAt], and stays open
 * when one is chosen, [leadsOn], since choosing it opens its own list. A
 * [note] says what the choices are for; a list with none says only that.
 * Colours to choose from show each its own in [swatches].
 */
@Composable
fun ChoiceList(
    title: String,
    choices: List<String>,
    chosen: Int?,
    onChoose: (Int) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    values: List<String> = emptyList(),
    note: String? = null,
    startAt: Int = chosen ?: 0,
    leadsOn: Boolean = false,
    swatches: List<Color> = emptyList(),
) {
    val palette = LocalPalette.current
    val start = remember { FocusRequester() }
    val shape = RoundedCornerShape(10.dp)
    val first = startAt.coerceIn(0, (choices.size - 1).coerceAtLeast(0))
    BackHandler(onBack = onClose)
    Column(
        modifier = modifier
            .width(if (values.isEmpty()) 320.dp else 420.dp)
            .focusProperties { onExit = { cancelFocusChange() } }
            .then(if (choices.isEmpty()) Modifier.focusRequester(start).focusable() else Modifier.focusGroup())
            .boxShadow(shape, palette.shadowRaised)
            .clip(shape)
            .background(palette.surfaceRaised)
            .background(palette.accent.copy(alpha = 0.14f))
            .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
            .padding(6.dp),
    ) {
        Text(
            title,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
        )
        Box(Modifier.fillMaxWidth().height(1.dp).background(palette.border))
        note?.let {
            Text(
                it,
                fontSize = 13.sp,
                lineHeight = 19.sp,
                color = palette.textSoft,
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 10.dp),
            )
        }
        LazyColumn(
            state = rememberLazyListState(initialFirstVisibleItemIndex = first),
            modifier = Modifier.heightIn(max = TALLEST),
        ) {
            itemsIndexed(choices) { at, name ->
                Choice(
                    name,
                    value = values.getOrNull(at),
                    swatch = swatches.getOrNull(at),
                    chosen = at == chosen,
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
private fun Choice(name: String, value: String?, swatch: Color?, chosen: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(8.dp)
    Row(
        modifier = modifier
            .padding(top = 2.dp)
            .fillMaxWidth()
            .clip(shape)
            .background(palette.accent.copy(alpha = 0.26f * lit))
            .border(1.dp, palette.accent.copy(alpha = 0.45f * lit), shape)
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // A setting says what it is on rather than being ticked.
        if (value == null) {
            Box(Modifier.width(26.dp), contentAlignment = Alignment.CenterStart) {
                if (chosen) Glyph(Icons.Tick, palette.accentBright, 18.dp)
            }
        }
        swatch?.let {
            Box(
                Modifier
                    .padding(end = 10.dp)
                    .size(16.dp)
                    .clip(CircleShape)
                    .background(it)
                    .border(1.dp, palette.text.copy(alpha = 0.3f), CircleShape),
            )
        }
        Text(
            name,
            fontSize = 14.sp,
            color = if (lit > 0.5f) palette.accentBright else palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        value?.let {
            Text(
                it,
                fontSize = 13.sp,
                color = palette.textFaint,
                maxLines = 1,
                modifier = Modifier.padding(start = 12.dp),
            )
            Glyph(Icons.ChevronRight, palette.textFaint, 16.dp, Modifier.padding(start = 4.dp))
        }
    }
}
