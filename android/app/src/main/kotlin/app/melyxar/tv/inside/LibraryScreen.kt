package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Browse
import app.melyxar.server.Filters
import app.melyxar.server.Library
import app.melyxar.server.Refusal
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.litBy
import java.io.IOException
import kotlinx.coroutines.launch

/** The orders a grid can be read in, as the web offers them. */
private val ORDERS = listOf("title", "added_at", "release_year", "community_rating", "runtime")

/** Which list of choices is open over the grid. */
private enum class Picking { Order, Genre, Decade }

/**
 * A library, its works in a grid as the web shows them, with the web's sort
 * and filters under its name and, while it is read by title, its letters
 * down the side to jump to.
 */
@Composable
fun LibraryScreen(library: Library, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val api = LocalServer.current
    val scope = rememberCoroutineScope()
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    var order by rememberSaveable(library.id) { mutableStateOf("title") }
    var descending by rememberSaveable(library.id) { mutableStateOf(false) }
    var genre by rememberSaveable(library.id) { mutableStateOf<String?>(null) }
    var decade by rememberSaveable(library.id) { mutableStateOf<Int?>(null) }
    var picking by remember { mutableStateOf<Picking?>(null) }
    val browse = Browse(library = library.id, order = order, descending = descending, genre = genre, decade = decade)
    val works = rememberWorks(browse, comebacks)
    val filters = rememberLoaded("filters:${library.id}", comebacks) { filters(library.id) }.value.ready
    // A grid read another way starts again from its top.
    val grid = rememberSaveable(browse, saver = LazyGridState.Saver) { LazyGridState() }
    var at by remember { mutableStateOf<String?>(null) }

    /** Closes the list of choices, the remote back on what opened it. */
    fun closePicking() {
        picking = null
        scope.launch {
            withFrameNanos {}
            memory.restore()
        }
    }

    /** Brings the first work under [letter] to the remote, reading on to it
     *  as the web does rather than narrowing the grid to the letter. */
    fun jump(letter: String) {
        scope.launch {
            val opening = try {
                api.works(browse.copy(initial = letter), null, 1).cards.firstOrNull()
            } catch (_: Refusal) {
                null
            } catch (_: IOException) {
                null
            } ?: return@launch
            if (!works.readUntil(opening.id)) {
                return@launch
            }
            val index = works.cards.orEmpty().indexOfFirst { it.id == opening.id }
            // The grid's head comes before its cards.
            grid.scrollToItem(index + 1)
            withFrameNanos {}
            memory.requester(opening.id).requestFocus()
        }
    }

    val letters = filters?.initials.orEmpty().map { it.name }.takeIf { order == "title" && it.size > 1 }
    Box(Modifier.fillMaxSize()) {
        WorksGrid(
            works = works,
            lying = library.kind == "home_media",
            empty = stringResource(R.string.tv_empty_library),
            memory = memory,
            grid = grid,
            onOpen = { chrome.go(Place.Opened(it.id)) },
            firstCard = first,
            onReach = { card -> at = card.initial },
        ) {
            Column {
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
                SortAndFilters(
                    order = order,
                    descending = descending,
                    genre = genre,
                    decade = decade,
                    filters = filters,
                    memory = memory,
                    onPick = { picking = it },
                    onTurn = { descending = !descending },
                )
            }
        }
        letters?.let {
            Letters(
                it,
                current = at,
                onJump = ::jump,
                modifier = Modifier.align(Alignment.TopEnd).padding(top = 92.dp, end = 10.dp),
            )
        }
        when (picking) {
            Picking.Order -> ChoiceList(
                title = stringResource(R.string.tv_sort),
                choices = ORDERS.map { orderName(it) },
                chosen = ORDERS.indexOf(order),
                onChoose = { order = ORDERS[it] },
                onClose = ::closePicking,
                modifier = Modifier.align(Alignment.Center),
            )
            Picking.Genre -> filters?.let { known ->
                ChoiceList(
                    title = stringResource(R.string.tv_genre),
                    choices = listOf(stringResource(R.string.tv_filter_any)) +
                        known.genres.map { stringResource(R.string.tv_counted, it.name, it.works) },
                    chosen = known.genres.indexOfFirst { it.name == genre } + 1,
                    onChoose = { genre = if (it == 0) null else known.genres[it - 1].name },
                    onClose = ::closePicking,
                    modifier = Modifier.align(Alignment.Center),
                )
            }
            Picking.Decade -> filters?.let { known ->
                ChoiceList(
                    title = stringResource(R.string.tv_decade),
                    choices = listOf(stringResource(R.string.tv_filter_any)) +
                        known.decades.map { stringResource(R.string.tv_counted, decadeName(it.decade), it.works) },
                    chosen = known.decades.indexOfFirst { it.decade == decade } + 1,
                    onChoose = { decade = if (it == 0) null else known.decades[it - 1].decade },
                    onClose = ::closePicking,
                    modifier = Modifier.align(Alignment.Center),
                )
            }
            null -> Unit
        }
    }
    RestoreFocus(memory, first, ready = works.cards != null)
}

/** The sort and the filters, as the web's bar above a grid: each opens its
 *  list of choices, the arrow turns the order round. */
@Composable
private fun SortAndFilters(
    order: String,
    descending: Boolean,
    genre: String?,
    decade: Int?,
    filters: Filters?,
    memory: FocusMemory,
    onPick: (Picking) -> Unit,
    onTurn: () -> Unit,
) {
    val any = stringResource(R.string.tv_filter_any)
    Row(
        modifier = Modifier.padding(top = 14.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        OrderButtons(orderName(order), descending, memory, onPick = { onPick(Picking.Order) }, onTurn = onTurn)
        if (filters != null && filters.genres.isNotEmpty()) {
            PlainButton(
                stringResource(R.string.tv_genre_is, genre ?: any),
                onClick = { onPick(Picking.Genre) },
                modifier = Modifier.remembered(memory, "genre"),
            )
        }
        if (filters != null && filters.decades.isNotEmpty()) {
            PlainButton(
                stringResource(R.string.tv_decade_is, decade?.let { decadeName(it) } ?: any),
                onClick = { onPick(Picking.Decade) },
                modifier = Modifier.remembered(memory, "decade"),
            )
        }
    }
}

@Composable
private fun orderName(order: String): String = stringResource(
    when (order) {
        "added_at" -> R.string.tv_sort_added_at
        "release_year" -> R.string.tv_sort_release_year
        "community_rating" -> R.string.tv_sort_community_rating
        "runtime" -> R.string.tv_sort_runtime
        else -> R.string.tv_sort_title
    },
)

@Composable
private fun decadeName(decade: Int): String = stringResource(R.string.tv_decade_name, decade)

/**
 * The letters the library's titles start with, down the side of the grid as
 * the web's rail, the one the remote is under lit: choosing one brings its
 * first work to the remote.
 */
@Composable
private fun Letters(letters: List<String>, current: String?, onJump: (String) -> Unit, modifier: Modifier) {
    val said = stringResource(R.string.tv_letters)
    Column(modifier.semantics { contentDescription = said }, horizontalAlignment = Alignment.CenterHorizontally) {
        letters.forEach { letter ->
            Letter(letter, current = letter.equals(current, ignoreCase = true), onClick = { onJump(letter) })
        }
    }
}

@Composable
private fun Letter(letter: String, current: Boolean, onClick: () -> Unit) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(5.dp)
    Box(
        modifier = Modifier
            .size(26.dp, 16.dp)
            .clip(shape)
            .background(palette.accent.copy(alpha = lit))
            .clickable(interaction, indication = null, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            letter.uppercase(),
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
            color = when {
                lit > 0.5f -> palette.accentContrast
                current -> palette.accentBright
                else -> palette.textFaint
            },
        )
    }
}
