package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.dp
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Pivot

/** The widest a page read line by line stands, in the middle of the
 *  screen. */
private val ReadingWidth = 820.dp

/** What stands in the middle of a page read line by line, as wide as such
 *  a page is. */
@Composable
fun ReadingColumn(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Box(modifier.fillMaxWidth().padding(horizontal = Gutter), contentAlignment = Alignment.TopCenter) {
        Column(Modifier.widthIn(max = ReadingWidth).fillMaxWidth()) { content() }
    }
}

/** Where the piece the remote is on stands once a page scrolls, as a share
 *  of the screen's height. */
private const val PAGE_STANDS_AT = 0.3f

/** Which parts of a page's head hold the remote. */
@Stable
class PageHead {
    private val holding = mutableStateMapOf<String, Unit>()

    val held: Boolean get() = holding.isNotEmpty()

    fun hold(part: String, holds: Boolean) {
        if (holds) holding[part] = Unit else holding.remove(part)
    }
}

/** Marks a part of a page's head, [part] naming it among the others. */
fun Modifier.head(page: PageHead, part: String): Modifier = onFocusChanged { page.hold(part, it.hasFocus) }

/**
 * A page that scrolls under the remote, the same on every screen: its head,
 * what fits on the first screen, stays shown whole from the top while the
 * remote is in it, and the page scrolls once the remote goes below it, each
 * piece then brought to the same height of the screen.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ScrollingPage(toTop: suspend () -> Unit, content: @Composable (PageHead) -> Unit) {
    val head = remember { PageHead() }
    val held by remember { derivedStateOf { head.held } }
    val pivot = remember { Pivot(0f, PAGE_STANDS_AT) { held } }
    val goUp by rememberUpdatedState(toTop)
    LaunchedEffect(held) {
        if (held) goUp()
    }
    CompositionLocalProvider(LocalBringIntoViewSpec provides pivot) { content(head) }
}
