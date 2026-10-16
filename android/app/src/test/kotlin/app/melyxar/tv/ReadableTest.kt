package app.melyxar.tv

import app.melyxar.server.Card
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test
import java.util.Locale

class ReadableTest {
    private fun card(length: Long? = null, runtime: Int? = null) =
        Card(id = "w", title = "Comètes", kind = "movie", resumeLengthSeconds = length, runtimeMinutes = runtime)

    @Test
    fun measuresAPlayByItsFileBeforeTheCatalogue() {
        assertEquals(5400L, lengthOfAPlay(card(length = 5400, runtime = 120)))
        assertEquals(7200L, lengthOfAPlay(card(runtime = 120)))
        assertNull(lengthOfAPlay(card(length = 0, runtime = 0)))
    }

    @Test
    fun saysHowMuchIsLeftAndHowFarIn() {
        assertEquals(86, minutesLeft(2892.0, 8040))
        assertNull(minutesLeft(8030.0, 8040))
        assertNull(minutesLeft(10.0, null))
        assertEquals(0.5f, howFarIn(60.0, 120))
        assertEquals(1f, howFarIn(500.0, 120))
        assertEquals(Duration(2, 14), durationOf(134))
    }

    @Test
    fun namesAPictureByItsLargerSide() {
        assertEquals("4K", pictureName(3840, 1600))
        assertEquals("1080p", pictureName(1920, 800))
        assertEquals("720p", pictureName(1280, 534))
        assertNull(pictureName(720, 576))
        assertNull(pictureName(null, null))
    }

    @Test
    fun namesWhatAFileHoldsAsItsMakersDo() {
        assertEquals(listOf("4K", "Dolby Vision", "Dolby Atmos"), fileBadges(3840, 2160, "dolby_vision", "TrueHD Atmos"))
        assertEquals(listOf("1080p", "Dolby Digital+"), fileBadges(1920, 1080, null, "eac3"))
        assertEquals(listOf("PCM"), fileBadges(null, null, null, "pcm_s24le"))
    }

    @Test
    fun namesTheSpeakersOfASurroundTrackOnly() {
        assertEquals("5.1", speakersName(6))
        assertEquals("7.1", speakersName(8))
        assertNull(speakersName(2))
        assertNull(speakersName(null))
    }

    @Test
    fun showsAMomentAsAPlayersClockDoes() {
        assertEquals("1:02:33", clock(3753.4))
        assertEquals("4:05", clock(245.0))
        assertEquals("0:00", clock(-3.0))
    }

    @Test
    fun namesALanguageInTheScreensOwn() {
        assertEquals("Français", languageName("fre", Locale.FRENCH))
        assertEquals("French", languageName("fra", Locale.ENGLISH))
        assertEquals("Anglais", languageName("eng", Locale.FRENCH))
        assertNull(languageName("und", Locale.FRENCH))
        assertNull(languageName(null, Locale.FRENCH))
    }

    @Test
    fun readsTheInstantsTheServerWrites() {
        assertEquals(86_400_000L, instantOf("1970-01-02T00:00:00Z"))
        assertEquals(1_000L, instantOf("1970-01-01T00:00:01.25Z"))
        assertNull(instantOf("1970-01-02"))
    }

    @Test
    fun countsTheYearsUpToTheBirthday() {
        assertEquals(40, yearsBetween("1985-03-12", "2025-03-12"))
        assertEquals(39, yearsBetween("1985-03-12", "2025-03-11"))
        assertEquals(39, yearsBetween("1985-03-12", "2025-02-28"))
        assertNull(yearsBetween("1985", "2025-03-12"))
    }

    @Test
    fun writesADayWithItsMonthInWords() {
        assertEquals("12 mars 1985", readableDay("1985-03-12", Locale.FRENCH))
        assertEquals("March 12, 1985", readableDay("1985-03-12", Locale.US))
        assertNull(readableDay("1985-3-12", Locale.FRENCH))
    }

    @Test
    fun writesTodayAsTheServerWritesADay() {
        val day = java.util.Calendar.getInstance().apply {
            clear()
            set(2026, 0, 5)
        }
        assertEquals("2026-01-05", todayOf(day))
    }

    @Test
    fun writesAnOffsetAsTheWebDoes() {
        assertEquals("+1.5 s", offsetName(1.5f))
        assertEquals("0.0 s", offsetName(0f))
        assertEquals("-2.0 s", offsetName(-2f))
    }

    @Test
    fun writesASpeedAsTheWebDoes() {
        assertEquals("1×", speedName(1f))
        assertEquals("0.75×", speedName(0.75f))
        assertEquals("2×", speedName(2f))
    }
}
