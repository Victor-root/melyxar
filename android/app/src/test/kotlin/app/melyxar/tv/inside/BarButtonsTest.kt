package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class BarButtonsTest {
    @Test
    fun theAccountsOrderIsKeptAndTheRestFollows() {
        val buttons = barButtons(listOf("watch_later", "search"), inTheBar = listOf("search", "watch_later"))
        assertEquals(listOf("watch_later", "search"), buttons.bar)
        assertEquals(listOf("favourites"), buttons.menu)
    }

    @Test
    fun unknownDoubledAndUselessButtonsAreLeftOut() {
        val buttons = barButtons(listOf("cast", "search", "nonsense", "search"), inTheBar = listOf("cast", "search"))
        assertEquals(listOf("search"), buttons.bar)
        assertEquals(listOf("favourites", "watch_later"), buttons.menu)
    }

    @Test
    fun theClockWaitsForTheTurnOfTheMinute() {
        assertEquals(60_000, untilTheNextMinute(120_000))
        assertEquals(1, untilTheNextMinute(179_999))
    }
}
