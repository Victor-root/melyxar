package app.melyxar.tv.play

import app.melyxar.server.Chapter
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ChaptersTest {
    private val named = listOf(Chapter(0.0, "Opening"), Chapter(600.0, "Harbour"), Chapter(1500.0, "Storm"))

    @Test
    fun keepsTheChaptersAFileNames() {
        assertEquals(named, chaptersOf(named, 7200.0))
    }

    @Test
    fun marksEveryFewMinutesAFilmThatNamesNone() {
        assertEquals(listOf(0.0, 300.0, 600.0, 900.0), chaptersOf(emptyList(), 1000.0).map { it.atSecond })
        assertTrue(chaptersOf(emptyList(), 250.0).isEmpty())
        assertTrue(chaptersOf(emptyList(), null).isEmpty())
    }

    @Test
    fun findsTheChapterAMomentIsIn() {
        assertEquals(0, chapterAt(named, 10.0))
        assertEquals(1, chapterAt(named, 599.9))
        assertEquals(2, chapterAt(named, 9000.0))
        assertEquals(-1, chapterAt(listOf(Chapter(30.0)), 10.0))
    }
}
