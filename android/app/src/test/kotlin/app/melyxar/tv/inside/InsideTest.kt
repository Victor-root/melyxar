package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class InsideTest {
    private val first = Play("episode-1", "file-1", null)
    private val second = Play("episode-2", "file-2", null)

    @Test
    fun theEpisodesOwnPageFollowsTheSitting() {
        assertEquals(Place.Opened("episode-2"), pageUnderTheNext(Place.Opened("episode-1"), first, second))
    }

    @Test
    fun theSeriesPageItWasStartedFromStays() {
        assertNull(pageUnderTheNext(Place.Opened("series"), first, second))
    }

    @Test
    fun anythingElseStays() {
        assertNull(pageUnderTheNext(Place.Home, first, second))
        assertNull(pageUnderTheNext(null, first, second))
    }
}
