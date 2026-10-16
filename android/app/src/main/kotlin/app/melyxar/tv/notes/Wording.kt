package app.melyxar.tv.notes

import android.content.res.Resources
import app.melyxar.R
import app.melyxar.server.Note
import app.melyxar.server.Picture
import app.melyxar.server.Said

/** How many titles are named before the rest are counted. */
private const val NAMED_AT_MOST = 3

/** The season holding what was made outside the seasons. */
private const val SPECIALS = 0

/** Where pressing a notification leads on a television. */
sealed interface Leads {
    data class Work(val id: String) : Leads

    data class Library(val id: String) : Leads
}

/** A notification put into words, where pressing it leads, and the poster
 *  it wears. */
data class NoteSaid(val title: String, val detail: String?, val leads: Leads?, val poster: List<Picture>)

/** The words notifications are said in, kept apart so the saying can be
 *  tested without a device. */
interface Wording {
    fun said(id: Int, vararg args: Any): String

    fun counted(id: Int, count: Int, vararg args: Any): String

    /** An instant the server wrote, as a reader writes a day and a time. */
    fun moment(written: String): String?
}

/** The words of the device's own resources. */
class ResourceWording(private val resources: Resources, private val moment: (String) -> String?) : Wording {
    override fun said(id: Int, vararg args: Any): String = resources.getString(id, *args)

    override fun counted(id: Int, count: Int, vararg args: Any): String = resources.getQuantityString(id, count, *args)

    override fun moment(written: String): String? = moment.invoke(written)
}

/** A few titles named, the rest counted. */
private fun Wording.someOf(titles: List<String>, total: Int): String {
    val named = titles.take(NAMED_AT_MOST).joinToString(", ")
    val rest = total - minOf(titles.size, NAMED_AT_MOST)
    return if (rest > 0) counted(R.plurals.tv_notes_and_more, rest, named, rest) else named
}

private fun Wording.seasons(numbers: List<Int>): String =
    numbers.joinToString(", ") { if (it == SPECIALS) said(R.string.tv_specials) else said(R.string.tv_season, it) }

/** What became of a title asked for; a television only leads to the title
 *  once it is here. */
private fun Wording.request(told: Said, work: Leads?, poster: List<Picture>): NoteSaid {
    val named = told.year?.let { "${told.title} ($it)" } ?: told.title
    fun about(first: String? = null) = listOfNotNull(first?.takeIf { it.isNotEmpty() }, told.overview?.takeIf { it.isNotEmpty() })
        .joinToString(" · ")
        .ifEmpty { null }
    val news = told.news
    return when (news?.what) {
        "asked" -> NoteSaid(
            said(R.string.tv_notes_request_asked, news.by, named),
            about(news.seasons.takeIf { it.isNotEmpty() }?.let { seasons(it) }),
            null,
            poster,
        )
        "accepted" -> NoteSaid(said(R.string.tv_notes_request_accepted, named), about(), null, poster)
        "refused" -> NoteSaid(said(R.string.tv_notes_request_refused, named), about(news.answer), null, poster)
        else -> NoteSaid(said(R.string.tv_notes_request_added, named), about(), work, poster)
    }
}

/** What [note] says, as the web's notifications say it. */
fun Wording.say(note: Note): NoteSaid {
    val told = note.data
    val work = note.workId?.let { Leads.Work(it) }
    // Its work's poster, or failing that the one the notification names.
    val poster = note.poster.ifEmpty { listOfNotNull(told?.poster?.let { Picture(it) }) }
    return when (told?.kind) {
        "message" -> NoteSaid(told.title, told.text.ifEmpty { null }, null, poster)
        "maintenance" -> {
            val at = note.dueAt?.let(::moment)?.let { said(R.string.tv_notes_maintenance_at, it) }
            NoteSaid(told.title, listOfNotNull(at, told.text.ifEmpty { null }).joinToString(" · ").ifEmpty { null }, null, poster)
        }
        "new_content" -> newContent(told, work, poster)
        "deletion" -> when (val gone = told.checkedGone) {
            null -> NoteSaid(said(R.string.tv_notes_removed_library), said(R.string.tv_notes_removed_library_why), null, poster)
            0 -> NoteSaid(said(R.string.tv_notes_removed_disk), said(R.string.tv_notes_removed_disk_none), null, poster)
            else -> NoteSaid(said(R.string.tv_notes_removed_disk), counted(R.plurals.tv_notes_removed_disk_checked, gone, gone), null, poster)
        }
        "request" -> request(told, work, poster)
        else -> NoteSaid(said(R.string.tv_notes_unreadable), null, null, poster)
    }
}

private fun Wording.newContent(told: Said, work: Leads?, poster: List<Picture>): NoteSaid {
    val library = Leads.Library(told.library)
    val series = told.series
    return when {
        series.isEmpty() && told.films == 1 ->
            NoteSaid(said(R.string.tv_notes_new_film, told.filmTitles.firstOrNull().orEmpty()), told.libraryName, work, poster)
        series.isEmpty() ->
            NoteSaid(counted(R.plurals.tv_notes_new_films, told.films, told.films), someOf(told.filmTitles, told.films), library, poster)
        told.films == 0 && series.size == 1 -> {
            val one = series.single()
            val title = if (one.episodes == 1) {
                said(R.string.tv_notes_new_episode, one.title)
            } else {
                counted(R.plurals.tv_notes_new_episodes, one.episodes, one.episodes, one.title)
            }
            NoteSaid(title, told.libraryName, Leads.Work(one.id), poster)
        }
        else -> NoteSaid(
            (told.films + series.sumOf { it.episodes }).let { count -> counted(R.plurals.tv_notes_new_many, count, count, told.libraryName) },
            someOf(series.map { it.title } + told.filmTitles, series.size + told.films),
            library,
            poster,
        )
    }
}
