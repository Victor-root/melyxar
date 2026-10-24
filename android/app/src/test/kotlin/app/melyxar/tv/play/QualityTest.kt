package app.melyxar.tv.play

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class QualityTest {
    @Test
    fun namesARungByItsNumbers() {
        assertEquals("Original", AS_IT_IS.name("Original"))
        assertEquals("1080p · 20 Mb/s", qualityCalled("1080p-20").name("Original"))
        assertEquals("480p · 1.5 Mb/s", qualityCalled("480p-1.5").name("Original"))
        assertEquals("360p · 0.7 Mb/s", qualityCalled("360p-0.7").name("Original"))
    }

    @Test
    fun saysARungOfTheAutomaticQualityInAWordOrTwo() {
        assertEquals("40 Mb/s", ADAPTIVE_RUNGS.first { it.bitrate == 40_000_000L }.name("Original"))
        assertEquals("720p", ADAPTIVE_RUNGS.first { it.bitrate == 2_000_000L }.shortName("Origine"))
        assertEquals("40 Mb/s", ADAPTIVE_RUNGS.first { it.bitrate == 40_000_000L }.shortName("Origine"))
        assertEquals("Origine", ADAPTIVE_RUNGS.last().shortName("Origine"))
    }

    @Test
    fun fallsBackToTheFilmAsItIs() {
        assertEquals(AS_IT_IS, qualityCalled(null))
        assertEquals(AS_IT_IS, qualityCalled("4k-100"))
    }
}
