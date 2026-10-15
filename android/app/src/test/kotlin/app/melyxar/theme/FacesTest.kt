package app.melyxar.theme

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class FacesTest {
    private fun hex(colour: Int) = "#%06x".format(colour and 0xFFFFFF)

    @Test
    fun picksThePaintingTheWebPicks() {
        // faceArtOf from web/src/faces.ts, run by node on the same names.
        assertEquals(FaceArt(FaceDrawing.Nebula, 40), faceArtOf("Victor"))
        assertEquals(FaceArt(FaceDrawing.Eclipse, 285), faceArtOf("Élodie"))
        assertEquals(FaceArt(FaceDrawing.Orbits, 0), faceArtOf("Zoë"))
        assertEquals(FaceArt(FaceDrawing.Nebula, 205), faceArtOf("Marie-Hélène"))
        assertEquals(FaceArt(FaceDrawing.Horizon, 0), faceArtOf("Bob"))
        assertEquals(FaceArt(FaceDrawing.Record, 285), faceArtOf(""))
    }

    @Test
    fun hashesAnEmojiAsOneCodePoint() {
        assertEquals(FaceArt(FaceDrawing.Rings, 285), faceArtOf("🎬 Cinéma"))
    }

    @Test
    fun readsColoursInOklchAsCssColour4Does() {
        fun assertOklch(expected: Oklch, colour: String) {
            val actual = oklchOf(parseHex(colour)!!)
            assertEquals(expected.lightness, actual.lightness, 1e-5)
            assertEquals(expected.chroma, actual.chroma, 1e-5)
            assertEquals(expected.hue, actual.hue, 1e-3)
        }
        assertOklch(Oklch(0.627955, 0.257683, 29.233880), "#ff0000")
        assertOklch(Oklch(0.534915, 0.202633, 27.609718), "#c81e1e")
        assertOklch(Oklch(0.607131, 0.197908, 259.581572), "#2f7cf6")
        assertOklch(Oklch(0.636707, 0.126946, 166.529636), "#16a37a")
    }

    @Test
    fun givesAGreyNoHue() {
        assertEquals(0.0, oklchOf(parseHex("#808080")!!).hue)
        assertEquals(0.0, oklchOf(0xFFFFFFFF.toInt()).hue)
    }

    @Test
    fun clampsWhatAnSrgbScreenCannotShow() {
        assertEquals("#c65b4d", hex(fromOklch(Oklch(0.6, 0.14, 29.0))))
        assertEquals("#00c1cc", hex(fromOklch(Oklch(0.72, 0.16, 200.0))))
        assertEquals("#eee0ff", hex(fromOklch(Oklch(0.93, 0.06, 300.0))))
    }

    @Test
    fun paintsInTheTintsTheStylesheetWorksOut() {
        fun tints(accent: String, turn: Int) = faceTints(parseHex(accent)!!, turn).let {
            listOf(it.night, it.deep, it.base, it.glow, it.light).map(::hex)
        }
        assertEquals(listOf("#160605", "#760010", "#df3b34", "#ff7005", "#ffd3bc"), tints("#c81e1e", 0))
        assertEquals(listOf("#000f10", "#004d4a", "#00a0ab", "#00c0f3", "#9cfaff"), tints("#c81e1e", 170))
        assertEquals(listOf("#000f0d", "#004e36", "#00a28f", "#00c5da", "#9efbfa"), tints("#2f7cf6", 285))
        assertEquals(listOf("#160703", "#5e2218", "#c45e39", "#ed8725", "#ffddc5"), tints("#808080", 40))
    }
}
