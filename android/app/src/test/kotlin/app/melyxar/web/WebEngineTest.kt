package app.melyxar.web

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WebEngineTest {
    @Test
    fun readsTheMajorVersionFromTheEnginesName() {
        assertEquals(87, WebEngine.majorOf("87.0.4280.101"))
        assertEquals(142, WebEngine.majorOf("142.0.7444.48"))
        assertNull(WebEngine.majorOf("unknown"))
    }

    @Test
    fun onlyAnEngineOlderThanTheOldestDrawingTheInterfaceIsTooOld() {
        assertTrue(WebEngine.isTooOld(87))
        assertTrue(WebEngine.isTooOld(WebEngine.OLDEST_DRAWING_IT - 1))
        assertFalse(WebEngine.isTooOld(WebEngine.OLDEST_DRAWING_IT))
        assertFalse(WebEngine.isTooOld(null))
    }
}
