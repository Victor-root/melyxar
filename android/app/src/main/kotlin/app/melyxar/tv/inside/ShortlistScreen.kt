package app.melyxar.tv.inside

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.melyxar.R
import app.melyxar.server.Browse
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons

/** The two lists an account keeps of works, each a page of its own as on
 *  the web. */
enum class Shortlist { Favourites, WatchLater }

/**
 * The account's favourites, or what it set aside to watch later, in a grid
 * as the web's pages show them: a work taken off the list has left it at
 * once, and the list is read again on coming back.
 */
@Composable
fun ShortlistScreen(list: Shortlist, comebacks: Int) {
    val chrome = LocalChrome.current
    val marks = LocalMarks.current
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val works = rememberWorks(
        when (list) {
            Shortlist.Favourites -> Browse(favourites = true)
            Shortlist.WatchLater -> Browse(watchLater = true)
        },
        comebacks,
    )
    WorksGrid(
        works = works,
        lying = false,
        empty = stringResource(if (list == Shortlist.Favourites) R.string.tv_favourites_empty else R.string.tv_watch_later_empty),
        memory = memory,
        grid = rememberLazyGridState(),
        onOpen = { chrome.go(Place.Opened(it.id)) },
        firstCard = first,
        belongs = {
            when (list) {
                Shortlist.Favourites -> marks.favourite(it)
                Shortlist.WatchLater -> marks.watchLater(it)
            }
        },
    ) {
        // The grid keeps the page's margin already.
        TopBar(Modifier.padding(bottom = 22.dp), inset = Gutter) {
            BarTitle(
                if (list == Shortlist.Favourites) Icons.Heart else Icons.Clock,
                stringResource(if (list == Shortlist.Favourites) R.string.tv_favourites else R.string.tv_watch_later),
            )
        }
    }
    RestoreFocus(memory, first, ready = works.cards != null)
}
