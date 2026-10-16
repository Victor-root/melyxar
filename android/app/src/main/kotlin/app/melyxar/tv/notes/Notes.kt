package app.melyxar.tv.notes

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.LiveWord
import app.melyxar.server.Note
import app.melyxar.server.NoteHeard
import app.melyxar.server.NoteSettings
import app.melyxar.server.NotesNamed
import app.melyxar.server.Refusal
import app.melyxar.server.markNotesRead
import app.melyxar.server.noteChoices
import app.melyxar.server.notes
import app.melyxar.server.serverJson
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerializationException
import java.io.IOException
import java.util.Calendar

/** How many notifications are said on the screen at once. */
private const val SHOWN_AT_MOST = 3

/**
 * This account's notifications, held once for the whole television, as the
 * web holds them: the bell, its page and the corner of the screen show the
 * same history. Reading one is shown at once and put back if the server
 * refuses; what changes on another device arrives by the live line. Each the
 * server says is for the screen is said in the corner, unless a film is
 * playing or the account's quiet hours hold.
 */
@Stable
class Notes(private val api: Api, private val scope: CoroutineScope, private val signedOut: () -> Unit) {
    var held by mutableStateOf(Held())
        private set

    /** Whether the first page has been read. */
    var loaded by mutableStateOf(false)
        private set

    /** What is said in the corner of the screen, the newest last. */
    val shown = mutableStateListOf<Note>()

    /** Whether a film has the screen, which only what is urgent may take. */
    var filmOnScreen = false

    private var settings = NoteSettings()

    /** Asks the server, the account sent back to the door if its session
     *  ended; nothing when it could not be asked. */
    private suspend fun <T> asking(ask: suspend Api.() -> T): T? = try {
        api.ask()
    } catch (refused: Refusal) {
        if (refused.signedOut) signedOut()
        null
    } catch (_: IOException) {
        null
    }

    fun readAfresh() {
        scope.launch {
            asking { notes() }?.let { page ->
                held = Held(page.notifications, page.unread, page.more)
                loaded = true
            }
        }
    }

    private fun readChoices() {
        scope.launch { asking { noteChoices() }?.let { settings = it.settings } }
    }

    fun loadOlder() {
        val oldest = held.notes.lastOrNull()?.id ?: return
        scope.launch {
            asking { notes(oldest) }?.let { page -> held = held.olderPage(page.notifications, page.more) }
        }
    }

    /** Marks [ids] read, or every one when none is named. */
    fun markRead(ids: List<String>? = null) {
        val changing = held.unreadAmong(ids)
        if (changing.isEmpty()) {
            return
        }
        held = held.marked(changing, read = true)
        scope.launch {
            if (asking { markNotesRead(ids) } == null) {
                held = held.marked(changing, read = false)
            }
        }
    }

    /** Takes [note] out of the corner. */
    fun dismiss(note: Note) {
        shown.removeAll { it.id == note.id }
    }

    /** Takes everything out of the corner, the page of notifications open. */
    fun dismissAll() = shown.clear()

    /** A word of the live line, its opening included. */
    fun heard(word: LiveWord) {
        when (word.name) {
            "open", "notifications_missed" -> {
                readAfresh()
                if (word.name == "open") readChoices()
            }
            "notification_choices" -> readChoices()
            "notification" -> read(word, NoteHeard.serializer())?.let { heard ->
                if (heard.kept) held = held.arrived(heard.notification)
                if (heard.screen) show(heard.notification)
            }
            "notification_recalled" -> read(word, NoteHeard.serializer())?.let { heard ->
                held = held.arrived(heard.notification)
                show(heard.notification)
            }
            "notifications_read", "notifications_unread" -> read(word, NotesNamed.serializer())?.let { named ->
                held = held.marked(named.ids, read = word.name == "notifications_read")
            }
            // Gone from another device: whether it was unread is only known
            // by reading the history again.
            "notifications_removed" -> readAfresh()
        }
    }

    private fun <T> read(word: LiveWord, reading: DeserializationStrategy<T>): T? = try {
        serverJson.decodeFromString(reading, word.data)
    } catch (_: SerializationException) {
        null
    }

    private fun show(note: Note) {
        val now = Calendar.getInstance()
        if (!showsNow(note, settings, filmOnScreen, now.get(Calendar.HOUR_OF_DAY) * 60 + now.get(Calendar.MINUTE))) {
            return
        }
        shown.removeAll { it.id == note.id }
        shown += note
        while (shown.size > SHOWN_AT_MOST) shown.removeAt(0)
    }
}

val LocalNotes = staticCompositionLocalOf<Notes> { error("no notifications") }
