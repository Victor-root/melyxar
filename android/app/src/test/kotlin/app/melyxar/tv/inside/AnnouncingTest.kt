package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class AnnouncingTest {
    @Test
    fun aNameThatFitsIsSaidTheUsualTime() {
        assertEquals(SAID_WHOLE_MS, saidForMs(0, 40f))
        assertEquals(SAID_WHOLE_MS, saidForMs(-20, 40f))
    }

    @Test
    fun aLongNameIsSaidUntilItsEndHasBeenRead() {
        // 300 pixels past the glass at 40 a second: seven and a half seconds
        // to run, after the wait and before its end is read.
        assertEquals(7_500L, runForMs(300, 40f))
        assertEquals(1_200L + 7_500L + 1_500L, saidForMs(300, 40f))
    }
}
