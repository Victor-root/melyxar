package app.melyxar.tv.play

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class SoundHoleTest {
    @Test
    fun isStuckOnceItHasStoodStillLongEnoughWithFilmHeldAhead() {
        val first = watchStill(null, 299_900, 0, 44_000)
        assertFalse(first.stuck)
        assertFalse(watchStill(first.watch, 299_900, STILL_FOR_MS - 1, 44_000).stuck)
        assertTrue(watchStill(first.watch, 299_900, STILL_FOR_MS, 44_000).stuck)
    }

    @Test
    fun isOnlyWaitingForTheNetworkWhenNothingIsHeldAhead() {
        val first = watchStill(null, 12_000, 0, 200)
        assertFalse(watchStill(first.watch, 12_000, STILL_FOR_MS * 3, 200).stuck)
    }

    @Test
    fun startsCountingAgainAsSoonAsItMoves() {
        val first = watchStill(null, 12_000, 0, 30_000)
        val moved = watchStill(first.watch, 12_500, STILL_FOR_MS, 30_000)
        assertFalse(moved.stuck)
        assertEquals(StillWatch(12_500, STILL_FOR_MS), moved.watch)
    }
}
