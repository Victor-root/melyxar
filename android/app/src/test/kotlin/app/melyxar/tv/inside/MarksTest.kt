package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class MarksTest {
    private val started = Whereabouts("in_progress", 1200, 0)

    @Test
    fun watchedLetsGoOfWhereItWasLeft() {
        assertEquals(Whereabouts("watched", null, 0), markedWatched(true, started, episodes = 0))
    }

    @Test
    fun unwatchedKeepsWhereItWasStopped() {
        assertEquals(started, markedWatched(false, started, episodes = 0))
        assertEquals(Whereabouts("not_started", null, 8), markedWatched(false, Whereabouts("watched", null, 0), episodes = 8))
    }

    @Test
    fun whatWasSaidHoldsUntilTheServerSaysOtherwise() {
        val said = Said(Whereabouts("watched", null, 0), started)
        assertEquals(said.said, whereaboutsOf(started, said))
        val playedSince = Whereabouts("in_progress", 2400, 0)
        assertEquals(playedSince, whereaboutsOf(playedSince, said))
    }
}
