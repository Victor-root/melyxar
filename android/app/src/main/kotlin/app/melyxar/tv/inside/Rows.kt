package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.SectionGap
import app.melyxar.tv.look.rememberRowScrolling

/** A row's heading, as the web's: its icon in the accent, then its name. */
@Composable
fun RowHead(title: String, icon: ImageVector?, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    Row(
        modifier = modifier.padding(start = Gutter, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        icon?.let { Glyph(it, palette.accentBright, 22.dp) }
        Text(title, fontSize = 21.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.01).em, color = palette.text)
    }
}

/**
 * A heading and its cards side by side, scrolled only once the remote
 * reaches the end of what is shown. Coming into the row from above or below
 * lands on the card it left, or on the first card the first time.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun <T> CardRow(
    title: String?,
    icon: ImageVector?,
    items: List<T>,
    key: (T) -> String,
    modifier: Modifier = Modifier,
    card: @Composable (T) -> Unit,
) {
    val scrolling = rememberRowScrolling()
    val first = remember { FocusRequester() }
    Column(modifier.padding(bottom = SectionGap)) {
        title?.let { RowHead(it, icon) }
        CompositionLocalProvider(LocalBringIntoViewSpec provides scrolling) {
            LazyRow(
                state = rememberLazyListState(),
                contentPadding = PaddingValues(horizontal = Gutter, vertical = 18.dp),
                horizontalArrangement = Arrangement.spacedBy(CardGap),
                modifier = Modifier.focusRestorer(first),
            ) {
                items(items.size, key = { key(items[it]) }) { at ->
                    Box(if (at == 0) Modifier.focusRequester(first) else Modifier) { card(items[at]) }
                }
            }
        }
    }
}

/** A section of a page, only when it has something to show. */
fun <T> LazyListScope.section(key: String, items: List<T>, content: @Composable () -> Unit) {
    if (items.isNotEmpty()) {
        item(key = key) { content() }
    }
}
