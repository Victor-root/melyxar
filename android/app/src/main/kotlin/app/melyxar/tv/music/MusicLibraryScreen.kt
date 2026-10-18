package app.melyxar.tv.music

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.itemsIndexed
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.layout
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.FoundMusic
import app.melyxar.server.Library
import app.melyxar.server.Refusal
import app.melyxar.server.Slice
import app.melyxar.server.Song
import app.melyxar.server.albums
import app.melyxar.server.artists
import app.melyxar.server.genres
import app.melyxar.server.listened
import app.melyxar.server.musicFavourites
import app.melyxar.server.playlists
import app.melyxar.server.songs
import app.melyxar.tv.inside.BarTitle
import app.melyxar.tv.inside.CardRow
import app.melyxar.tv.inside.FocusMemory
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.OrderButtons
import app.melyxar.tv.inside.PageHead
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.RowHead
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.Trouble
import app.melyxar.tv.inside.firstLineHoldsTheHead
import app.melyxar.tv.inside.focusTraced
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberReaching
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.GlassBar
import app.melyxar.tv.look.GlassIcon
import app.melyxar.tv.look.GlassRule
import app.melyxar.tv.look.GlassTab
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import java.io.IOException
import kotlinx.coroutines.launch

/** The tabs of a library of music, in the web's order. */
private val TABS = listOf("for_you", "albums", "album_artists", "artists", "songs", "playlists", "favourites", "genres")

/** The orders albums and songs can be read in, as the web offers them. */
private val ALBUM_ORDERS = listOf("title", "artist", "year", "added")
private val SONG_ORDERS = listOf("title", "album", "added")

/** How many albums the row of the newest shows. */
private const val NEWEST_SHOWN = 24

/**
 * A library of music, as the web's: its tabs, those the account hides left
 * out, and under them what each holds, in one page that scrolls under the
 * remote, the tabs staying where they are as one is chosen.
 */
@Composable
fun MusicLibraryScreen(library: Library, comebacks: Int) {
    val listening = LocalListening.current
    val scope = rememberCoroutineScope()
    val memory = rememberFocusMemory()
    val firstTab = remember { FocusRequester() }
    val firstTile = remember { FocusRequester() }
    val tabs = TABS.filter { it !in listening.preferences.hiddenTabs }.ifEmpty { listOf("albums") }
    var chosen by rememberSaveable(library.id) { mutableStateOf(tabs.first()) }
    val tab = chosen.takeIf { it in tabs } ?: tabs.first()
    var albumOrder by rememberSaveable(library.id) { mutableStateOf("title") }
    var albumsDescending by rememberSaveable(library.id) { mutableStateOf(false) }
    var songOrder by rememberSaveable(library.id) { mutableStateOf("title") }
    var songsDescending by rememberSaveable(library.id) { mutableStateOf(false) }
    var picking by remember { mutableStateOf(false) }
    val grid = rememberSaveable(library.id, tab, saver = LazyGridState.Saver) { LazyGridState() }

    /** Plays the whole library, or shuffles it. */
    fun playAll(shuffle: Boolean) {
        scope.launch {
            try {
                listening.playLibrary(library.id, shuffle)
            } catch (_: Refusal) {
                // Nothing started: the buttons are there to be pressed again.
            } catch (_: IOException) {
                // Nor with the network gone.
            }
        }
    }

    /** Closes the list of orders, the remote back on what opened it. */
    fun closePicking() {
        picking = false
        scope.launch {
            withFrameNanos {}
            memory.restore()
        }
    }

    val ordering = when (tab) {
        "albums" -> Ordering(ALBUM_ORDERS, albumOrder, albumsDescending, { albumOrder = it }, { albumsDescending = !albumsDescending })
        "songs" -> Ordering(SONG_ORDERS, songOrder, songsDescending, { songOrder = it }, { songsDescending = !songsDescending })
        else -> null
    }?.let { it.copy(name = sortName(it.order)) }
    val content = tabContent(library, tab, comebacks, albumOrder, albumsDescending, songOrder, songsDescending)
    val lined = tab == "songs"
    Box(Modifier.fillMaxSize()) {
        ScrollingPage(toTop = { grid.animateScrollToItem(0) }) { head ->
            LazyVerticalGrid(
                columns = GridCells.Adaptive(TileWidth),
                state = grid,
                contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = 40.dp),
                horizontalArrangement = Arrangement.spacedBy(CardGap),
                // The songs a line each against the next, as the web's list.
                verticalArrangement = Arrangement.spacedBy(if (lined) 0.dp else 26.dp),
                modifier = Modifier.fillMaxSize().focusTraced("music library"),
            ) {
                wide("top") {
                    Box(Modifier.head(head, "top").padding(bottom = if (lined) 26.dp else 0.dp)) {
                        Head(
                            library = library,
                            tabs = tabs,
                            tab = tab,
                            ordering = ordering,
                            memory = memory,
                            firstTab = firstTab,
                            onTab = { chosen = it },
                            onPlay = ::playAll,
                            onPick = { picking = true },
                        )
                    }
                }
                content.lay(this, GridSpot(grid, head, memory, firstTile))
            }
        }
        if (picking && ordering != null) {
            ChoiceList(
                title = stringResource(R.string.tv_sort),
                choices = ordering.orders.map { sortName(it) },
                chosen = ordering.orders.indexOf(ordering.order),
                onChoose = { ordering.onOrder(ordering.orders[it]) },
                onClose = ::closePicking,
                modifier = Modifier.align(Alignment.Center),
            )
        }
    }
    // On the first of what the tab holds when it says where that is, as the
    // newest albums of "For you" do; on the tabs otherwise.
    RestoreFocus(memory, firstTile, ready = content.ready, orElse = firstTab)
}

/** What a tab holds: whether it is read enough to start the remote on it,
 *  and how it is laid in the grid. */
private class TabContent(val ready: Boolean, val lay: LazyGridScope.(GridSpot) -> Unit)

/**
 * The albums of one genre of a library, in a grid as the web shows them.
 */
@Composable
fun GenreScreen(library: Library, genre: String, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val grid = rememberSaveable(library.id, genre, saver = LazyGridState.Saver) { LazyGridState() }
    val list = rememberMusicList("music:genre:${library.id}:$genre", comebacks) { albums(library.id, "title", false, it, genre = genre) }
    val content = paged(list, stringResource(R.string.tv_music_no_album), key = { "album/${it.id}" }) { album, modifier ->
        AlbumTile(album, onClick = { chrome.go(Place.Album(album.id)) }, modifier = modifier)
    }
    ScrollingPage(toTop = { grid.animateScrollToItem(0) }) { head ->
        LazyVerticalGrid(
            columns = GridCells.Adaptive(TileWidth),
            state = grid,
            contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = 40.dp),
            horizontalArrangement = Arrangement.spacedBy(CardGap),
            verticalArrangement = Arrangement.spacedBy(26.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            wide("top") {
                Column(Modifier.head(head, "top")) {
                    // The grid keeps the page's margin already.
                    TopBar(inset = Gutter)
                    Text(
                        genre,
                        fontSize = 28.sp,
                        fontWeight = FontWeight.Bold,
                        letterSpacing = (-0.02).em,
                        color = palette.text,
                        modifier = Modifier.padding(top = 26.dp, bottom = 12.dp),
                    )
                }
            }
            content(this, GridSpot(grid, head, memory, first))
        }
    }
    RestoreFocus(memory, first, ready = list.items != null)
}

/** How a tab's list is read: the orders it offers, the one in force, which
 *  way round, and how to change them. */
private data class Ordering(
    val orders: List<String>,
    val order: String,
    val descending: Boolean,
    val onOrder: (String) -> Unit,
    val onTurn: () -> Unit,
    val name: String = "",
)

/** Where the pieces of a grid of music stand: the grid, its head, the
 *  remote's memory, and what the first tile is given, for a screen that
 *  starts the remote there. */
class GridSpot(val grid: LazyGridState, val head: PageHead, val memory: FocusMemory, val firstTile: FocusRequester? = null)

/** The library's name, its tabs, and the buttons over what the tab holds. */
@Composable
private fun Head(
    library: Library,
    tabs: List<String>,
    tab: String,
    ordering: Ordering?,
    memory: FocusMemory,
    firstTab: FocusRequester,
    onTab: (String) -> Unit,
    onPlay: (shuffle: Boolean) -> Unit,
    onPick: () -> Unit,
) {
    val palette = LocalPalette.current
    Column {
        // The grid keeps the page's margin already.
        TopBar(inset = Gutter) { BarTitle(Icons.Music, library.name) }
        // The tabs on one piece of glass, and what the tab is played and
        // read by on another, as the web's bar above a library of music.
        FlowRow(
            modifier = Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            GlassBar(Modifier.focusRestorer(firstTab)) {
                tabs.forEachIndexed { at, name ->
                    GlassTab(
                        tabName(name),
                        on = name == tab,
                        onClick = { onTab(name) },
                        modifier = Modifier
                            .remembered(memory, "tab/$name")
                            .then(if (at == 0) Modifier.focusRequester(firstTab) else Modifier),
                    )
                }
            }
            GlassBar {
                ordering?.let {
                    OrderButtons(it.name, it.descending, memory, onPick = onPick, onTurn = it.onTurn)
                    GlassRule()
                }
                GlassIcon(Icons.Play, stringResource(R.string.tv_music_play_all), onClick = { onPlay(false) }, modifier = Modifier.remembered(memory, "play"))
                GlassIcon(Icons.Shuffle, stringResource(R.string.tv_music_shuffle), onClick = { onPlay(true) }, modifier = Modifier.remembered(memory, "shuffle"))
            }
        }
    }
}

/**
 * What a tab holds, read from the server here, where it may be asked for,
 * and laid in the grid by what this answers. Only the tab shown is read.
 */
@Composable
private fun tabContent(
    library: Library,
    tab: String,
    comebacks: Int,
    albumOrder: String,
    albumsDescending: Boolean,
    songOrder: String,
    songsDescending: Boolean,
): TabContent {
    val chrome = LocalChrome.current
    val listening = LocalListening.current
    val id = library.id
    val openAlbum = { album: String -> chrome.go(Place.Album(album)) }
    val openArtist = { artist: String -> chrome.go(Place.Artist(artist)) }
    val lay: LazyGridScope.(GridSpot) -> Unit = when (tab) {
        "albums" -> {
            val list = rememberMusicList("music:albums:$id:$albumOrder:$albumsDescending", comebacks) {
                albums(id, albumOrder, albumsDescending, it)
            }
            val empty = stringResource(R.string.tv_music_no_album)
            paged(list, empty, key = { "album/${it.id}" }) { album, modifier -> AlbumTile(album, onClick = { openAlbum(album.id) }, modifier = modifier) }
        }
        "album_artists", "artists" -> {
            val albumArtists = tab == "album_artists"
            val list = rememberMusicList("music:artists:$id:$albumArtists", comebacks) { artists(id, albumArtists, it) }
            val empty = stringResource(R.string.tv_music_no_artist)
            paged(list, empty, key = { "artist/${it.id}" }) { artist, modifier -> ArtistTile(artist, onClick = { openArtist(artist.id) }, modifier = modifier) }
        }
        "songs" -> {
            val list = rememberMusicList("music:songs:$id:$songOrder:$songsDescending", comebacks) {
                songs(id, songOrder, songsDescending, it)
            }
            val empty = stringResource(R.string.tv_music_no_song)
            pagedSongs(list, empty) { songs, at -> listening.play(songs, at) }
        }
        "playlists" -> {
            val loaded = rememberLoaded("music:playlists", comebacks) { playlists() }.value
            val empty = stringResource(R.string.tv_music_no_playlist)
            val playlists = loaded.ready
            ({ spot ->
                tiles(playlists, empty, spot, key = { "playlist/${it.id}" }) { playlist, modifier ->
                    PlaylistTile(playlist, onClick = { chrome.go(Place.Playlist(playlist.id)) }, modifier = modifier)
                }
            })
        }
        "genres" -> {
            val genres = rememberLoaded("music:genres:$id", comebacks) { genres(id) }.value.ready
            val empty = stringResource(R.string.tv_music_no_album)
            ({ spot ->
                tiles(genres, empty, spot, key = { "genre/${it.name}" }) { genre, modifier ->
                    GenreTile(genre, onClick = { chrome.go(Place.Genre(library, genre.name)) }, modifier = modifier)
                }
            })
        }
        "favourites" -> {
            val found = rememberLoaded("music:favourites:$id", comebacks) { musicFavourites(id) }.value.ready
            val empty = stringResource(R.string.tv_music_no_favourite)
            val names = foundNames()
            ({ spot ->
                found?.let { foundMusic(it, names, empty, spot, chrome.go, listening) }
            })
        }
        else -> {
            val newest = rememberLoaded("music:newest:$id", comebacks) { albums(id, "added", false, Slice(0, NEWEST_SHOWN)).items }.value.ready
            val lately = rememberLoaded("music:lately:$id", comebacks) { listened(id, "lately") }.value.ready
            val most = rememberLoaded("music:most:$id", comebacks) { listened(id, "most") }.value.ready
            val heard = listOf(
                Heard(stringResource(R.string.tv_music_listened_lately), Icons.History, lately.orEmpty()),
                Heard(stringResource(R.string.tv_music_listened_most), Icons.Flame, most.orEmpty()),
            ).filter { it.songs.isNotEmpty() }
            val newestTitle = stringResource(R.string.tv_music_newest)
            // The remote starts on the newest albums, which stand with the
            // tabs at the top of the page: it scrolls once the remote goes
            // below them.
            return TabContent(newest != null) { spot ->
                newest?.takeIf { it.isNotEmpty() }?.let { albums ->
                    wide("newest") {
                        CardRow(newestTitle, Icons.NewAlbum, albums, key = { it.id }, modifier = Modifier.toTheEdges(Gutter).head(spot.head, "newest")) { album ->
                            AlbumTile(
                                album,
                                onClick = { openAlbum(album.id) },
                                modifier = Modifier
                                    .remembered(spot.memory, "newest/${album.id}")
                                    .then(if (album == albums.first() && spot.firstTile != null) Modifier.focusRequester(spot.firstTile) else Modifier),
                            )
                        }
                    }
                }
                // What was listened to lately and most, side by side, each
                // scrolling in its own box, as the web's.
                if (heard.isNotEmpty()) {
                    wide("listened") {
                        Row(Modifier.toTheEdges(Gutter).padding(end = Gutter)) {
                            heard.forEach { one ->
                                HeardList(one, spot.memory, onPlay = { at -> listening.play(one.songs, at) }, modifier = Modifier.weight(1f))
                            }
                        }
                    }
                }
            }
        }
    }
    return TabContent(true, lay)
}

/** Songs the account listened to, under their name. */
private class Heard(val title: String, val icon: ImageVector, val songs: List<Song>)

/** How tall a list of what was listened to stands: its own box, which
 *  scrolls under the remote as the page does not. */
private val HEARD_TALL = 430.dp

/** One list of what was listened to: its name, then its songs numbered in
 *  their order, a press playing them from the one pressed. */
@Composable
private fun HeardList(heard: Heard, memory: FocusMemory, onPlay: (Int) -> Unit, modifier: Modifier) {
    Column(modifier) {
        RowHead(heard.title, heard.icon)
        LazyColumn(Modifier.padding(start = Gutter).height(HEARD_TALL)) {
            itemsIndexed(heard.songs, key = { at, song -> "$at/${song.id}" }) { at, song ->
                SongLine(
                    song,
                    number = "${at + 1}",
                    onClick = { onPlay(at) },
                    modifier = Modifier.remembered(memory, "${heard.title}/$at"),
                    withAlbum = false,
                    afterAnother = at > 0,
                )
            }
        }
    }
}

/** A list read a stretch at a time, as tiles, more read as the remote
 *  comes near the end of it. */
@Composable
private fun <T> paged(
    list: MusicList<T>,
    empty: String,
    key: (T) -> String,
    tile: @Composable (T, Modifier) -> Unit,
): LazyGridScope.(GridSpot) -> Unit {
    val reach = rememberReaching(list, list.items, list::reachedTo)
    if (list.failed) {
        return { wide("trouble") { Trouble(onRetry = list::retry) } }
    }
    val items = list.items
    return { spot ->
        tiles(items, empty, spot, key = key, onReach = reach, tile = tile)
    }
}

/** The songs of a library, read a stretch at a time, a line each; a press
 *  plays them from the one pressed. */
@Composable
private fun pagedSongs(
    list: MusicList<Song>,
    empty: String,
    onPlay: (List<Song>, Int) -> Unit,
): LazyGridScope.(GridSpot) -> Unit {
    val reach = rememberReaching(list, list.items, list::reachedTo)
    if (list.failed) {
        return { wide("trouble") { Trouble(onRetry = list::retry) } }
    }
    val songs = list.items
    return { spot ->
        songLines(songs, empty, spot, onReach = reach, onPlay = onPlay)
    }
}

/** The headings of what was found: albums, artists and songs. */
class FoundNames(val albums: String, val artists: String, val songs: String)

@Composable
fun foundNames() = FoundNames(tabName("albums"), tabName("artists"), tabName("songs"))

/**
 * Music found, by a search or among the account's hearts, as the web draws
 * it: albums and artists in rows, songs in lines, a press on one playing
 * them all from there; [empty] said when nothing was, if anything is to be.
 */
fun LazyGridScope.foundMusic(found: FoundMusic, names: FoundNames, empty: String?, spot: GridSpot, go: (Place) -> Unit, listening: Listening) {
    if (found.albums.isEmpty() && found.artists.isEmpty() && found.songs.isEmpty()) {
        empty?.let { wide("music/empty") { Empty(it) } }
        return
    }
    if (found.albums.isNotEmpty()) {
        wide("music/albums") {
            CardRow(names.albums, Icons.Music, found.albums, key = { it.id }, modifier = Modifier.toTheEdges(Gutter)) { album ->
                AlbumTile(album, onClick = { go(Place.Album(album.id)) }, modifier = Modifier.remembered(spot.memory, "album/${album.id}"))
            }
        }
    }
    if (found.artists.isNotEmpty()) {
        wide("music/artists") {
            CardRow(names.artists, Icons.People, found.artists, key = { it.id }, modifier = Modifier.toTheEdges(Gutter)) { artist ->
                ArtistTile(artist, onClick = { go(Place.Artist(artist.id)) }, modifier = Modifier.remembered(spot.memory, "artist/${artist.id}"))
            }
        }
    }
    if (found.songs.isNotEmpty()) {
        wide("music/songs") { RowHead(names.songs, Icons.Music, Modifier.toTheEdges(Gutter)) }
        songLines(found.songs, empty = null, spot) { songs, at -> listening.play(songs, at) }
    }
}

/** [items] as tiles of the grid, [empty] said when there are none. */
private fun <T> LazyGridScope.tiles(
    items: List<T>?,
    empty: String,
    spot: GridSpot,
    key: (T) -> String,
    onReach: (Int) -> Unit = {},
    tile: @Composable (T, Modifier) -> Unit,
) {
    if (items == null) {
        return
    }
    if (items.isEmpty()) {
        wide("empty") { Empty(empty) }
    }
    itemsIndexed(items, key = { _, item -> key(item) }) { at, item ->
        val named = key(item)
        tile(
            item,
            Modifier
                .remembered(spot.memory, named)
                .then(if (at == 0 && spot.firstTile != null) Modifier.focusRequester(spot.firstTile) else Modifier)
                .onFocusChanged { if (it.isFocused) onReach(at) }
                .firstLineHoldsTheHead(spot.grid, spot.head, named),
        )
    }
}

/** Songs a line each across the grid; a press plays them from the one
 *  pressed. */
private fun LazyGridScope.songLines(
    songs: List<Song>?,
    empty: String?,
    spot: GridSpot,
    onReach: (Int) -> Unit = {},
    onPlay: (List<Song>, Int) -> Unit,
) {
    if (songs == null) {
        return
    }
    if (songs.isEmpty() && empty != null) {
        wide("empty") { Empty(empty) }
    }
    itemsIndexed(songs, key = { _, song -> "song/${song.id}" }, span = { _, _ -> GridItemSpan(maxLineSpan) }) { at, song ->
        SongLine(
            song,
            number = "${at + 1}",
            onClick = { onPlay(songs, at) },
            afterAnother = at > 0,
            modifier = Modifier
                .remembered(spot.memory, "song/${song.id}")
                .onFocusChanged { if (it.isFocused) onReach(at) }
                .firstLineHoldsTheHead(spot.grid, spot.head, "song/${song.id}"),
        )
    }
}

/** Something across the whole width of the grid. */
private fun LazyGridScope.wide(key: String, content: @Composable () -> Unit) =
    item(key = key, span = { GridItemSpan(maxLineSpan) }) { content() }

@Composable
private fun Empty(said: String) {
    Text(said, fontSize = 16.sp, color = LocalPalette.current.textSoft, modifier = Modifier.padding(top = 8.dp))
}

/** A row reaching out over the grid's margin to the edges of the screen,
 *  where a row of the page stands, its own margin then lining it up. */
private fun Modifier.toTheEdges(margin: Dp): Modifier = layout { measurable, constraints ->
    val out = margin.roundToPx()
    val placed = measurable.measure(constraints.copy(minWidth = constraints.maxWidth + 2 * out, maxWidth = constraints.maxWidth + 2 * out))
    layout(constraints.maxWidth, placed.height) { placed.place(-out, 0) }
}

@Composable
private fun tabName(tab: String): String = stringResource(
    when (tab) {
        "for_you" -> R.string.tv_music_for_you
        "album_artists" -> R.string.tv_music_album_artists
        "artists" -> R.string.tv_music_artists
        "songs" -> R.string.tv_music_songs
        "playlists" -> R.string.tv_music_playlists
        "favourites" -> R.string.tv_music_favourites
        "genres" -> R.string.tv_music_genres
        else -> R.string.tv_music_albums
    },
)

@Composable
private fun sortName(order: String): String = stringResource(
    when (order) {
        "artist" -> R.string.tv_music_sort_artist
        "year" -> R.string.tv_music_sort_year
        "added" -> R.string.tv_music_sort_added
        "album" -> R.string.tv_music_sort_album
        else -> R.string.tv_music_sort_title
    },
)
