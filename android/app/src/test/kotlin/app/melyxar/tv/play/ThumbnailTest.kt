package app.melyxar.tv.play

import androidx.compose.ui.unit.dp
import app.melyxar.server.Thumbnails
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ThumbnailTest {
    private val sheets = Thumbnails(
        url = "/thumbs",
        everySeconds = 10.0,
        width = 320,
        height = 180,
        columns = 10,
        rows = 10,
        counted = 250,
    )

    @Test
    fun findsAMomentOnItsSheet() {
        assertEquals(Spot(sheet = 0, column = 0, row = 0), sheets.spotAt(0.0))
        assertEquals(Spot(sheet = 0, column = 3, row = 1), sheets.spotAt(135.0))
        assertEquals(Spot(sheet = 1, column = 0, row = 0), sheets.spotAt(1000.0))
    }

    @Test
    fun neverGoesPastTheLastOne() {
        assertEquals(Spot(sheet = 2, column = 9, row = 4), sheets.spotAt(99_999.0))
        assertEquals(Spot(sheet = 0, column = 0, row = 0), sheets.spotAt(-5.0))
    }

    @Test
    fun hasNoneForAFilmNotRead() {
        assertNull(sheets.copy(counted = 0).spotAt(10.0))
        assertNull(sheets.copy(everySeconds = 0.0).spotAt(10.0))
    }

    @Test
    fun countsTheSheetsTheLastOneHalfFilled() {
        assertEquals(3, sheets.sheetsCounted())
        assertEquals(2, sheets.copy(counted = 200).sheetsCounted())
        assertEquals(0, sheets.copy(columns = 0).sheetsCounted())
    }

    @Test
    fun keepsTheFilmsShape() {
        assertEquals(135.dp, sheets.heightAt(240.dp))
    }

    @Test
    fun goesFurtherTheLongerTheArrowIsHeld() {
        assertEquals(10, scrubStep(0))
        assertEquals(30, scrubStep(10))
        assertEquals(60, scrubStep(40))
    }
}
