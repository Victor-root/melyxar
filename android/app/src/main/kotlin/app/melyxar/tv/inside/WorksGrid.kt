package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Api
import app.melyxar.server.Browse
import app.melyxar.server.Card
import app.melyxar.server.Refusal
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Gutter
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

    /** Reads the grid again from its top, as far as it had been read: what
     *  was watched meanwhile shows on its card. */
    suspend fun refresh() {
        read(null, (cards?.size ?: PAGE).coerceIn(PAGE, MOST), emptyList())
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

    private suspend fun read(after: String?, limit: Int, adding: List<Card>): Boolean = try {
        val page = api.works(browse, after, limit)
        val pages = Pages(adding + page.cards, page.next)
        answers[question] = pages
        next = pages.next
        cards = pages.cards
        failed = false
        true
    } catch (refused: Refusal) {
        if (refused.signedOut) signedOut() else failed = cards == null
        false
    } catch (_: IOException) {
        failed = cards == null
        false
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
    top: @Composable (PageHead) -> Unit,
) {
    var reached by remember(works) { mutableIntStateOf(0) }
    LaunchedEffect(works, reached, works.cards) { works.reachedTo(reached) }
    if (works.failed) {
        Trouble(onRetry = works::retry)
        return
    }
    CardGrid(
        cards = works.cards?.filter(belongs),
        lying = lying,
        empty = empty,
        memory = memory,
        grid = grid,
        onOpen = onOpen,
        modifier = modifier,
        firstCard = firstCard,
        onReach = { at, card ->
            reached = at
            onReach(card)
        },
        besides = besides,
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
                itemsIndexed(cards.orEmpty(), key = { _, card -> card.id }) { at, card ->
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
        }
    }
}

/**
 * A piece of a grid whose head is its first item: on the first line under
 * the head, the piece is part of the head, which stays shown whole while the
 * remote is on it.
 */
fun Modifier.firstLineHoldsTheHead(grid: LazyGridState, head: PageHead, key: String): Modifier = onFocusChanged {
    val line = grid.layoutInfo.visibleItemsInfo.firstOrNull { item -> item.key == key }?.row
    head.hold(key, it.isFocused && line == 1)
}
