package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListPrefetchStrategy
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.SectionGap
import app.melyxar.tv.look.rememberRowScrolling
import kotlin.math.ceil

/** The room a row of cards keeps above and below them. */
private val ROW_ROOM = 18.dp

/** A row's heading, as the web's: its icon in the accent, then its name. */
@Composable
fun RowHead(title: String, icon: ImageVector?, modifier: Modifier = Modifier, inset: Dp = Gutter) {
    val palette = LocalPalette.current
    Row(
        modifier = modifier.padding(start = inset, bottom = 12.dp),
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
    val ahead = cardsAcross()
    val row = remember { RowMemory() }
    // The room the cards are given above and below, to grow into as the
    // remote comes, counts in the gap to what follows.
    Column(modifier.padding(bottom = SectionGap - ROW_ROOM)) {
        title?.let { RowHead(it, icon) }
        CompositionLocalProvider(LocalBringIntoViewSpec provides scrolling) {
            LazyRow(
                state = rememberLazyListState(prefetchStrategy = remember(ahead) { LazyListPrefetchStrategy(nestedPrefetchItemCount = ahead) }),
                contentPadding = PaddingValues(horizontal = Gutter, vertical = ROW_ROOM),
                horizontalArrangement = Arrangement.spacedBy(CardGap),
                modifier = Modifier.landsAsLeft(row, items.firstOrNull()?.let(key)),
            ) {
                items(items.size, key = { key(items[it]) }) { at ->
                    Box(Modifier.inRow(row, key(items[at]))) { card(items[at]) }
                }
            }
        }
    }
}

/** How many of the narrowest cards a screen shows across: as many of a row
 *  are composed ahead while the page brings it near, rather than in the
 *  frame that scrolls it in. */
@Composable
private fun cardsAcross(): Int {
    val across = with(LocalDensity.current) { LocalWindowInfo.current.containerSize.width.toDp() }
    return ceil((across - Gutter) / (PosterWidth + CardGap)).toInt().coerceAtLeast(1)
}

/** A section of a page, only when it has something to show. */
fun <T> LazyListScope.section(key: String, items: List<T>, content: @Composable () -> Unit) {
    if (items.isNotEmpty()) {
        item(key = key) { content() }
    }
}

/** A hairline of [accent] down the left edge, faded at both ends: the air
 *  between two rows sharing a line, as the web draws it. */
fun Modifier.hairlineBefore(accent: Color): Modifier = drawWithCache {
    val line = Brush.verticalGradient(
        0f to Color.Transparent,
        0.18f to accent.copy(alpha = 0.45f),
        0.82f to accent.copy(alpha = 0.45f),
        1f to Color.Transparent,
    )
    onDrawBehind { drawRect(line, size = Size(1.dp.toPx(), size.height)) }
}
