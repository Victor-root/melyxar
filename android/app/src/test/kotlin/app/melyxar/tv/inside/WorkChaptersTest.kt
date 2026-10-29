package app.melyxar.tv.inside

import app.melyxar.server.Chapter
import app.melyxar.server.Plan
import app.melyxar.server.Thumbnails
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertSame
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WorkChaptersTest {
    private val sheets = Thumbnails("/sheets", everySeconds = 10.0, width = 160, height = 90, columns = 10, rows = 10, counted = 720)

    /** Only what the chapters are worked out from. */
    private fun plan(chapters: List<Chapter> = emptyList(), thumbnails: Thumbnails? = null, seconds: Double? = null) =
        Plan(url = "", method = "direct_play", durationSeconds = seconds, thumbnails = thumbnails, chapters = chapters)

    @Test
    fun startsAtTheBeginningAndKeepsToRoundDistances() {
        assertEquals((0..11).map { it * 600.0 }, inventedMoments(2 * 3600.0))
    }

    @Test
    fun widensTheDistanceForALongFilmSoTheRowStaysASizeToRead() {
        val moments = inventedMoments(3 * 3600.0 + 20 * 60)
        assertTrue(moments.size <= 12)
        assertEquals(1200.0, moments[1] - moments[0])
    }

    @Test
    fun makesNoCardForTheLastMinuteOfTheFilm() {
        assertEquals(listOf(0.0, 300.0, 600.0, 900.0, 1200.0), inventedMoments(1500.0))
        assertEquals(listOf(0.0, 300.0, 600.0, 900.0), inventedMoments(1210.0))
    }

    @Test
    fun makesNothingOfAFilmTooShortToMakeARowOrOfNoLength() {
        assertEquals(emptyList<Double>(), inventedMoments(400.0))
        assertEquals(emptyList<Double>(), inventedMoments(0.0))
        assertEquals(emptyList<Double>(), inventedMoments(Double.NaN))
    }

    @Test
    fun keepsTheChaptersTheFileNamesAndInventsNothingBesideThem() {
        val named = listOf(Chapter(0.0, "Opening"))
        assertSame(named, chaptersShownOf(plan(chapters = named, thumbnails = sheets)))
    }

    @Test
    fun inventsThemFromThePicturesWhenTheFileNamesNone() {
        val found = chaptersShownOf(plan(thumbnails = sheets, seconds = 7200.0))
        assertEquals(12, found.size)
        assertTrue(found.all { it.title == null })
        assertEquals(600.0, found[1].atSecond)
    }

    @Test
    fun inventsNoneWithoutPicturesToPutOnThem() {
        assertEquals(emptyList<Chapter>(), chaptersShownOf(plan(seconds = 7200.0)))
    }

    @Test
    fun keepsToTheShorterOfWhatTheFileSaysAndWhatThePicturesCover() {
        // The pictures cover two hours and the file says half an hour.
        val found = chaptersShownOf(plan(thumbnails = sheets, seconds = 1800.0))
        assertTrue(found.last().atSecond < 1800)
    }
}
