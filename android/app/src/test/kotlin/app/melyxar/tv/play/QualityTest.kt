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
    fun fallsBackToTheFilmAsItIs() {
        assertEquals(AS_IT_IS, qualityCalled(null))
        assertEquals(AS_IT_IS, qualityCalled("4k-100"))
    }
}
