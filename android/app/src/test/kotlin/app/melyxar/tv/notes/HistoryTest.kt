package app.melyxar.tv.notes

import app.melyxar.server.Note
import app.melyxar.server.NoteSettings
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class HistoryTest {
    private fun note(id: String, read: Boolean = false, urgent: Boolean = false) =
        Note(id = id, kind = "message", read = read, priority = urgent)

    @Test
    fun anArrivalGoesFirstAndCountsOnceUnread() {
        val held = Held(listOf(note("02"), note("01", read = true)), unread = 1).arrived(note("03"))
        assertEquals(listOf("03", "02", "01"), held.notes.map { it.id })
        assertEquals(2, held.unread)
        assertEquals(2, held.arrived(note("03")).unread)
    }

    @Test
    fun anArrivalOlderThanWhatIsHeldWaitsForItsPage() {
        val held = Held(listOf(note("05")), unread = 1, more = true).arrived(note("02"))
        assertEquals(listOf("05"), held.notes.map { it.id })
        assertEquals(2, held.unread)
    }

    @Test
    fun markingCountsOnlyWhatChanges() {
        val held = Held(listOf(note("02"), note("01", read = true)), unread = 4)
        val read = held.marked(listOf("02", "01", "00"), read = true)
        assertEquals(2, read.unread)
        assertTrue(read.notes.all { it.read })
        assertEquals(listOf("02"), held.unreadAmong(null))
    }

    @Test
    fun anOlderPageGoesAfterWithoutDoubles() {
        val held = Held(listOf(note("03"), note("02")), more = true).olderPage(listOf(note("02"), note("01")), more = false)
        assertEquals(listOf("03", "02", "01"), held.notes.map { it.id })
        assertFalse(held.more)
    }

    @Test
    fun quietHoursMayRunAcrossMidnight() {
        val night = NoteSettings(quietFrom = 22 * 60, quietUntil = 7 * 60)
        assertTrue(isQuiet(23 * 60, night))
        assertTrue(isQuiet(6 * 60, night))
        assertFalse(isQuiet(12 * 60, night))
        assertFalse(isQuiet(12 * 60, NoteSettings()))
    }

    @Test
    fun onlyWhatIsUrgentShowsOverAFilm() {
        val settings = NoteSettings()
        assertFalse(showsNow(note("01"), settings, filmOnScreen = true, minute = 0))
        assertTrue(showsNow(note("01", urgent = true), settings, filmOnScreen = true, minute = 0))
        assertFalse(showsNow(note("01", urgent = true), settings.copy(priorityWhilePlaying = false), filmOnScreen = true, minute = 0))
        assertTrue(showsNow(note("01"), settings, filmOnScreen = false, minute = 0))
    }
}
