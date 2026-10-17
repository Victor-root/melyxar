package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.melyxar.R
import app.melyxar.server.Card
import app.melyxar.server.Refusal
import app.melyxar.server.Song
import app.melyxar.server.addToPlaylist
import app.melyxar.server.playlists
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.music.LocalListening
import java.io.IOException
import kotlinx.coroutines.launch

/** What a menu is opened for: a work, or a song. */
sealed interface Offered {
    val key: String

    data class Work(val card: Card) : Offered {
        override val key get() = "work/${card.id}"
    }

    data class Tune(val song: Song) : Offered {
        override val key get() = "song/${song.id}"
    }
}

/**
 * What the remote is on, which the key of more options held down opens a
 * menu of, as the web's menu of a card or of a song.
 */
@Stable
class CardMenus {
    /** Not watched: only read when the key is held. */
    var focused: Offered? = null

    var open by mutableStateOf<Offered?>(null)

    /** Opens the menu of what the remote is on; false when it is on nothing
     *  that has one. */
    fun openFocused(): Boolean {
        open = focused ?: return false
        return true
    }
}

val LocalCardMenus = staticCompositionLocalOf { CardMenus() }

/** Makes [offered] what the key of more options opens a menu of while the
 *  remote is on it. */
@Composable
fun Modifier.offersMenuOf(offered: Offered): Modifier {
    val menus = LocalCardMenus.current
    return onFocusChanged {
        if (it.hasFocus) {
            menus.focused = offered
        } else if (menus.focused?.key == offered.key) {
            menus.focused = null
        }
    }
}

/** A line of a menu; one that [leadsOn] opens what follows in its place,
 *  and leaves the menu open. */
private class Entry(val name: String, val icon: ImageVector, val leadsOn: Boolean = false, val act: () -> Unit)

/** The menu of [offered] over the screen. */
@Composable
fun CardMenu(offered: Offered, onClose: () -> Unit) {
    var choosingList by remember(offered.key) { mutableStateOf(false) }
    val (title, entries) = when (offered) {
        is Offered.Work -> offered.card.title to workEntries(offered.card)
        is Offered.Tune -> offered.song.title to songEntries(offered.song, onAddToPlaylist = { choosingList = true })
    }
    Popup(alignment = Alignment.Center, onDismissRequest = onClose, properties = PopupProperties(focusable = true)) {
        if (choosingList && offered is Offered.Tune) {
            PlaylistChoice(offered.song, onClose)
        } else {
            ChoiceList(
                title = title,
                choices = entries.map { it.name },
                chosen = null,
                onChoose = { at ->
                    entries[at].act()
                    if (!entries[at].leadsOn) onClose()
                },
                onClose = onClose,
                leadsOn = true,
                icons = entries.map { it.icon },
            )
        }
    }
}

/** Play it, open its page, and the marks an account puts on it. */
@Composable
private fun workEntries(card: Card): List<Entry> {
    val chrome = LocalChrome.current
    val marks = LocalMarks.current
    val whereabouts = marks.whereabouts(card)
    val favourite = marks.favourite(card)
    val later = marks.watchLater(card)
    val watched = whereabouts.seen == "watched"
    return listOfNotNull(
        card.source?.let { source ->
            val resume = whereabouts.resume
            Entry(stringResource(if (resume != null) R.string.tv_resume else R.string.tv_play), Icons.Play) {
                chrome.play(Play(card.id, source, resume?.toDouble()))
            }
        },
        Entry(stringResource(R.string.tv_open), Icons.Film) { chrome.go(Place.Opened(card.id)) },
        Entry(stringResource(if (favourite) R.string.tv_menu_unfavourite else R.string.tv_menu_favourite), if (favourite) Icons.HeartFull else Icons.Heart) {
            marks.setFavourite(card, !favourite)
        },
        Entry(stringResource(if (later) R.string.tv_menu_unwatch_later else R.string.tv_watch_later), Icons.Clock) {
            marks.setWatchLater(card, !later)
        },
        Entry(stringResource(if (watched) R.string.tv_menu_mark_unwatched else R.string.tv_menu_mark_watched), Icons.Tick) {
            marks.setWatched(card, !watched)
        }.takeIf { card.watchedMarks },
    )
}

/** Play it next or last, put it in a playlist, reach its album and its
 *  artist. */
@Composable
private fun songEntries(song: Song, onAddToPlaylist: () -> Unit): List<Entry> {
    val chrome = LocalChrome.current
    val listening = LocalListening.current
    return listOfNotNull(
        Entry(stringResource(R.string.tv_menu_play_next), Icons.NextSong) { listening.playNext(listOf(song)) },
        Entry(stringResource(R.string.tv_menu_play_last), Icons.Queue) { listening.playLast(listOf(song)) },
        Entry(stringResource(R.string.tv_menu_add_to_playlist), Icons.Playlist, leadsOn = true, act = onAddToPlaylist),
        song.album?.let { album ->
            Entry(stringResource(R.string.tv_menu_go_to_album), Icons.Music) { chrome.go(Place.Album(album.id)) }
        },
        song.artists.firstOrNull()?.let { artist ->
            Entry(stringResource(R.string.tv_menu_go_to_artist), Icons.People) { chrome.go(Place.Artist(artist.id)) }
        },
    )
}

/** This account's playlists, [song] put at the end of the one chosen, the
 *  list closing once the server has it. */
@Composable
private fun PlaylistChoice(song: Song, onClose: () -> Unit) {
    val api = LocalServer.current
    val marks = LocalMarks.current
    val scope = rememberCoroutineScope()
    val playlists = rememberLoaded("music:playlists") { playlists() }.value.ready ?: return
    ChoiceList(
        title = stringResource(R.string.tv_menu_add_to_playlist),
        choices = playlists.map { it.name },
        chosen = null,
        onChoose = { at ->
            scope.launch {
                try {
                    api.addToPlaylist(playlists[at].id, listOf(song.id))
                    marks.rowsHaveMoved()
                } catch (_: Refusal) {
                    // Left as it was, as the web leaves it.
                } catch (_: IOException) {
                    // Nor with the network gone.
                }
                onClose()
            }
        },
        onClose = onClose,
        leadsOn = true,
        note = stringResource(R.string.tv_menu_no_playlist).takeIf { playlists.isEmpty() },
        icons = playlists.map { Icons.Playlist },
    )
}
