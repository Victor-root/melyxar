package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Icons

/**
 * The card the remote is on, which the key of more options held down opens
 * a menu of, as the web's menu of a card.
 */
@Stable
class CardMenus {
    /** Not watched: only read when the key is held. */
    var focused: Card? = null

    var open by mutableStateOf<Card?>(null)

    /** Opens the menu of the card the remote is on; false when it is on none. */
    fun openFocused(): Boolean {
        open = focused ?: return false
        return true
    }
}

val LocalCardMenus = staticCompositionLocalOf { CardMenus() }

/** Makes [card] the one whose menu the key of more options opens while the
 *  remote is on it. */
@Composable
fun Modifier.offersMenuOf(card: Card): Modifier {
    val menus = LocalCardMenus.current
    return onFocusChanged {
        if (it.hasFocus) {
            menus.focused = card
        } else if (menus.focused?.id == card.id) {
            menus.focused = null
        }
    }
}

private class Entry(val name: String, val icon: ImageVector, val act: () -> Unit)

/** The menu of [card] over the screen: play it, open its page, and the marks
 *  an account puts on it. */
@Composable
fun CardMenu(card: Card, onClose: () -> Unit) {
    val chrome = LocalChrome.current
    val marks = LocalMarks.current
    val whereabouts = marks.whereabouts(card)
    val favourite = marks.favourite(card)
    val later = marks.watchLater(card)
    val watched = whereabouts.seen == "watched"
    val entries = listOfNotNull(
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
    Popup(alignment = Alignment.Center, onDismissRequest = onClose, properties = PopupProperties(focusable = true)) {
        ChoiceList(
            title = card.title,
            choices = entries.map { it.name },
            chosen = null,
            onChoose = { entries[it].act() },
            onClose = onClose,
            icons = entries.map { it.icon },
        )
    }
}
