package app.melyxar.theme

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ShadesTest {
    private fun hex(colour: Int) = "#%06x".format(colour and 0xFFFFFF)

    @Test
    fun keepsTheMeasuredShadesOfTheUsualRed() {
        val shades = accentShades(USUAL_ACCENT, light = false)
        assertEquals("#e63535", hex(shades.bright))
        assertEquals("#9b1616", hex(shades.dim))
        assertEquals("#e50000", hex(shades.mark))
        assertEquals(0xFFFFFFFF.toInt(), shades.contrast)
        assertEquals(0x59, shades.glow ushr 24)
    }

    @Test
    fun mixesTheShadesOfAnyOtherAccentAsTheWebDoes() {
        val blue = parseHex("#2f7cf6")!!
        val shades = accentShades(blue, light = true)
        // color-mix(in srgb, #2f7cf6 82%, white) and (78%, black).
        assertEquals("#5494f8", hex(shades.bright))
        assertEquals("#2561c0", hex(shades.dim))
        assertEquals(0x38, shades.glow ushr 24)
    }

    @Test
    fun writesDarkOnAPaleAccentAndWhiteOnADeepOne() {
        assertEquals(0xFF14161B.toInt(), readableOn(parseHex("#f5d142")!!))
        assertEquals(0xFFFFFFFF.toInt(), readableOn(parseHex("#16a37a")!!))
    }

    @Test
    fun drawsTheLogoInAVividColourOfTheAccentsHue() {
        // mark.ts gives #e50000 for the usual red, whose hue is 0.
        assertEquals("#e50000", hex(vividOf(USUAL_ACCENT)))
        assertEquals("#0059e6", hex(vividOf(parseHex("#2f7cf6")!!)))
    }

    @Test
    fun readsOnlyColoursWrittenInSixDigits() {
        assertEquals(0xFFC81E1E.toInt(), parseHex("#c81e1e"))
        assertNull(parseHex("#c81e1"))
        assertNull(parseHex("red"))
    }

    @Test
    fun laysTheReliefOverTheColourInHardLight() {
        val red = 0xFFE50000.toInt()
        // Black darkens to black, white lightens to white, the middle keeps
        // the colour, and where the relief does not cover nothing is drawn.
        assertEquals(0xFF000000.toInt(), markPixel(0xFF000000.toInt(), red))
        assertEquals(0xFFFFFFFF.toInt(), markPixel(0xFFFFFFFF.toInt(), red))
        // 0x80 is a hair above the middle, which lifts what is black.
        assertEquals("#e50101", hex(markPixel(0xFF808080.toInt(), red)))
        assertEquals(0, markPixel(0x00808080, red) ushr 24)
        // Half covered, half the shade.
        assertEquals(0x80, markPixel(0x80000000.toInt(), red) ushr 24)
        assertEquals("#720000", hex(markPixel(0x80000000.toInt(), red)))
    }
}
