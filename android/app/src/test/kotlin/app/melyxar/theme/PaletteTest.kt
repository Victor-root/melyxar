package app.melyxar.theme

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class PaletteTest {
    @Test
    fun followsTheAccountThenTheServerThenTheDevice() {
        assertTrue(wantsLight("light", "dark", deviceIsDark = true))
        assertFalse(wantsLight("dark", "light", deviceIsDark = false))
        assertTrue(wantsLight("server", "light", deviceIsDark = true))
        assertTrue(wantsLight("server", "system", deviceIsDark = false))
        assertFalse(wantsLight("system", "light", deviceIsDark = true))
    }
}
