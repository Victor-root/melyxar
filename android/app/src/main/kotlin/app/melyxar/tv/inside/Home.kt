package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.melyxar.R
import app.melyxar.server.Album
import app.melyxar.server.Card
import app.melyxar.server.Home
import app.melyxar.server.Slice
import app.melyxar.server.albums
import app.melyxar.tv.WatchNext
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Drawn
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.WideWidth
import app.melyxar.tv.look.rememberFetchAhead
import kotlinx.coroutines.withTimeoutOrNull

/** How many of the newest albums are asked for, for the tile of music to
 *  find covers among. */
private const val COVERS_ASKED = 10

/** How many cards of each row are fetched before the home screen first
 *  shows: more than a row shows at once. */
private const val CARDS_AHEAD = 10

/** The longest the home screen waits on its pictures the first time. */
private const val AHEAD_AT_MOST_MS = 4_000L

/** How many frames the page is drawn under the ring before it shows: its
 *  first, and the lighting of what the remote lands on. */
private const val FRAMES_UNDER_THE_RING = 24

/** Whether the home screen has been shown since the app started: only the
 *  first time does it wait for its pictures. */
private var shownOnce = false

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
    val marks = LocalMarks.current
    var tries by remember { mutableIntStateOf(0) }
    val loaded = rememberLoaded("home", comebacks, tries) { home() }.value
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    // Without the banner, the remote starts on the first library.
    val firstLibrary = remember { FocusRequester() }
    val home = loaded.ready
    // The covers the tile of music fans out, which the server's shelves do
    // not hold: the newest albums of every library of music, as on the web.
    val musicLibraries = chrome.libraries.filter { it.kind == "music" }.map { it.id }
    val newestAlbums = rememberLoaded("music:band:$musicLibraries", comebacks) {
        inTurn(musicLibraries.map { albums(it, "added", false, Slice(0, COVERS_ASKED)).items }, COVERS_ASKED)
    }.value.ready.orEmpty()
    val context = LocalContext.current
    val api = LocalServer.current
    // The system's own row follows Melyxar's, whenever home is read.
    LaunchedEffect(home) {
        home?.let { WatchNext(context).show(api, it.carryOn) }
    }
    // The first time, the ring turns on while the pictures of the banner and
    // the rows come into memory, then while the page is drawn under it, the
    // remote already on it: the graphics card does its first drawing of each
    // piece, and of a piece lit, out of sight, and the page comes smooth.
    val fetchAhead = rememberFetchAhead()
    var fetched by remember { mutableStateOf(shownOnce) }
    var shown by remember { mutableStateOf(shownOnce) }
    LaunchedEffect(home) {
        home ?: return@LaunchedEffect
        withTimeoutOrNull(AHEAD_AT_MOST_MS) { fetchAhead(drawnOn(home)) }
        fetched = true
    }
    LaunchedEffect(fetched) {
        if (fetched && !shown) {
            repeat(FRAMES_UNDER_THE_RING) { withFrameNanos {} }
            shownOnce = true
            shown = true
        }
    }
    val open = { card: Card -> chrome.go(Place.Opened(card.id)) }
    val list = rememberLazyListState()
    Box(Modifier.fillMaxSize()) {
        if (home != null && fetched) {
            ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
                LazyColumn(state = list, modifier = Modifier.fillMaxSize()) {
                    item("top") {
                        Box(Modifier.head(head, "top")) {
                            if (home.hero.isNotEmpty()) {
                                Hero(
                                    items = home.hero,
                                    onPlay = { chrome.play(Play(it.card.id, it.card.source, marks.whereabouts(it.card).resume?.toDouble())) },
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
                    sections(home, newestAlbums, open, memory, band, firstLibrary)
                }
            }
        }
        if (loaded is Loaded.Failed) {
            Trouble(onRetry = { tries += 1 })
        } else if (home == null || !shown) {
            Box(Modifier.fillMaxSize().background(LocalPalette.current.ground)) {
                TopBar()
                ArrivalLoader(Modifier.align(Alignment.Center))
            }
        }
    }
    RestoreFocus(memory, if (home?.hero.isNullOrEmpty()) firstLibrary else first, ready = home != null && fetched, orElse = first)
}

/** What [home] draws first: its banner's first two works, and the first
 *  cards of each of its rows. */
private fun drawnOn(home: Home): List<Drawn> =
    home.hero.take(2).flatMap(::bannerDrawnOf) +
        home.sections.flatMap { section -> cardsIn(section, home).take(CARDS_AHEAD).map { it.drawn(liesIn(section)) } }

private fun LazyListScope.sections(
    home: Home,
    newestAlbums: List<Album>,
    open: (Card) -> Unit,
    memory: FocusMemory,
    band: Modifier,
    firstLibrary: FocusRequester,
) {
    laidOut(home.sections).forEach { laid ->
        when (laid) {
            is Laid.One -> section(laid.section, home, newestAlbums, open, memory, band, firstLibrary)
            is Laid.Two -> {
                val first = cardsIn(laid.first, home).size
                val second = cardsIn(laid.second, home).size
                if (first == 0 || second == 0) {
                    section(laid.first, home, newestAlbums, open, memory, band, firstLibrary)
                    section(laid.second, home, newestAlbums, open, memory, band, firstLibrary)
                } else {
                    item("${laid.first}+${laid.second}") {
                        Together(laid.first, laid.second, first, second, home, open, memory)
                    }
                }
            }
        }
    }
}

/** The cards of the row [section], none for the band. */
private fun cardsIn(section: String, home: Home): List<Card> = when {
    section == "carry_on" -> home.carryOn.map { it.card }
    section == "up_next" -> home.upNext.map { it.card }
    section == "recently_added" -> home.recentlyAdded
    section.startsWith("newest:") -> home.shelves.firstOrNull { it.kind == section.removePrefix("newest:") }?.cards.orEmpty()
    else -> emptyList()
}

/** Whether the cards of the row [section] lie down, their stills shown. */
private fun liesIn(section: String) = section == "carry_on" || section == "up_next" || section == "newest:home_media"

/**
 * The two rows of what is under way on one line while both hold on it, as
 * the web lays them, a hairline of the accent down the air between them;
 * one under the other otherwise.
 */
@Composable
private fun Together(firstSection: String, secondSection: String, first: Int, second: Int, home: Home, open: (Card) -> Unit, memory: FocusMemory) {
    val palette = LocalPalette.current
    BoxWithConstraints {
        if (shareALine(first, second, WideWidth, CardGap, Gutter, maxWidth)) {
            Row {
                UnderWay(firstSection, home, open, memory, Modifier.width(rowWidth(first, WideWidth, CardGap, Gutter)))
                UnderWay(
                    secondSection,
                    home,
                    open,
                    memory,
                    Modifier
                        .width(rowWidth(second, WideWidth, CardGap, Gutter))
                        .drawWithCache {
                            val line = Brush.verticalGradient(
                                0f to Color.Transparent,
                                0.18f to palette.accent.copy(alpha = 0.45f),
                                0.82f to palette.accent.copy(alpha = 0.45f),
                                1f to Color.Transparent,
                            )
                            onDrawBehind { drawRect(line, size = Size(1.dp.toPx(), size.height)) }
                        },
                )
            }
        } else {
            Column {
                UnderWay(firstSection, home, open, memory, Modifier)
                UnderWay(secondSection, home, open, memory, Modifier)
            }
        }
    }
}

private fun LazyListScope.section(
    section: String,
    home: Home,
    newestAlbums: List<Album>,
    open: (Card) -> Unit,
    memory: FocusMemory,
    band: Modifier,
    firstLibrary: FocusRequester,
) {
    when {
        section == "band" -> item("band") {
            val chrome = LocalChrome.current
            Box(band) {
                Band(home.shelves, newestAlbums, chrome.libraries, home.kindOrder, onOpen = { chrome.go(Place.Shelf(it)) }, memory, firstLibrary)
            }
        }
        section == "carry_on" || section == "up_next" -> if (cardsIn(section, home).isNotEmpty()) {
            item(section) { UnderWay(section, home, open, memory, Modifier) }
        }
        section == "recently_added" -> section(section, home.recentlyAdded) {
            CardRow(stringResource(R.string.tv_recently_added), Icons.Camera, home.recentlyAdded, key = { it.id }) { card ->
                WorkCard(card, onClick = { open(card) }, modifier = Modifier.remembered(memory, "new/${card.id}"))
            }
        }
        section.startsWith("newest:") -> {
            val kind = section.removePrefix("newest:")
            val cards = cardsIn(section, home)
            section(section, cards) {
                CardRow(newestOfKind(kind), Icons.ofKind(kind), cards, key = { it.id }) { card ->
                    WorkCard(
                        card,
                        lying = liesIn(section),
                        onClick = { open(card) },
                        modifier = Modifier.remembered(memory, "$section/${card.id}"),
                    )
                }
            }
        }
    }
}

/** A row of what is under way: what was started, or what comes next. */
@Composable
private fun UnderWay(section: String, home: Home, open: (Card) -> Unit, memory: FocusMemory, modifier: Modifier) {
    if (section == "carry_on") {
        CardRow(stringResource(R.string.tv_carry_on), Icons.Eye, home.carryOn, key = { it.card.id }, modifier = modifier) { row ->
            val card = row.card
            val at = LocalMarks.current.whereabouts(card).resume?.toDouble() ?: 0.0
            WorkCard(
                card = card,
                lying = liesIn(section),
                title = row.more.seriesTitle ?: card.title,
                note = whichEpisode(row.more.seasonNumber, row.more.episodeNumber, card.title),
                trailing = whatIsLeft(at, lengthOfAPlay(card)),
                onClick = { open(card) },
                modifier = Modifier.remembered(memory, "carry/${card.id}"),
            )
        }
    } else {
        CardRow(stringResource(R.string.tv_up_next), Icons.Binoculars, home.upNext, key = { it.card.id }, modifier = modifier) { row ->
            WorkCard(
                card = row.card,
                lying = liesIn(section),
                title = row.more.seriesTitle ?: row.card.title,
                note = whichEpisode(row.more.seasonNumber, row.more.episodeNumber, row.card.title),
                onClick = { open(row.card) },
                modifier = Modifier.remembered(memory, "next/${row.card.id}"),
            )
        }
    }
}
