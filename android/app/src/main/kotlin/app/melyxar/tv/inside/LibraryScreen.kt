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
import app.melyxar.server.Library
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette

/** A library, its works in a grid as the web shows them. */
@Composable
fun LibraryScreen(library: Library, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val works = rememberWorks(Browse(library = library.id), comebacks)
    WorksGrid(
        works = works,
        lying = library.kind == "home_media",
        empty = stringResource(R.string.tv_empty_library),
        memory = memory,
        grid = rememberLazyGridState(),
        onOpen = { chrome.go(Place.Opened(it.id)) },
        firstCard = first,
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
        }
    }
    RestoreFocus(memory, first, ready = works.cards != null)
}
