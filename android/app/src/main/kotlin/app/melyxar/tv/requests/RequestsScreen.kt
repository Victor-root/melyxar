package app.melyxar.tv.requests

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Found
import app.melyxar.server.TitleRequest
import app.melyxar.server.searchTitles
import app.melyxar.server.titleSuggestions
import app.melyxar.theme.Palette
import app.melyxar.tv.inside.BarTitle
import app.melyxar.tv.inside.CardRow
import app.melyxar.tv.inside.FocusMemory
import app.melyxar.tv.inside.Listen
import app.melyxar.tv.inside.listensOnItsKey
import app.melyxar.tv.inside.rememberVoice
import app.melyxar.tv.inside.Loaded
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.ReadingColumn
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.Voice
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.instantOf
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.GlassBar
import app.melyxar.tv.look.GlassTab
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.TvField
import app.melyxar.tv.look.listLine
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce

/** How long typing rests before the provider is asked, as the web's page
 *  waits. */
private const val ASKED_AFTER_MS = 450L

private const val ASK = "ask"
private const val MINE = "mine"

/** A card of a row of suggestions: a title, or the way to many more. */
private sealed interface OnShelf {
    data class Title(val found: Found) : OnShelf

    data object More : OnShelf
}

/**
 * The requests, as the web's page of them: looking a title up to ask for
 * it, with suggestions while nothing is typed, and the requests this account
 * made, each with what became of it.
 */
@OptIn(FlowPreview::class)
@Composable
fun RequestsScreen() {
    val requests = LocalRequests.current
    var tab by rememberSaveable { mutableStateOf(ASK) }
    val words = rememberTextFieldState()
    val voice = rememberVoice { words.setTextAndPlaceCursorAtEnd(it) }
    var asked by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(words) {
        snapshotFlow { words.text.toString().trim() }
            .debounce(ASKED_AFTER_MS)
            .collect { asked = it }
    }
    val memory = rememberFocusMemory()
    val firstTab = remember { FocusRequester() }
    val list = rememberLazyListState()
    val language = requestLanguage()
    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 40.dp), modifier = Modifier.fillMaxSize().listensOnItsKey(voice)) {
            item("top") {
                Column(Modifier.head(head, "top")) {
                    TopBar { BarTitle(Icons.Request, stringResource(R.string.tv_requests)) }
                    Box(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 10.dp), contentAlignment = Alignment.Center) {
                        GlassBar(Modifier.focusRestorer(firstTab)) {
                            listOf(ASK to R.string.tv_requests_tab_ask, MINE to R.string.tv_requests_tab_mine).forEach { (name, said) ->
                                GlassTab(
                                    stringResource(said),
                                    on = name == tab,
                                    onClick = { tab = name },
                                    modifier = Modifier
                                        .remembered(memory, "tab/$name")
                                        .then(if (name == ASK) Modifier.focusRequester(firstTab) else Modifier),
                                )
                            }
                        }
                    }
                }
            }
            when (tab) {
                ASK -> asking(words, voice, asked, language, memory)
                else -> mine(requests.mine, memory)
            }
        }
    }
    RestoreFocus(memory, firstTab, ready = true)
}

/** The field a title is looked up in, and under it what it finds, or the
 *  suggestions while nothing is typed. */
private fun LazyListScope.asking(
    words: TextFieldState,
    voice: Voice?,
    asked: String,
    language: String,
    memory: FocusMemory,
) {
    item("field") {
        ReadingColumn {
            Row(
                modifier = Modifier.padding(top = 10.dp, bottom = 18.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                TvField(
                    state = words,
                    placeholder = stringResource(R.string.tv_requests_search_placeholder),
                    onDone = {},
                    modifier = Modifier.weight(1f).remembered(memory, "field"),
                )
                Listen(voice)
            }
        }
    }
    if (asked.isEmpty()) {
        item("suggestions") { Suggestions(language, memory) }
    } else {
        item("found:$asked") { Found(asked, language, memory) }
    }
}

@Composable
private fun Found(asked: String, language: String, memory: FocusMemory) {
    val palette = LocalPalette.current
    val loaded = rememberLoaded("requests:search:$language:$asked") { searchTitles(asked, language) }.value
    val found = loaded.ready
    when {
        found != null && found.isEmpty() || loaded is Loaded.Failed -> ReadingColumn {
            Text(stringResource(R.string.tv_requests_nothing_found), fontSize = 16.sp, color = palette.textSoft)
        }
        found != null -> CardRow(null, null, found, key = { it.catalogue + it.tmdbId }) { one ->
            TitleTile(one, Modifier.remembered(memory, "found/${one.catalogue}/${one.tmdbId}"))
        }
    }
}

/** Rows of titles for the genres this account watches most, or the most
 *  popular while it has not watched enough to tell, each ending on the way
 *  to many more. */
@Composable
private fun Suggestions(language: String, memory: FocusMemory) {
    val chrome = LocalChrome.current
    val shelves = rememberLoaded("requests:suggestions:$language") { titleSuggestions(language) }.value.ready.orEmpty()
    Column {
        shelves.forEach { shelf ->
            CardRow(
                shelfName(shelf.genre),
                if (shelf.catalogue == "series") Icons.Series else Icons.Film,
                shelf.items.map(OnShelf::Title) + OnShelf.More,
                key = { if (it is OnShelf.Title) it.found.tmdbId else "more" },
            ) { one ->
                val at = "${shelf.catalogue}/${shelf.genreId}"
                when (one) {
                    is OnShelf.Title -> TitleTile(one.found, Modifier.remembered(memory, "shelf/$at/${one.found.tmdbId}"))
                    OnShelf.More -> MoreTile(
                        onClick = { chrome.go(Place.MoreTitles(shelf.catalogue, shelf.genreId, shelf.genre)) },
                        modifier = Modifier.remembered(memory, "shelf/$at/more"),
                    )
                }
            }
        }
    }
}

/** What a row of suggestions is called: its genre, or what is popular. */
@Composable
fun shelfName(genre: String?): String =
    genre?.let { stringResource(R.string.tv_requests_suggested_genre, it) } ?: stringResource(R.string.tv_requests_suggested_popular)

/** The requests this account made, newest first. */
private fun LazyListScope.mine(mine: List<TitleRequest>?, memory: FocusMemory) {
    if (mine == null) {
        return
    }
    if (mine.isEmpty()) {
        item("none") {
            ReadingColumn {
                Text(
                    stringResource(R.string.tv_requests_none_yet),
                    fontSize = 16.sp,
                    color = LocalPalette.current.textSoft,
                    modifier = Modifier.padding(top = 10.dp),
                )
            }
        }
        return
    }
    items(mine, key = { it.id }) { request ->
        ReadingColumn { MyRequest(request, memory) }
    }
}

/** The colour a request's state wears. */
fun Palette.ofState(state: String): Color = when (state) {
    "pending" -> news
    "refused" -> trouble
    else -> ok
}

@Composable
private fun stateName(state: String): String = stringResource(
    when (state) {
        "pending" -> R.string.tv_requests_state_pending
        "accepted" -> R.string.tv_requests_state_accepted
        "refused" -> R.string.tv_requests_state_refused
        else -> R.string.tv_requests_state_added
    },
)

/** The names of [seasons], one after the other. */
@Composable
fun seasonsNamed(seasons: List<Int>): String = seasons.map { stringResource(R.string.tv_season, it) }.joinToString(", ")

/** One request, with what became of it: one that waits can be cancelled,
 *  one refused or added cleared from the list, one added opened. */
@Composable
private fun MyRequest(request: TitleRequest, memory: FocusMemory) {
    val chrome = LocalChrome.current
    val requests = LocalRequests.current
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(8.dp)
    Row(
        modifier = Modifier
            .padding(top = 10.dp)
            .fillMaxWidth()
            .listLine { 0f }
            .padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Box(Modifier.size(72.dp, 108.dp).clip(shape)) { TitlePoster(request.poster, 72.dp) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                listOfNotNull(request.title, request.year?.toString()).joinToString(" · "),
                fontSize = 17.sp,
                fontWeight = FontWeight.SemiBold,
                color = palette.text,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                StatePill(stateName(request.state), palette.ofState(request.state))
                Text(catalogueName(request.catalogue), fontSize = 13.sp, color = palette.textFaint)
                instantOf(request.createdAt)?.let { at ->
                    Text(DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(at)), fontSize = 13.sp, color = palette.textFaint)
                }
            }
            if (request.seasons.isNotEmpty()) {
                Text(seasonsNamed(request.seasons), fontSize = 14.sp, color = palette.textSoft)
            }
            if (request.note.isNotEmpty()) {
                Text(request.note, fontSize = 14.sp, fontStyle = FontStyle.Italic, color = palette.textSoft)
            }
            if (request.state == "refused" && request.answer.isNotEmpty()) {
                Text(stringResource(R.string.tv_requests_answer, request.answer), fontSize = 14.sp, color = palette.textSoft)
            }
            Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                val workId = request.workId
                if (request.state == "added" && workId != null) {
                    AccentButton(
                        stringResource(R.string.tv_requests_open),
                        onClick = { chrome.go(Place.Opened(workId)) },
                        modifier = Modifier.remembered(memory, "open/${request.id}"),
                        icon = null,
                    )
                }
                when (request.state) {
                    "pending" -> PlainButton(
                        stringResource(R.string.tv_requests_cancel),
                        onClick = { requests.withdraw(request.id) },
                        modifier = Modifier.remembered(memory, "withdraw/${request.id}"),
                    )
                    "refused", "added" -> PlainButton(
                        stringResource(R.string.tv_requests_clear),
                        onClick = { requests.withdraw(request.id) },
                        modifier = Modifier.remembered(memory, "withdraw/${request.id}"),
                    )
                }
            }
        }
    }
}
