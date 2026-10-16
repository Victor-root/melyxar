package app.melyxar.tv.notes

import app.melyxar.server.Note
import app.melyxar.server.NoteSettings

/*
 * The part of an account's history the television holds, and how each change
 * moves it, as the web's (notifications/history.ts); and whether one may
 * show on the screen now (notifications/wording.ts).
 */

/** The notifications held, newest first; how many are unread over the
 *  whole history, the pages not yet read included; and whether older pages
 *  remain. */
data class Held(val notes: List<Note> = emptyList(), val unread: Int = 0, val more: Boolean = false)

/** Identifiers are ordered by time, written the same length: as text, the
 *  later is the greater. */
private fun List<Note>.newestFirst() = sortedByDescending { it.id }

/** Whether a notification belongs among those held, rather than in a page
 *  not read yet. */
private fun Held.within(id: String): Boolean = !more || notes.isEmpty() || id > notes.last().id

/** A notification that arrived, or came back unread. */
fun Held.arrived(note: Note): Held {
    val there = notes.firstOrNull { it.id == note.id }
    val becameUnread = if (!note.read && (there == null || there.read)) 1 else 0
    val others = notes.filter { it.id != note.id }
    return copy(
        notes = if (there != null || within(note.id)) (others + note).newestFirst() else notes,
        unread = unread + becameUnread,
    )
}

/** Marks notifications read or unread. One not held is counted all the
 *  same: the server only says what changed. */
fun Held.marked(ids: List<String>, read: Boolean): Held {
    val changes = ids.count { id -> notes.firstOrNull { it.id == id }?.read != read }
    return copy(
        notes = notes.map { if (it.id in ids) it.copy(read = read) else it },
        unread = (unread + if (read) -changes else changes).coerceAtLeast(0),
    )
}

/** The ids among [ids], or among all when none are named, held and unread. */
fun Held.unreadAmong(ids: List<String>?): List<String> =
    notes.filter { !it.read && (ids == null || it.id in ids) }.map { it.id }

/** An older page read, put after what is held. */
fun Held.olderPage(page: List<Note>, more: Boolean): Held {
    val known = notes.map { it.id }.toSet()
    return copy(notes = notes + page.filter { it.id !in known }, more = more)
}

private const val MINUTES_IN_A_DAY = 24 * 60

/** Whether a minute of the day falls in quiet hours, which may run across
 *  midnight. */
fun isQuiet(minute: Int, settings: NoteSettings): Boolean {
    val from = settings.quietFrom ?: return false
    val until = settings.quietUntil ?: return false
    val now = Math.floorMod(minute, MINUTES_IN_A_DAY)
    return if (from < until) now in from until until else now >= from || now < until
}

/** Whether a notification that wants the screen may have it now: during a
 *  film, and during quiet hours, only what is urgent shows, and only for an
 *  account that said it should. */
fun showsNow(note: Note, settings: NoteSettings, filmOnScreen: Boolean, minute: Int): Boolean {
    val urgent = note.priority || note.mandatory
    return when {
        filmOnScreen && !(urgent && settings.priorityWhilePlaying) -> false
        isQuiet(minute, settings) && !(urgent && settings.priorityWhileQuiet) -> false
        else -> true
    }
}
