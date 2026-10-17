package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Browse
import app.melyxar.server.Library
import app.melyxar.server.searchMusic
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.TvField
import app.melyxar.tv.music.GridSpot
import app.melyxar.tv.music.LocalListening
import app.melyxar.tv.music.foundMusic
import app.melyxar.tv.music.foundNames
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce

/** How long the words have to stay as they are before they are asked
 *  about, as the web's header waits. */
private const val SEARCH_WAIT_MS = 200L

/**
 * The search, as the web's: the title of a film, a series or a video, typed
 * with the television's keyboard or said into the remote, and the works it
 * names in a grid under it, as soon as the words stop changing. Asked from
 * the pages of a [library], it looks only there, its music or its works.
 */
@OptIn(FlowPreview::class)
@Composable
fun SearchScreen(library: Library?, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val words = rememberTextFieldState()
    val voice = rememberVoice { words.setTextAndPlaceCursorAtEnd(it) }
    var asked by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(words) {
        snapshotFlow { words.text.toString().trim() }
            .debounce(SEARCH_WAIT_MS)
            .collect { asked = it }
    }
    val memory = rememberFocusMemory()
    val field = remember { FocusRequester() }
    val inMusic = library?.kind == "music"
    val works = rememberWorks(Browse(library = library?.id, search = asked), comebacks, asking = asked.isNotEmpty() && !inMusic)
    // What the words find in the music, above the works, as on the web.
    val music = asked.takeIf { it.isNotEmpty() && (library == null || inMusic) }?.let { words ->
        rememberLoaded("music:search:${library?.id}:$words", comebacks) { searchMusic(words, library?.id) }.value.ready
    }
    val names = foundNames()
    val listening = LocalListening.current
    val grid = rememberLazyGridState()
    WorksGrid(
        works = works,
        lying = false,
        empty = stringResource(R.string.tv_search_none),
        memory = memory,
        grid = grid,
        onOpen = { chrome.go(Place.Opened(it.id)) },
        modifier = Modifier.listensOnItsKey(voice),
        besides = { head ->
            music?.let { foundMusic(it, names, empty = null, GridSpot(grid, head, memory), chrome.go, listening) }
        },
    ) {
        Column {
            // The grid keeps the page's margin already.
            TopBar(inset = Gutter) {
                BarTitle(Icons.Search, library?.let { stringResource(R.string.tv_search_in, it.name) } ?: stringResource(R.string.tv_search))
            }
            Row(
                modifier = Modifier.padding(top = 26.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                TvField(
                    state = words,
                    placeholder = stringResource(R.string.tv_search_placeholder),
                    onDone = {},
                    modifier = Modifier.weight(1f).remembered(memory, "field").focusRequester(field),
                )
                Listen(voice)
            }
            if (asked.isEmpty()) {
                Text(stringResource(R.string.tv_search_lead), fontSize = 15.sp, color = palette.textSoft)
            }
        }
    }
    RestoreFocus(memory, field, ready = true)
}
