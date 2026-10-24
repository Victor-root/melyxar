package app.melyxar.tv.play

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class AdaptiveTest {
    private fun rungAt(bitrate: Long?) = ADAPTIVE_RUNGS.indexOfFirst { it.bitrate == bitrate }
    private val sixMegabits = rungAt(6_000_000)

    /** Heavier than every rung, so that every move changes the film. */
    private val heavy = Film(60_000_000, 2160)
    private val light = Film(8_000_000, 720)

    private fun measured(rung: Int, at: Long) = Course(rung, at, measured = 5)

    @Test
    fun takesTheBestRungThatFitsInPartOfTheSpeed() {
        assertEquals(2_000_000L, ADAPTIVE_RUNGS[rungFor(5_000_000.0)].bitrate)
        assertEquals(6_000_000L, ADAPTIVE_RUNGS[rungFor(10_000_000.0)].bitrate)
        assertEquals(40_000_000L, ADAPTIVE_RUNGS[rungFor(60_000_000.0)].bitrate)
        assertEquals(ADAPTIVE_RUNGS.lastIndex, rungFor(200_000_000.0))
        assertEquals(0, rungFor(100_000.0))
        assertEquals(4_000_000L, ADAPTIVE_RUNGS[UNMEASURED_RUNG].bitrate)
    }

    @Test
    fun believesOneSegmentThatCrawledInAndLittleOfOneThatFlew() {
        val fell = Speed(200_000_000.0, 200_000_000.0).withMeasure(3_000_000.0, 8.0)
        assertTrue(fell.believed < 20_000_000)
        val flew = Speed(5_000_000.0, 5_000_000.0).withMeasure(500_000_000.0, 0.01)
        assertTrue(flew.believed < 6_000_000)
    }

    @Test
    fun aRungLetsTheFilmThroughWhenItIsNoHeavierAndNoTaller() {
        assertTrue(letsThrough(light, rungAt(10_000_000)))
        assertFalse(letsThrough(light, sixMegabits))
        assertFalse(letsThrough(Film(8_000_000, 2160), rungAt(10_000_000)))
        assertTrue(letsThrough(null, ADAPTIVE_RUNGS.lastIndex))
        assertFalse(letsThrough(null, rungAt(40_000_000)))
    }

    @Test
    fun comesDownWhenTheConnectionFallsShortAndLittleIsHeld() {
        assertEquals(Move(rungAt(2_000_000), Because.SlowerLink), nextMove(measured(sixMegabits, 0), 4_000_000.0, 5.0, 1_000, heavy))
        assertNull(nextMove(measured(sixMegabits, 0), 4_000_000.0, 40.0, 1_000, heavy))
        assertNull(nextMove(Course(sixMegabits, 0), 1_000_000.0, 0.0, 1_000, heavy))
    }

    @Test
    fun comesDownARungAfterTwoStallsInAMinute() {
        assertEquals(
            Move(sixMegabits - 1, Because.Stalls),
            nextMove(Course(sixMegabits, 0, stalls = listOf(10_000, 40_000)), null, 0.0, 45_000, heavy),
        )
        assertNull(nextMove(Course(sixMegabits, 0, stalls = listOf(10_000, 100_000)), null, 0.0, 105_000, heavy))
        assertNull(nextMove(Course(0, 0, stalls = listOf(1_000, 2_000)), 100_000.0, 0.0, 3_000, heavy))
    }

    @Test
    fun goesUpOnlyOnceTheRungWasKeptAWhileAndEnoughIsHeld() {
        val course = measured(rungAt(2_000_000), 0)
        assertNull(nextMove(course, 30_000_000.0, 20.0, 10_000, heavy))
        assertNull(nextMove(course, 30_000_000.0, 5.0, 40_000, heavy))
        assertEquals(Move(rungAt(20_000_000), Because.FasterLink), nextMove(course, 30_000_000.0, 20.0, 40_000, heavy))
        val lowered = measured(sixMegabits, 0).afterMoving(Move(rungAt(2_000_000), Because.SlowerLink), 0).copy(measured = 5)
        assertNull(nextMove(lowered, 30_000_000.0, 20.0, 60_000, heavy))
        assertEquals(Because.FasterLink, nextMove(lowered, 30_000_000.0, 20.0, 130_000, heavy)?.because)
    }

    @Test
    fun neverOpensTheFilmAgainForARungThatGivesTheSameFilm() {
        val asItIs = measured(ADAPTIVE_RUNGS.lastIndex, 0)
        assertNull(nextMove(asItIs, 60_000_000.0, 5.0, 1_000, light))
        assertEquals(Move(sixMegabits, Because.SlowerLink), nextMove(asItIs, 10_000_000.0, 5.0, 1_000, light))
        assertEquals(
            Move(sixMegabits, Because.Stalls),
            nextMove(Course(ADAPTIVE_RUNGS.lastIndex, 0, stalls = listOf(1_000, 2_000)), null, 0.0, 3_000, light),
        )
    }
}
