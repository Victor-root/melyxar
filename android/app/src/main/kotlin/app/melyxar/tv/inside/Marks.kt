package app.melyxar.tv.inside

import androidx.compose.runtime.MutableState
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.Card
import app.melyxar.server.Refusal
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import java.io.IOException

/** Where somebody is in a work: whether they watched it, where to carry on
 *  from, in seconds, and for a series or a season how many episodes are
 *  left. */
data class Whereabouts(val seen: String, val resume: Long?, val unwatched: Int)

/** What was said about a work, and what the server had sent when it was. */
data class Said<T>(val said: T, val over: T)

/** What has been said about one work since the television showed it. */
data class Mark(
    val watched: Said<Whereabouts>? = null,
    val favourite: Boolean? = null,
    /** Seeing a work takes it off the list on the server's side, so a card
     *  that has since come back saying otherwise is the server's newer word. */
    val watchLater: Said<Boolean>? = null,
)

/** Where a work stands once marked watched or not, from where it stood, as
 *  the server holds it. Unwatched keeps where it was stopped: saying it was
 *  not watched after all moves nobody back to the beginning. */
fun markedWatched(watched: Boolean, before: Whereabouts, episodes: Int): Whereabouts = when {
    watched -> Whereabouts("watched", null, 0)
    else -> Whereabouts(if (before.resume != null) "in_progress" else "not_started", before.resume, episodes)
}

/** Where a work stands on a screen: what was said about it, while the
 *  server's answer is still the one it was said over. A fresher answer is
 *  the server's own account of what happened since, and wins. */
fun whereaboutsOf(sent: Whereabouts, said: Said<Whereabouts>?): Whereabouts = if (said != null && said.over == sent) said.said else sent

private fun sentOf(card: Card) = Whereabouts(card.seen, card.resumeFromSeconds, card.unwatched)

/**
 * What this account has made of each work, held once for every screen of
 * the television, as the web's store: a press writes it here before the
 * server answers, put back if it refuses, and every card, button and list
 * reads it here rather than on the card the server sent. A card is drawn
 * again when its own work changes, and not when another one does.
 */
@Stable
class Marks(private val api: Api, private val scope: CoroutineScope, private val signedOut: () -> Unit) {
    private val said = HashMap<String, MutableState<Mark?>>()

    /** Bumped whenever something changes what the server's rows and grids
     *  hold: whatever shows one asks for it again. */
    var rowsMoved by mutableIntStateOf(0)
        private set

    private fun markOf(id: String) = said.getOrPut(id) { mutableStateOf(null) }

    fun whereabouts(card: Card): Whereabouts = whereaboutsOf(sentOf(card), markOf(card.id).value?.watched)

    fun favourite(card: Card): Boolean = markOf(card.id).value?.favourite ?: card.favourite

    fun watchLater(card: Card): Boolean =
        markOf(card.id).value?.watchLater?.takeIf { it.over == card.watchLater }?.said ?: card.watchLater

    fun rowsHaveMoved() {
        rowsMoved += 1
    }

    fun setWatched(card: Card, watched: Boolean) {
        val before = markOf(card.id).value
        val sent = sentOf(card)
        say(
            card.id,
            before.orEmpty().copy(
                watched = Said(markedWatched(watched, whereaboutsOf(sent, before?.watched), card.episodes), sent),
                // Watched is what later was waiting for: the server takes
                // it off the list too.
                watchLater = if (watched) Said(false, card.watchLater) else before?.watchLater,
            ),
        )
        // A work ticked off leaves the row of what is unfinished, and the
        // episode a series waits on is another: read again once the server
        // holds it.
        tell(card, "watched", watched, before, then = ::rowsHaveMoved)
    }

    fun setFavourite(card: Card, favourite: Boolean) {
        val before = markOf(card.id).value
        say(card.id, before.orEmpty().copy(favourite = favourite))
        tell(card, "favourite", favourite, before)
    }

    fun setWatchLater(card: Card, later: Boolean) {
        val before = markOf(card.id).value
        say(card.id, before.orEmpty().copy(watchLater = Said(later, card.watchLater)))
        tell(card, "watch-later", later, before)
    }

    private fun say(id: String, mark: Mark) {
        markOf(id).value = mark
    }

    /** Tells the server, and puts the work back as it was if it refuses. */
    private fun tell(card: Card, mark: String, on: Boolean, before: Mark?, then: () -> Unit = {}) {
        scope.launch {
            try {
                api.mark(card.id, mark, on)
                then()
            } catch (refused: Refusal) {
                say(card.id, before.orEmpty())
                if (refused.signedOut) signedOut()
            } catch (_: IOException) {
                say(card.id, before.orEmpty())
            }
        }
    }

    private fun Mark?.orEmpty() = this ?: Mark()
}

val LocalMarks = staticCompositionLocalOf<Marks> { error("no marks") }
