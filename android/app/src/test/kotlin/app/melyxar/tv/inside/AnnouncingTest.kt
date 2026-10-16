package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class AnnouncingTest {
    @Test
    fun aNameThatFitsIsSaidTheUsualTime() {
        assertEquals(SAID_WHOLE_MS, saidForMs(300, 400, 60f))
        assertEquals(SAID_WHOLE_MS, saidForMs(400, 400, 60f))
    }

    @Test
    fun aLongNameIsSaidUntilItHasRunBy() {
        // 600 wide in 300 of glass, at 60 a second: 600 + 100 to run, so
        // eleven and two thirds seconds, after the wait and before the rest.
        assertEquals(1_200L + 11_666L + 800L, saidForMs(600, 300, 60f))
    }
}
