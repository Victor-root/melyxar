package app.melyxar.tv.notes

import app.melyxar.R
import app.melyxar.server.Note
import app.melyxar.server.Picture
import app.melyxar.server.RequestNews
import app.melyxar.server.Said
import app.melyxar.server.SeriesArrived
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class WordingTest {
    /** Says each string by its name and what fills it. */
    private val words = object : Wording {
        override fun said(id: Int, vararg args: Any) = (listOf(name(id)) + args.map(Any::toString)).joinToString("|")

        override fun counted(id: Int, count: Int, vararg args: Any) = (listOf(name(id)) + args.map(Any::toString)).joinToString("|")

        override fun moment(written: String) = "at $written"
    }

    private fun name(id: Int) = when (id) {
        R.string.tv_notes_new_film -> "new_film"
        R.plurals.tv_notes_new_films -> "new_films"
        R.plurals.tv_notes_new_episodes -> "new_episodes"
        R.plurals.tv_notes_new_many -> "new_many"
        R.plurals.tv_notes_and_more -> "and_more"
        R.string.tv_notes_request_added -> "request_added"
        R.string.tv_notes_request_asked -> "request_asked"
        R.string.tv_notes_maintenance_at -> "maintenance_at"
        R.string.tv_season -> "season"
        else -> id.toString()
    }

    private fun note(said: Said, work: String? = null, dueAt: String? = null) =
        Note(id = "01", kind = said.kind, data = said, workId = work, dueAt = dueAt)

    @Test
    fun oneFilmLeadsToItself() {
        val said = words.say(note(Said(kind = "new_content", library = "l", libraryName = "Films", films = 1, filmTitles = listOf("Lantern")), work = "w"))
        assertEquals("new_film|Lantern", said.title)
        assertEquals(Leads.Work("w"), said.leads)
    }

    @Test
    fun manyFilmsAreNamedAFewAndCounted() {
        val titles = listOf("Dawn", "Tide", "Harbour", "Lantern", "Comet")
        val said = words.say(note(Said(kind = "new_content", library = "l", libraryName = "Films", films = 5, filmTitles = titles)))
        assertEquals("new_films|5", said.title)
        assertEquals("and_more|Dawn, Tide, Harbour|2", said.detail)
        assertEquals(Leads.Library("l"), said.leads)
    }

    @Test
    fun episodesOfOneSeriesLeadToIt() {
        val said = words.say(note(Said(kind = "new_content", library = "l", libraryName = "Séries", series = listOf(SeriesArrived("s", "Orbit", 3)))))
        assertEquals("new_episodes|3|Orbit", said.title)
        assertEquals(Leads.Work("s"), said.leads)
    }

    @Test
    fun aMixCountsEveryTitle() {
        val said = words.say(
            note(Said(kind = "new_content", library = "l", libraryName = "Tout", films = 1, filmTitles = listOf("Dawn"), series = listOf(SeriesArrived("s", "Orbit", 2)))),
        )
        assertEquals("new_many|3|Tout", said.title)
        assertEquals("Orbit, Dawn", said.detail)
    }

    @Test
    fun aRequestWearsItsPosterAndLeadsOnlyOnceHere() {
        val asked = words.say(
            note(Said(kind = "request", title = "Comet", year = 2001, poster = "/p.jpg", news = RequestNews("asked", by = "Mira", seasons = listOf(2)))),
        )
        assertEquals("request_asked|Mira|Comet (2001)", asked.title)
        assertEquals("season|2", asked.detail)
        assertNull(asked.leads)
        assertEquals(listOf(Picture("/p.jpg")), asked.poster)
        val added = words.say(note(Said(kind = "request", title = "Comet", news = RequestNews("added")), work = "w"))
        assertEquals(Leads.Work("w"), added.leads)
    }

    @Test
    fun aMaintenanceSaysWhen() {
        val said = words.say(note(Said(kind = "maintenance", title = "Pause", text = "Disks"), dueAt = "2026-01-01T10:00:00Z"))
        assertEquals("maintenance_at|at 2026-01-01T10:00:00Z · Disks", said.detail)
    }
}
