package app.melyxar.tv.play

import androidx.media3.common.text.Cue
import androidx.media3.extractor.text.CuesWithTiming
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WordsTest {
    private fun said(text: String) = Cue.Builder().setText(text).build()

    private fun timed(text: String, from: Long, until: Long) = CuesWithTiming(listOf(said(text)), from, until - from)

    private fun List<Stretch>.textAt(us: Long) = cuesAt(us).map { it.text.toString() }

    @Test
    fun showsEachLineOnlyOverItsOwnTime() {
        val words = stretchesOf(listOf(timed("one", 1_000, 2_000), timed("two", 3_000, 4_000)))
        assertTrue(words.textAt(500).isEmpty())
        assertEquals(listOf("one"), words.textAt(1_000))
        assertTrue(words.textAt(2_500).isEmpty())
        assertEquals(listOf("two"), words.textAt(3_999))
        assertTrue(words.textAt(4_000).isEmpty())
    }

    @Test
    fun showsOverlappingLinesTogether() {
        val words = stretchesOf(listOf(timed("one", 1_000, 3_000), timed("two", 2_000, 4_000)))
        assertEquals(listOf("one"), words.textAt(1_500))
        assertEquals(listOf("one", "two"), words.textAt(2_500))
        assertEquals(listOf("two"), words.textAt(3_500))
    }

    @Test
    fun keepsTheSameLinesWhileTheyStay() {
        val words = stretchesOf(listOf(timed("one", 1_000, 3_000)))
        assertTrue(words.cuesAt(1_200) === words.cuesAt(2_800))
    }
}
