package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.melyxar.R
import app.melyxar.server.Card
import app.melyxar.server.Home
import app.melyxar.tv.howFarIn
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.Icons

/**
 * The home screen, as the web's: the banner, the band of libraries, and the
 * rows in the order the account chose, those it hid left out.
 *
 * The banner and the band under it are shown whole from the top of the page
 * while the remote is in them; the page scrolls once it goes down into the
 * rows below.
 */
@Composable
fun HomeScreen(comebacks: Int) {
    val chrome = LocalChrome.current
    var tries by remember { mutableIntStateOf(0) }
    val loaded = rememberLoaded("home", comebacks, tries) { home() }.value
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val home = loaded.ready
    val open = { card: Card -> chrome.go(Place.Opened(card.id)) }
    val list = rememberLazyListState()
    Box(Modifier.fillMaxSize()) {
        when {
            home != null -> ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
                LazyColumn(state = list, modifier = Modifier.fillMaxSize()) {
                    item("top") {
                        Box(Modifier.head(head, "top")) {
                            if (home.hero.isNotEmpty()) {
                                Hero(
                                    items = home.hero,
                                    onPlay = { chrome.play(Play(it.card.id, it.card.source, it.card.resumeFromSeconds?.toDouble())) },
                                    onOpen = { chrome.go(Place.Opened(it)) },
                                    first = first,
                                )
                            } else {
                                Box(Modifier.height(100.dp))
                            }
                            TopBar(Modifier.align(Alignment.TopCenter), first = if (home.hero.isEmpty()) first else null)
                        }
                    }
                    // The band is part of the head only where it stands right
                    // under the banner.
                    val band = if (home.sections.firstOrNull() == "band") Modifier.head(head, "band") else Modifier
                    sections(home, open, memory, band)
                }
            }
            loaded is Loaded.Failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    RestoreFocus(memory, first, ready = home != null)
}

private fun LazyListScope.sections(home: Home, open: (Card) -> Unit, memory: FocusMemory, band: Modifier) {
    home.sections.forEach { section ->
        when {
            section == "band" -> item("band") {
                val chrome = LocalChrome.current
                Box(band) {
                    Band(home.shelves, chrome.libraries, home.kindOrder, onOpen = { chrome.go(Place.Shelf(it)) }, memory)
                }
            }
            section == "carry_on" -> section(section, home.carryOn) {
                val chrome = LocalChrome.current
                CardRow(stringResource(R.string.tv_carry_on), Icons.Eye, home.carryOn, key = { it.card.id }) { row ->
                    val card = row.card
                    val length = lengthOfAPlay(card)
                    val at = card.resumeFromSeconds?.toDouble() ?: 0.0
                    WorkCard(
                        card = card,
                        lying = true,
                        title = row.more.seriesTitle ?: card.title,
                        note = whichEpisode(row.more.seasonNumber, row.more.episodeNumber, card.title),
                        trailing = whatIsLeft(at, length),
                        progress = howFarIn(at, length),
                        // What was left halfway goes on at once.
                        onClick = { chrome.play(Play(card.id, card.source, at)) },
                        modifier = Modifier.remembered(memory, "carry/${card.id}"),
                    )
                }
            }
            section == "up_next" -> section(section, home.upNext) {
                CardRow(stringResource(R.string.tv_up_next), Icons.Binoculars, home.upNext, key = { it.card.id }) { row ->
                    WorkCard(
                        card = row.card,
                        lying = true,
                        title = row.more.seriesTitle ?: row.card.title,
                        note = whichEpisode(row.more.seasonNumber, row.more.episodeNumber, row.card.title),
                        onClick = { open(row.card) },
                        modifier = Modifier.remembered(memory, "next/${row.card.id}"),
                    )
                }
            }
            section == "recently_added" -> section(section, home.recentlyAdded) {
                CardRow(stringResource(R.string.tv_recently_added), Icons.Camera, home.recentlyAdded, key = { it.id }) { card ->
                    WorkCard(card, onClick = { open(card) }, modifier = Modifier.remembered(memory, "new/${card.id}"))
                }
            }
            section.startsWith("newest:") -> {
                val kind = section.removePrefix("newest:")
                val cards = home.shelves.firstOrNull { it.kind == kind }?.cards.orEmpty()
                section(section, cards) {
                    CardRow(newestOfKind(kind), Icons.ofKind(kind), cards, key = { it.id }) { card ->
                        WorkCard(
                            card,
                            lying = kind == "home_media",
                            onClick = { open(card) },
                            modifier = Modifier.remembered(memory, "$section/${card.id}"),
                        )
                    }
                }
            }
        }
    }
}
