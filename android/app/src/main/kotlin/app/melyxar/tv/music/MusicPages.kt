package app.melyxar.tv.music

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.ArtistPage
import app.melyxar.server.Picture
import app.melyxar.server.Playlist
import app.melyxar.server.Refusal
import app.melyxar.server.Song
import app.melyxar.server.album
import app.melyxar.server.artist
import app.melyxar.server.artistSongs
import app.melyxar.server.playlist
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.theme.parseHex
import app.melyxar.tv.inside.CardRow
import app.melyxar.tv.inside.FocusMemory
import app.melyxar.tv.inside.Loaded
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.RowHead
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.Trouble
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.inside.section
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.RoundMark
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised
import java.io.IOException
import kotlinx.coroutines.launch

/** How wide the picture at the head of an album, an artist or a playlist is. */
private val HERO_PICTURE = 220.dp

/** An album, as the web's page: its cover and what it is, then its songs,
 *  a heading for each disc when it has more than one. */
@Composable
fun AlbumScreen(id: String, comebacks: Int) {
    MusicScreen("music:album:$id", comebacks, { album(id) }) { album, memory, first ->
        val listening = LocalListening.current
        val hearts = LocalHearts.current
        val discs = album.tracks.groupBy { it.disc ?: 1 }
        MusicPage(
            head = {
                Hero(
                    pictures = album.cover,
                    color = album.color,
                    letter = album.title.take(1),
                    shape = CardShape,
                    title = album.title,
                    lines = listOfNotNull(
                        artistsOf(album.artists, album.compilation),
                        listOfNotNull(album.year?.toString(), howManySongs(album.tracks), howLong(album.tracks)).joinToString(" · "),
                    ),
                ) {
                    PlayAndShuffle(album.tracks, first)
                    Heart(album.id, hearts)
                }
            },
        ) {
            discs.forEach { (disc, songs) ->
                if (discs.size > 1) {
                    item("disc/$disc") { RowHead(stringResource(R.string.tv_music_disc, disc), null, Modifier.padding(top = 18.dp)) }
                }
                songLines(songs, album.tracks, memory, listening, numbered = true)
            }
        }
    }
}

/** An artist, as the web's page: their picture and how much of theirs is
 *  here, their albums, then those they appear on. */
@Composable
fun ArtistScreen(id: String, comebacks: Int) {
    MusicScreen("music:artist:$id", comebacks, { artist(id) }) { artist, memory, first ->
        val chrome = LocalChrome.current
        val hearts = LocalHearts.current
        MusicPage(
            head = {
                Hero(
                    pictures = artist.picture,
                    color = artist.color,
                    letter = artist.name.take(1),
                    shape = CircleShape,
                    title = artist.name,
                    lines = listOf(
                        listOf(
                            pluralStringResource(R.plurals.tv_music_album_count, artist.albums, artist.albums),
                            pluralStringResource(R.plurals.tv_music_song_count, artist.songs, artist.songs),
                        ).joinToString(" · "),
                    ),
                ) {
                    PlayAllOf(artist, first)
                    Heart(artist.id, hearts)
                }
            },
        ) {
            listOf(R.string.tv_music_albums to artist.theirAlbums, R.string.tv_music_appears_on to artist.appearsOn).forEach { (title, albums) ->
                section("albums/$title", albums) {
                    CardRow(stringResource(title), Icons.Music, albums, key = { it.id }) { album ->
                        AlbumTile(album, onClick = { chrome.go(Place.Album(album.id)) }, modifier = Modifier.remembered(memory, "$title/${album.id}"))
                    }
                }
            }
        }
    }
}

/** A playlist of the account, as the web's page: what it is, then its
 *  songs. */
@Composable
fun PlaylistScreen(id: String, comebacks: Int) {
    MusicScreen("music:playlist:$id", comebacks, { playlist(id) }) { playlist: Playlist, memory, first ->
        val listening = LocalListening.current
        MusicPage(
            head = {
                Hero(
                    pictures = playlist.tracks.firstOrNull()?.cover.orEmpty(),
                    color = null,
                    letter = playlist.name.take(1),
                    shape = CardShape,
                    title = playlist.name,
                    lines = listOf(listOf(howManySongs(playlist.tracks), howLong(playlist.tracks)).joinToString(" · ")),
                ) {
                    PlayAndShuffle(playlist.tracks, first)
                }
            },
        ) {
            if (playlist.tracks.isEmpty()) {
                item("empty") {
                    Text(
                        stringResource(R.string.tv_music_playlist_empty),
                        fontSize = 16.sp,
                        color = LocalPalette.current.textSoft,
                        modifier = Modifier.padding(horizontal = Gutter),
                    )
                }
            }
            songLines(playlist.tracks, playlist.tracks, memory, listening, numbered = false)
        }
    }
}

/** A page of music asked of the server: shown once it is in, a way to try
 *  again when it does not come, the remote starting on its first button. */
@Composable
private fun <T> MusicScreen(
    question: String,
    comebacks: Int,
    ask: suspend app.melyxar.server.Api.() -> T,
    page: @Composable (T, FocusMemory, FocusRequester) -> Unit,
) {
    var tries by remember { mutableIntStateOf(0) }
    val loaded = rememberLoaded(question, comebacks, tries, ask = ask).value
    val shown = loaded.ready
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        when {
            shown != null -> page(shown, memory, first)
            loaded is Loaded.Failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    RestoreFocus(memory, first, ready = shown != null)
}

/** The page under the top bar: [head] staying shown whole while the remote
 *  is in it, then what the page lists, scrolling under the remote. */
@Composable
private fun MusicPage(head: @Composable () -> Unit, content: LazyListScope.() -> Unit) {
    val list = rememberLazyListState()
    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { page ->
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 40.dp), modifier = Modifier.fillMaxSize()) {
            item("top") {
                Column(Modifier.head(page, "top")) {
                    TopBar()
                    head()
                }
            }
            content()
        }
    }
}

/** The picture and the name of an album, an artist or a playlist, what it
 *  is under them, and the buttons that play it. */
@Composable
private fun Hero(
    pictures: List<Picture>,
    color: String?,
    letter: String,
    shape: Shape,
    title: String,
    lines: List<String>,
    actions: @Composable RowScope.() -> Unit,
) {
    val palette = LocalPalette.current
    Row(
        modifier = Modifier.padding(start = Gutter, end = Gutter, top = 30.dp, bottom = 30.dp),
        horizontalArrangement = Arrangement.spacedBy(36.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .size(HERO_PICTURE)
                .boxShadow(shape, palette.shadowRaised)
                .clip(shape)
                .background(color?.let(::parseHex)?.let(::Color) ?: palette.surfaceRaised),
            contentAlignment = Alignment.Center,
        ) {
            if (pictures.isEmpty()) {
                Text(letter.uppercase(), fontFamily = MelyxarDisplay, fontSize = 72.sp, color = palette.text.copy(alpha = 0.3f))
            } else {
                ServerPicture(pictures, HERO_PICTURE, Modifier.fillMaxSize())
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.weight(1f)) {
            Text(
                title,
                fontSize = 36.sp,
                lineHeight = 40.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            lines.filter { it.isNotBlank() }.forEach {
                Text(it, fontSize = 16.sp, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(top = 10.dp),
                content = actions,
            )
        }
    }
}

/** Play, from the first song, and shuffle. */
@Composable
private fun PlayAndShuffle(songs: List<Song>, first: FocusRequester) {
    val listening = LocalListening.current
    AccentButton(stringResource(R.string.tv_music_play), onClick = { listening.play(songs) }, modifier = Modifier.focusRequester(first))
    PlainButton(stringResource(R.string.tv_music_shuffle), onClick = { listening.play(songs, songs.indices.random(), shuffle = true) }, icon = Icons.Shuffle)
}

/** Every song of an artist played, or shuffled, once they are fetched. */
@Composable
private fun PlayAllOf(artist: ArtistPage, first: FocusRequester) {
    val api = LocalServer.current
    val listening = LocalListening.current
    val scope = rememberCoroutineScope()
    val start = { shuffle: Boolean ->
        scope.launch {
            try {
                val songs = api.artistSongs(artist.id)
                if (songs.isNotEmpty()) {
                    listening.play(songs, if (shuffle) songs.indices.random() else 0, shuffle)
                }
            } catch (_: Refusal) {
                // Nothing started: the button is there to be pressed again.
            } catch (_: IOException) {
                // Nor with the network gone.
            }
        }
    }
    AccentButton(stringResource(R.string.tv_music_play_all), onClick = { start(false) }, modifier = Modifier.focusRequester(first))
    PlainButton(stringResource(R.string.tv_music_shuffle), onClick = { start(true) }, icon = Icons.Shuffle)
}

/** The heart of an album, an artist or a song. */
@Composable
fun Heart(id: String, hearts: Hearts) {
    val on = hearts.of(id)
    RoundMark(if (on) Icons.HeartFull else Icons.Heart, label = stringResource(R.string.tv_music_heart), on = on, onClick = { hearts.set(id, !on) })
}

/** Songs a line each; a press plays [all] from the one pressed. */
private fun LazyListScope.songLines(songs: List<Song>, all: List<Song>, memory: FocusMemory, listening: Listening, numbered: Boolean) {
    itemsIndexed(songs, key = { _, song -> "song/${song.id}" }) { at, song ->
        SongLine(
            song,
            number = if (numbered) song.track?.toString() ?: "${at + 1}" else null,
            onClick = { listening.play(all, all.indexOf(song)) },
            modifier = Modifier.padding(horizontal = Gutter).remembered(memory, "song/${song.id}"),
            withAlbum = !numbered,
            afterAnother = at > 0,
        )
    }
}

@Composable
private fun howManySongs(songs: List<Song>): String = pluralStringResource(R.plurals.tv_music_song_count, songs.size, songs.size)

@Composable
private fun howLong(songs: List<Song>): String = stringResource(R.string.tv_music_minutes, (songs.sumOf { it.seconds ?: 0 } / 60).toInt())
