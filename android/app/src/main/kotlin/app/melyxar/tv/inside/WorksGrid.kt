package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Api
import app.melyxar.server.Browse
import app.melyxar.server.Card
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

/** How far a grid is read on to reach a card asked for, as the web reads it. */
private const val FURTHEST = 2000

/**
 * A grid of works as the server gives it for [browse], read a page at a time
 * as the remote goes down, well before it gets there. Its last answer is kept
 * while another screen is over it, and shown at once on coming back.
 */
@Stable
class Works(
    private val api: Api,
    private val browse: Browse,
    private val answers: MutableMap<String, Any?>,
    private val signedOut: () -> Unit,
) {
    private val question = "works:$browse"

    @Suppress("UNCHECKED_CAST")
    var cards by mutableStateOf((answers[question] as Pages?)?.cards)
        private set
    private var next: String? = (answers[question] as Pages?)?.next

    var failed by mutableStateOf(false)
        private set

    /** How many times the grid was asked for again after failing. */
    var tries by mutableIntStateOf(0)
        private set

    fun retry() {
        tries += 1
    }

    /** Reads the grid again from its top, as far as it had been read, up to
     *  [FURTHEST]: what was watched meanwhile shows on its card, and the
     *  card the remote is on stays in the grid. Shown once read whole. */
    suspend fun refresh() {
        val wanted = (cards?.size ?: PAGE).coerceIn(PAGE, FURTHEST)
        var fresh = emptyList<Card>()
        var after: String? = null
        do {
            val page = ask { api.works(browse, after, minOf(MOST, wanted - fresh.size)) } ?: return
            fresh = fresh + page.cards
            after = page.next
        } while (after != null && page.cards.isNotEmpty() && fresh.size < wanted)
        show(Pages(fresh, after))
    }

    /** The remote is on the card at [at]: the next page is read when it
     *  comes near the end of what is. */
    suspend fun reachedTo(at: Int) {
        val read = cards ?: return
        val after = next ?: return
        if (at >= read.size - AHEAD) {
            read(after, PAGE, read)
        }
    }

    /** Reads on until the card [id] is among those read, up to [FURTHEST];
     *  whether it now is. */
    suspend fun readUntil(id: String): Boolean {
        while (true) {
            val read = cards ?: return false
            if (read.any { it.id == id }) {
                return true
            }
            val after = next ?: return false
            if (read.size >= FURTHEST || !read(after, MOST, read)) {
                return false
            }
        }
    }

    private suspend fun read(after: String?, limit: Int, adding: List<Card>): Boolean {
        val page = ask { api.works(browse, after, limit) } ?: return false
        show(Pages(adding + page.cards, page.next))
        return true
    }

    private fun show(pages: Pages) {
        answers[question] = pages
        next = pages.next
        cards = pages.cards
        failed = false
    }

    /** What the server answers, or nothing when it does not. */
    private suspend fun <T> ask(asking: suspend () -> T): T? = try {
        asking()
    } catch (refused: Refusal) {
        if (refused.signedOut) signedOut() else failed = cards == null
        null
    } catch (_: IOException) {
        failed = cards == null
        null
    }
}

/** The grid [browse] asks for, read again whenever the screen comes on or
 *  what the server's grids hold moved; not asked for at all while [asking]
 *  is false. */
@Composable
fun rememberWorks(browse: Browse, comebacks: Int, asking: Boolean = true): Works {
    val api = LocalServer.current
    val answers = LocalAnswers.current
    val signedOut = LocalSignedOut.current
    val moved = LocalMarks.current.rowsMoved
    val works = remember(api, browse) { Works(api, browse, answers, signedOut) }
    LaunchedEffect(works, comebacks, moved, works.tries, asking) {
        if (asking) works.refresh()
    }
    return works
}

/**
 * The cards of [works] in a grid, as the web lays them, read on a page at a
 * time as the remote goes down. [firstCard] is given to the first card, for
 * a screen that starts the remote there.
 */
@Composable
fun WorksGrid(
    works: Works,
    lying: Boolean,
    empty: String,
    memory: FocusMemory,
    grid: LazyGridState,
    onOpen: (Card) -> Unit,
    modifier: Modifier = Modifier,
    firstCard: FocusRequester? = null,
    /** Told of the card the remote comes to. */
    onReach: (Card) -> Unit = {},
    /** Whether a card still has its place, for a grid made of what a mark
     *  says: what no longer does leaves it as soon as it is said. */
    belongs: (Card) -> Boolean = { true },
    /** What stands between the head and the cards. */
    besides: LazyGridScope.(PageHead) -> Unit = {},
    /** Where the letter last jumped to begins, marked in the grid. */
    startsAt: LetterStart? = null,
    top: @Composable (PageHead) -> Unit,
) {
    val reach = rememberReaching(works, works.cards, works::reachedTo)
    // Filtered again only when the cards or what [belongs] reads change, not
    // on every composition of the grid.
    val belonging by rememberUpdatedState(belongs)
    val shown by remember(works) { derivedStateOf { works.cards?.filter(belonging) } }
    if (works.failed) {
        Trouble(onRetry = works::retry)
        return
    }
    CardGrid(
        cards = shown,
        lying = lying,
        empty = empty,
        memory = memory,
        grid = grid,
        onOpen = onOpen,
        modifier = modifier,
        firstCard = firstCard,
        onReach = { at, card ->
            reach(at)
            onReach(card)
        },
        besides = besides,
        startsAt = startsAt,
        top = top,
    )
}

/**
 * [cards] in a grid, as the web lays them: [top] above them, which stays
 * shown whole with the first line of cards while the remote is in them, and
 * the page scrolling below that; [empty] said when there are none.
 */
@Composable
fun CardGrid(
    cards: List<Card>?,
    lying: Boolean,
    empty: String,
    memory: FocusMemory,
    grid: LazyGridState,
    onOpen: (Card) -> Unit,
    modifier: Modifier = Modifier,
    firstCard: FocusRequester? = null,
    /** What is written under a card, its year unless said otherwise. */
    noteOf: (Card) -> String? = { it.year?.toString() },
    /** Told of the card the remote comes to, and where it stands. */
    onReach: (Int, Card) -> Unit = { _, _ -> },
    /** What stands between the head and the cards. */
    besides: LazyGridScope.(PageHead) -> Unit = {},
    /** Where the letter last jumped to begins, marked in the grid. */
    startsAt: LetterStart? = null,
    top: @Composable (PageHead) -> Unit,
) {
    val palette = LocalPalette.current
    Box(modifier.fillMaxSize()) {
        ScrollingPage(toTop = { grid.animateScrollToItem(0) }) { head ->
            LazyVerticalGrid(
                columns = GridCells.Adaptive(if (lying) WideWidth else PosterWidth),
                state = grid,
                contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = 40.dp),
                horizontalArrangement = Arrangement.spacedBy(CardGap),
                verticalArrangement = Arrangement.spacedBy(22.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                item(key = "top", span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.head(head, "top")) { top(head) }
                }
                besides(head)
                if (cards != null && cards.isEmpty()) {
                    item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                        Text(empty, fontSize = 16.sp, color = palette.textSoft, modifier = Modifier.padding(top = 8.dp))
                    }
                }
                val shown = cards.orEmpty()
                // The cards before where the letter begins, its mark, and the
                // rest: two runs, each card still counted from the first.
                val marked = startsAt?.let { starts -> shown.indexOfFirst { it.id == starts.work } }?.takeIf { it >= 0 } ?: shown.size
                fun LazyGridScope.cardsFrom(from: Int, until: Int) {
                    itemsIndexed(shown.subList(from, until), key = { _, card -> card.id }) { onRun, card ->
                        val at = from + onRun
                        WorkCard(
                            card = card,
                            lying = lying,
                            note = noteOf(card),
                            onClick = { onOpen(card) },
                            modifier = Modifier
                                .remembered(memory, card.id)
                                .then(if (at == 0 && firstCard != null) Modifier.focusRequester(firstCard) else Modifier)
                                .onFocusChanged { if (it.isFocused) onReach(at, card) }
                                .firstLineHoldsTheHead(grid, head, card.id),
                        )
                    }
                }
                cardsFrom(0, marked)
                startsAt?.takeIf { marked < shown.size }?.let { starts ->
                    item(key = "starts:${starts.work}") { LetterStarts(starts.letter, lying) }
                }
                cardsFrom(marked, shown.size)
            }
        }
    }
}

/** Where a letter of the titles begins in a grid: the first work under
 *  [letter]. */
data class LetterStart(val work: String, val letter: String)

/**
 * Where the letter just jumped to begins, standing in the grid as one more
 * card, as the web marks it: the row it opens may still end the letter
 * before. Its letter and an arrow pointing on to the first of its works,
 * inside a dashed frame of the accent. Nothing the remote stops on.
 */
@Composable
private fun LetterStarts(letter: String, lying: Boolean) {
    val palette = LocalPalette.current
    val frame = frameOf(lying)
    Row(
        modifier = Modifier
            .size(frame)
            .drawBehind {
                val line = 2.dp.toPx()
                val corner = 10.dp.toPx()
                drawRoundRect(
                    palette.accent.copy(alpha = 0.6f),
                    topLeft = Offset(line / 2, line / 2),
                    size = Size(size.width - line, size.height - line),
                    cornerRadius = CornerRadius(corner),
                    style = Stroke(line, pathEffect = PathEffect.dashPathEffect(floatArrayOf(3 * line, 2 * line))),
                )
            },
        horizontalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(letter.uppercase(), fontSize = 44.sp, fontWeight = FontWeight.ExtraBold, color = palette.accentBright)
        Glyph(Icons.ChevronRight, palette.accentBright, 28.dp)
    }
}

/**
 * A piece of a grid whose head is its first item: on the first line under
 * the head, the piece is part of the head, which stays shown whole while the
 * remote is on it.
 */
@Composable
fun Modifier.firstLineHoldsTheHead(grid: LazyGridState, head: PageHead, key: String): Modifier = onFocusChanged {
    val line = grid.layoutInfo.visibleItemsInfo.firstOrNull { item -> item.key == key }?.row
    head.hold(key, it.isFocused && line == 1)
}
