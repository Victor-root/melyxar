package app.melyxar.tv.requests

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Found
import app.melyxar.server.Refusal
import app.melyxar.server.popularTitles
import app.melyxar.tv.inside.BarTitle
import app.melyxar.tv.inside.LocalSignedOut
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PosterWidth
import java.io.IOException

/** How near the end of what is read the remote comes before the next page
 *  is read, in titles. */
private const val READ_AHEAD = 12

/**
 * Many more titles of one row of suggestions: the most popular of a
 * catalogue, in a [genre] when one is named, a page at a time as far as the
 * provider goes, the next one read as the remote comes near the end.
 */
@Composable
fun MoreTitlesScreen(catalogue: String, genreId: String?, genre: String?) {
    val api = LocalServer.current
    val signedOut = LocalSignedOut.current
    val palette = LocalPalette.current
    val language = requestLanguage()
    val titles = remember { mutableStateListOf<Found>() }
    var page by remember { mutableIntStateOf(1) }
    var ended by remember { mutableStateOf(false) }
    var reading by remember { mutableStateOf(true) }
    var reached by remember { mutableIntStateOf(0) }
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val grid = rememberLazyGridState()
    LaunchedEffect(page) {
        reading = true
        try {
            val more = api.popularTitles(catalogue, genreId, page, language)
            val known = titles.map { it.tmdbId }.toSet()
            titles.addAll(more.filter { it.tmdbId !in known })
            ended = more.isEmpty()
        } catch (refused: Refusal) {
            if (refused.signedOut) signedOut()
            ended = true
        } catch (_: IOException) {
            ended = true
        }
        reading = false
    }
    // One page at a time: the next is asked once this one is in.
    LaunchedEffect(reached, reading, ended) {
        if (!reading && !ended && reached >= titles.size - READ_AHEAD) page += 1
    }
    ScrollingPage(toTop = { grid.animateScrollToItem(0) }) { head ->
        LazyVerticalGrid(
            columns = GridCells.Adaptive(PosterWidth),
            state = grid,
            contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = 40.dp),
            horizontalArrangement = Arrangement.spacedBy(CardGap),
            verticalArrangement = Arrangement.spacedBy(22.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item(key = "top", span = { GridItemSpan(maxLineSpan) }) {
                Column(Modifier.head(head, "top")) {
                    TopBar(inset = Gutter) {
                        BarTitle(if (catalogue == "series") Icons.Series else Icons.Film, shelfName(genre))
                    }
                    Row(Modifier.padding(top = 18.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(catalogueName(catalogue), fontSize = 15.sp, color = palette.textSoft)
                    }
                }
            }
            itemsIndexed(titles, key = { _, one -> one.tmdbId }) { at, one ->
                TitleTile(
                    one,
                    Modifier
                        .remembered(memory, one.tmdbId)
                        .then(if (at == 0) Modifier.focusRequester(first) else Modifier)
                        .onFocusChanged { if (it.hasFocus) reached = at },
                )
            }
        }
    }
    RestoreFocus(memory, first, ready = titles.isNotEmpty())
}
