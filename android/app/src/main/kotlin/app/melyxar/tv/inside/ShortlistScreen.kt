package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Browse
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette

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
    val palette = LocalPalette.current
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
        Column {
            // The grid keeps the page's margin already.
            TopBar(edges = 0.dp)
            Row(
                modifier = Modifier.padding(top = 26.dp, bottom = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Glyph(if (list == Shortlist.Favourites) Icons.Heart else Icons.Clock, palette.accentBright, 26.dp)
                Text(
                    stringResource(if (list == Shortlist.Favourites) R.string.tv_favourites else R.string.tv_watch_later),
                    fontSize = 28.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = (-0.02).em,
                    color = palette.text,
                )
            }
        }
    }
    RestoreFocus(memory, first, ready = works.cards != null)
}
