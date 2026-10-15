package app.melyxar.music

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class RampTest {
    @Test
    fun goesThereAtOnceWithoutFrames() {
        val ramp = Ramp()
        ramp.aim(0.5f, 0)
        assertEquals(0.5f, ramp.next())
        assertEquals(0.5f, ramp.next())
    }

    @Test
    fun movesInAStraightLineAndStopsOnTheGoal() {
        val ramp = Ramp(0f)
        ramp.aim(1f, 4)
        assertEquals(listOf(0.25f, 0.5f, 0.75f, 1f, 1f), List(5) { ramp.next() })
    }

    @Test
    fun turnsFromWhereverItHasGot() {
        val ramp = Ramp(1f)
        ramp.aim(0f, 4)
        ramp.next()
        ramp.next()
        ramp.aim(2f, 3)
        assertEquals(listOf(1f, 1.5f, 2f), List(3) { ramp.next() })
    }

    @Test
    fun saysWhenThereIsNothingToDo() {
        val ramp = Ramp()
        assertTrue(ramp.untouched)
        ramp.aim(0.8f, 2)
        assertFalse(ramp.untouched)
        ramp.aim(1f, 0)
        assertTrue(ramp.untouched)
    }
}
