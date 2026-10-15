package app.melyxar.tv.music

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.melyxar.server.Api
import app.melyxar.server.MusicPage
import app.melyxar.server.Refusal
import app.melyxar.server.Slice
import app.melyxar.tv.inside.LocalAnswers
import app.melyxar.tv.inside.LocalMarks
import app.melyxar.tv.inside.LocalSignedOut
import app.melyxar.tv.look.LocalServer
import java.io.IOException

/** How much of a list is read at once, as the web reads it. */
private const val READ_AT_ONCE = 200

/** How near the end of what is read the remote comes before more is asked
 *  for, in items. */
private const val AHEAD = 24

/**
 * A long list of music, albums, artists or songs, read a stretch at a time
 * as the remote goes down it, well before it gets there. Its last answer is
 * kept while another screen is over it, and shown at once on coming back.
 */
@Stable
class MusicList<T>(
    private val api: Api,
    private val question: String,
    private val answers: MutableMap<String, Any?>,
    private val signedOut: () -> Unit,
    private val reading: suspend Api.(Slice) -> MusicPage<T>,
) {
    @Suppress("UNCHECKED_CAST")
    private val kept = answers[question] as MusicPage<T>?

    var items by mutableStateOf(kept?.items)
        private set
    private var total = kept?.total ?: 0

    var failed by mutableStateOf(false)
        private set

    /** How many times the list was asked for again after failing. */
    var tries by mutableIntStateOf(0)
        private set

    fun retry() {
        tries += 1
    }

    /** Reads the list again from its top, as far as it had been read. */
    suspend fun refresh() {
        read(Slice(0, (items?.size ?: 0).coerceAtLeast(READ_AT_ONCE)), emptyList())
    }

    /** The remote is on the item at [at]: more is read when it comes near
     *  the end of what is. */
    suspend fun reachedTo(at: Int) {
        val read = items ?: return
        if (read.size < total && at >= read.size - AHEAD) {
            read(Slice(read.size, READ_AT_ONCE), read)
        }
    }

    private suspend fun read(slice: Slice, adding: List<T>) {
        try {
            val page = api.reading(slice)
            val whole = MusicPage(adding + page.items, page.total)
            answers[question] = whole
            total = whole.total
            items = whole.items
            failed = false
        } catch (refused: Refusal) {
            if (refused.signedOut) signedOut() else failed = items == null
        } catch (_: IOException) {
            failed = items == null
        }
    }
}

/** The list [question] names, read again whenever the screen comes on or
 *  what the libraries hold moved. */
@Composable
fun <T> rememberMusicList(question: String, comebacks: Int, reading: suspend Api.(Slice) -> MusicPage<T>): MusicList<T> {
    val api = LocalServer.current
    val answers = LocalAnswers.current
    val signedOut = LocalSignedOut.current
    val moved = LocalMarks.current.rowsMoved
    val list = remember(api, question) { MusicList(api, question, answers, signedOut, reading) }
    LaunchedEffect(list, comebacks, moved, list.tries) { list.refresh() }
    return list
}
