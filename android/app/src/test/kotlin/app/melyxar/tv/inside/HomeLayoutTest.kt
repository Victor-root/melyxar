package app.melyxar.tv.inside

import androidx.compose.ui.unit.dp
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class HomeLayoutTest {
    @Test
    fun pairsTheTwoRowsOfWhatIsUnderWayWhenTheyFollowEachOther() {
        assertEquals(
            listOf(Laid.One("band"), Laid.Two("carry_on", "up_next"), Laid.One("newest:films-library"), Laid.One("recently_added")),
            laidOut(listOf("band", "carry_on", "up_next", "newest:films-library", "recently_added")),
        )
        assertEquals(listOf(Laid.Two("up_next", "carry_on")), laidOut(listOf("up_next", "carry_on")))
    }

    @Test
    fun laysThemOutApartWhenAnotherSectionStandsBetweenThem() {
        assertEquals(listOf(Laid.One("carry_on"), Laid.One("band"), Laid.One("up_next")), laidOut(listOf("carry_on", "band", "up_next")))
        assertEquals(emptyList<Laid>(), laidOut(emptyList()))
    }

    @Test
    fun twoRowsShareALineOnlyWhileBothHoldOnIt() {
        assertEquals(288.dp, rowWidth(1, 240.dp, 10.dp, 24.dp))
        assertEquals(538.dp, rowWidth(2, 240.dp, 10.dp, 24.dp))
        assertTrue(shareALine(2, 1, 240.dp, 10.dp, 24.dp, 960.dp))
        assertFalse(shareALine(2, 2, 240.dp, 10.dp, 24.dp, 960.dp))
        assertFalse(shareALine(2, 0, 240.dp, 10.dp, 24.dp, 960.dp))
    }
}
