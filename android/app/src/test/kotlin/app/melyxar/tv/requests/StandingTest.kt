package app.melyxar.tv.requests

import app.melyxar.server.Found
import app.melyxar.server.TitleHeld
import app.melyxar.server.TitleRequest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class StandingTest {
    private val film = Found(catalogue = "films", tmdbId = "11", title = "The Lantern Keeper")
    private val series = Found(catalogue = "series", tmdbId = "22", title = "Harbour Lights")

    private fun request(found: Found, state: String) =
        TitleRequest(id = "r-${found.tmdbId}-$state", catalogue = found.catalogue, tmdbId = found.tmdbId, title = found.title, state = state)

    @Test
    fun aFilmHereIsOpenedRatherThanAskedFor() {
        assertEquals(Standing.Here("w1"), standingOf(film.copy(held = TitleHeld("w1")), emptyList()))
    }

    @Test
    fun aSeriesHeldInPartMayStillBeAskedForItsOtherSeasons() {
        assertEquals(Standing.Free(0, listOf(1, 2)), standingOf(series.copy(held = TitleHeld("w2", listOf(1, 2))), emptyList()))
    }

    @Test
    fun aRequestOfThisAccountThatWaitsComesFirstAndIsNotCountedAmongTheOthers() {
        val mine = request(film, "accepted")
        assertEquals(Standing.Mine(mine, 2), standingOf(film.copy(askedBy = 3, mine = mine.id, held = TitleHeld("w1")), listOf(mine)))
    }

    @Test
    fun aRequestDecidedNoLongerStandsInTheWay() {
        assertEquals(Standing.Free(1, emptyList()), standingOf(film.copy(askedBy = 1), listOf(request(film, "refused"))))
    }
}
