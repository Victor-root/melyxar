package app.melyxar.web

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class BarShadesTest {
    private fun all(colour: Int) = IntArray(96) { colour or 0xff000000.toInt() }

    @Test
    fun drawsLightIconsOverWhatIsDark() {
        assertFalse(wantsDarkIcons(all(0x0c0d10)))
        // A deep red, the colour of the banner that asked for this.
        assertFalse(wantsDarkIcons(all(0x7a1010)))
        assertFalse(wantsDarkIcons(all(0x555555)))
    }

    @Test
    fun drawsDarkIconsOverWhatIsLight() {
        assertTrue(wantsDarkIcons(all(0xf4f5f7)))
        assertTrue(wantsDarkIcons(all(0xf0c040)))
        assertTrue(wantsDarkIcons(all(0x909090)))
    }

    @Test
    fun weighsEveryPointTogether() {
        val half = IntArray(96) { if (it % 2 == 0) 0xff000000.toInt() else 0xffffffff.toInt() }
        // Half white is light enough, as the eye reads it, for dark icons.
        assertTrue(wantsDarkIcons(half))
        val mostlyDark = IntArray(96) { if (it % 8 == 0) 0xffffffff.toInt() else 0xff101010.toInt() }
        assertFalse(wantsDarkIcons(mostlyDark))
    }

    @Test
    fun keepsLightIconsWithNothingToLookAt() {
        assertFalse(wantsDarkIcons(IntArray(0)))
    }
}
