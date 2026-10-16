package app.melyxar.server

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

/*
 * The account's notifications, as the server's views write them
 * (crates/server/src/notifications.rs), with only what the television reads.
 */

/** One series some episodes arrived in. */
@Serializable
data class SeriesArrived(val id: String, val title: String, val episodes: Int = 0)

/** What became of a title asked for: asked, accepted, refused or added. */
@Serializable
data class RequestNews(val what: String, val by: String = "", val seasons: List<Int> = emptyList(), val answer: String = "")

/** What a notification says, the fields of its [kind] filled: message,
 *  maintenance, new_content, deletion or request. */
@Serializable
data class Said(
    val kind: String = "",
    val title: String = "",
    val text: String = "",
    val library: String = "",
    val libraryName: String = "",
    val films: Int = 0,
    val filmTitles: List<String> = emptyList(),
    val series: List<SeriesArrived> = emptyList(),
    /** For a deletion: nothing when only the library forgot it, or how many
     *  files were found gone from the disk. */
    val checkedGone: Int? = null,
    val year: Int? = null,
    val overview: String? = null,
    val poster: String? = null,
    val news: RequestNews? = null,
)

/** One notification kept for this account. */
@Serializable
data class Note(
    val id: String,
    val kind: String,
    /** ok, attention, trouble or news. */
    val level: String = "news",
    val data: Said? = null,
    /** The work it opens, absent once the work is gone. */
    val workId: String? = null,
    val poster: List<Picture> = emptyList(),
    val priority: Boolean = false,
    val mandatory: Boolean = false,
    val sticky: Boolean = false,
    val shownForMs: Long? = null,
    val dueAt: String? = null,
    val createdAt: String = "",
    val read: Boolean = false,
)

@Serializable
data class NotesPage(val notifications: List<Note> = emptyList(), val unread: Int = 0, val more: Boolean = false)

/** Quiet hours, in minutes after midnight on this device's clock. */
@Serializable
data class NoteSettings(
    val quietFrom: Int? = null,
    val quietUntil: Int? = null,
    val priorityWhilePlaying: Boolean = true,
    val priorityWhileQuiet: Boolean = true,
)

@Serializable
data class NoteChoices(val settings: NoteSettings = NoteSettings())

/** A word of the live line about one notification. */
@Serializable
data class NoteHeard(val notification: Note, val kept: Boolean = true, val screen: Boolean = true)

/** A word of the live line about some notifications. */
@Serializable
data class NotesNamed(val ids: List<String> = emptyList())

/** The newest page of the history, or the one older than [before]. */
suspend fun Api.notes(before: String? = null): NotesPage =
    get(asking("/api/v1/notifications") { before?.let { addQueryParameter("before", it) } }, NotesPage.serializer())

/** Marks [ids] read, or every one when none is named. */
suspend fun Api.markNotesRead(ids: List<String>?) {
    send(
        "POST",
        "/api/v1/notifications/read",
        buildJsonObject { put("ids", ids?.let { JsonArray(it.map(::JsonPrimitive)) } ?: JsonNull) },
    )
}

suspend fun Api.noteChoices(): NoteChoices = get("/api/v1/notifications/choices", NoteChoices.serializer())
