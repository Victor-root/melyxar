package app.melyxar.tv.requests

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.Asking
import app.melyxar.server.LiveWord
import app.melyxar.server.Refusal
import app.melyxar.server.RequestAccess
import app.melyxar.server.TitleRequest
import app.melyxar.server.askFor
import app.melyxar.server.myRequests
import app.melyxar.server.requestAccess
import app.melyxar.server.withdrawRequest
import java.io.IOException
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** The language the provider answers in: the screen's own. */
fun requestLanguage(): String = Locale.getDefault().language

/**
 * Whether this account may ask for titles, and the requests it made, held
 * once for every screen, as the web holds them: read again each time the
 * live line says the requests moved. Asking writes the request here once the
 * server has it; taking one back takes it out at once, and puts it back if
 * the server refuses.
 */
@Stable
class Requests(private val api: Api, private val scope: CoroutineScope, private val signedOut: () -> Unit) {
    /** Nothing until read. */
    var access by mutableStateOf<RequestAccess?>(null)
        private set

    /** Newest first; nothing until read, or when this account may not ask. */
    var mine by mutableStateOf<List<TitleRequest>?>(null)
        private set

    fun read() {
        scope.launch {
            try {
                val read = api.requestAccess()
                access = read
                mine = if (read.mayAsk) api.myRequests() else null
            } catch (refused: Refusal) {
                if (refused.signedOut) signedOut()
            } catch (_: IOException) {
                // Kept as it was: the next word of the line asks again.
            }
        }
    }

    /** Told by the live line: the requests moved, or the line opened again
     *  after nothing could be said to it. */
    fun heard(word: LiveWord) {
        if (word.name == "requests" || word.name == "open") read()
    }

    /** Throws what the server refused. */
    suspend fun ask(asking: Asking): TitleRequest {
        val made = api.askFor(asking)
        mine = listOf(made) + mine.orEmpty().filter { it.id != made.id }
        return made
    }

    /** Takes a request back, or clears a decided one from the list. */
    fun withdraw(id: String) {
        val before = mine
        mine = before?.filter { it.id != id }
        scope.launch {
            try {
                api.withdrawRequest(id)
            } catch (refused: Refusal) {
                if (refused.signedOut) signedOut()
                mine = before
            } catch (_: IOException) {
                mine = before
            }
        }
    }
}

val LocalRequests = staticCompositionLocalOf<Requests> { error("no requests here") }
