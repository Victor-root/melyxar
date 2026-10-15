package app.melyxar.tv.inside

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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Card
import app.melyxar.server.Library
import app.melyxar.server.Refusal
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.WideWidth
import java.io.IOException

/** The cards of a grid read so far, and where the next page starts. */
private data class Pages(val cards: List<Card>, val next: String?)

/** A page of a grid, and the most the server gives at once. */
private const val PAGE = 60
private const val MOST = 200

/** How near the end of what is read the remote comes before the next page
 *  is asked for, in cards. */
private const val AHEAD = 18

/**
 * A library, its works in a grid as the web shows them, read a page at a
 * time as the remote goes down, well before it gets there.
 */
@Composable
fun LibraryScreen(library: Library, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val api = LocalServer.current
    val answers = LocalAnswers.current
    val signedOut = LocalSignedOut.current
    val question = "library:${library.id}"
    @Suppress("UNCHECKED_CAST")
    var pages by remember { mutableStateOf(answers[question] as Pages?) }
    var failed by remember { mutableStateOf(false) }
    var tries by remember { mutableIntStateOf(0) }
    var reached by remember { mutableIntStateOf(0) }
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val lying = library.kind == "home_media"

    suspend fun read(after: String?, limit: Int, adding: List<Card>) {
        try {
            val page = api.works(library.id, after, limit)
            pages = Pages(adding + page.cards, page.next).also { answers[question] = it }
            failed = false
        } catch (refused: Refusal) {
            if (refused.signedOut) signedOut() else failed = pages == null
        } catch (_: IOException) {
            failed = pages == null
        }
    }

    // Read again from the top, as much as was read before, when the screen
    // comes on: what was watched meanwhile shows on its card.
    LaunchedEffect(library.id, comebacks, tries) {
        read(null, (pages?.cards?.size ?: PAGE).coerceIn(PAGE, MOST), emptyList())
    }
    val shown = pages
    LaunchedEffect(reached, shown?.next) {
        if (shown != null && shown.next != null && reached >= shown.cards.size - AHEAD) {
            read(shown.next, PAGE, shown.cards)
        }
    }

    val grid = rememberLazyGridState()
    Box(Modifier.fillMaxSize()) {
        when {
            shown != null -> ScrollingPage(toTop = { grid.animateScrollToItem(0) }) { head ->
                LazyVerticalGrid(
                    columns = GridCells.FixedSize(if (lying) WideWidth else PosterWidth),
                    state = grid,
                    contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = 40.dp),
                    horizontalArrangement = Arrangement.spacedBy(CardGap),
                    verticalArrangement = Arrangement.spacedBy(22.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    item(key = "top", span = { GridItemSpan(maxLineSpan) }) {
                        Column(Modifier.head(head, "top")) {
                            // The grid keeps the page's margin already.
                            TopBar(edges = 0.dp)
                            Row(
                                modifier = Modifier.padding(top = 26.dp, bottom = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                Glyph(Icons.ofKind(library.kind), palette.accentBright, 26.dp)
                                Text(library.name, fontSize = 28.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, color = palette.text)
                                Text("${library.works}", fontSize = 16.sp, color = palette.textFaint, modifier = Modifier.padding(start = 4.dp))
                            }
                        }
                    }
                    if (shown.cards.isEmpty()) {
                        item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                            Text(stringResource(R.string.tv_empty_library), fontSize = 16.sp, color = palette.textSoft)
                        }
                    }
                    itemsIndexed(shown.cards, key = { _, card -> card.id }) { at, card ->
                        WorkCard(
                            card = card,
                            lying = lying,
                            onClick = { chrome.go(Place.Opened(card.id)) },
                            modifier = Modifier
                                .remembered(memory, card.id)
                                .then(if (at == 0) Modifier.focusRequester(first) else Modifier)
                                .onFocusChanged {
                                    if (it.isFocused) reached = at
                                    // The first line of cards is part of the head,
                                    // the line under the name of the library.
                                    val line = grid.layoutInfo.visibleItemsInfo.firstOrNull { item -> item.key == card.id }?.row
                                    head.hold(card.id, it.isFocused && line == 1)
                                },
                        )
                    }
                }
            }
            failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    RestoreFocus(memory, first, ready = shown != null)
}
