package app.melyxar.tv.look

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch

/** How far one press of an arrow scrolls a long text. */
private const val STEP_PX = 180f

/**
 * A long text, an overview or a biography, cut after [maxLines] as the web
 * cuts it. When it is cut, the remote can come to it, and a press opens it
 * whole over the screen, where the arrows scroll it and the back key closes
 * it, as the web's "More" does.
 */
@Composable
fun Synopsis(
    text: String,
    title: String,
    maxLines: Int,
    modifier: Modifier = Modifier,
    fontSize: TextUnit = 15.5.sp,
    lineHeight: TextUnit = 25.sp,
) {
    val palette = LocalPalette.current
    var cut by remember(text) { mutableStateOf(false) }
    var reading by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val bright = litPast({ lit })
    val shape = RoundedCornerShape(8.dp)
    Text(
        text,
        fontSize = fontSize,
        lineHeight = lineHeight,
        color = if (bright) palette.text else palette.textSoft,
        maxLines = maxLines,
        overflow = TextOverflow.Ellipsis,
        onTextLayout = { cut = it.hasVisualOverflow },
        // Read when the remote comes, so a text found cut on its first
        // layout can be come to before the screen is drawn again.
        modifier = modifier
            .clip(shape)
            .drawBehind { drawRect(palette.accent.copy(alpha = 0.14f * lit)) }
            .rim(shape, palette.accentBright, 1.5.dp) { lit }
            .focusProperties { canFocus = cut }
            .clickable(interaction, indication = null) { reading = cut },
    )
    if (reading) {
        Dialog(onDismissRequest = { reading = false }) {
            Whole(text, title)
        }
    }
}

/** The text whole, under its title, scrolled by the arrows. */
@Composable
private fun Whole(text: String, title: String) {
    val palette = LocalPalette.current
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    val held = remember { FocusRequester() }
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier = Modifier
            .width(640.dp)
            .boxShadow(shape, palette.shadowRaised)
            .clip(shape)
            .background(palette.surfaceRaised)
            .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
            .padding(28.dp),
    ) {
        Text(title, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
        Text(
            text,
            fontSize = 16.sp,
            lineHeight = 26.sp,
            color = palette.textSoft,
            modifier = Modifier
                .padding(top = 14.dp)
                .heightIn(max = 380.dp)
                .focusRequester(held)
                .onPreviewKeyEvent { event ->
                    val by = when (event.key) {
                        Key.DirectionDown -> STEP_PX
                        Key.DirectionUp -> -STEP_PX
                        else -> return@onPreviewKeyEvent false
                    }
                    if (event.type == KeyEventType.KeyDown) {
                        scope.launch { scroll.animateScrollBy(by) }
                    }
                    true
                }
                .focusable()
                .verticalScroll(scroll),
        )
    }
    LaunchedEffect(Unit) { held.requestFocus() }
}
