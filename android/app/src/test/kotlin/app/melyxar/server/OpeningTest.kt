package app.melyxar.server

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class OpeningTest {
    @Test
    fun eachOpeningNamesItsProfile() {
        assertNull(Opening.Ask.who("Mira"))
        assertEquals("Mira", Opening.Last.who("Mira"))
        assertNull(Opening.Last.who(null))
        assertEquals("Odo", Opening.Always("Odo").who("Mira"))
    }

    @Test
    fun anOpeningIsReadBackAsItWasWritten() {
        listOf(Opening.Ask, Opening.Last, Opening.Always("Odo"), Opening.Always("a:b")).forEach {
            assertEquals(it, openingWritten(it.written()))
        }
        assertEquals(Opening.Ask, openingWritten(null))
        assertEquals(Opening.Ask, openingWritten("nonsense"))
    }
}
