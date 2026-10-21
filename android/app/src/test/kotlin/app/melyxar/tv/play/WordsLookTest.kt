package app.melyxar.tv.play

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class WordsLookTest {
    @Test
    fun sitsAtTheChosenShareOfTheScreen() {
        // At the bottom, on a 1000 high screen, words 100 high: their foot at
        // 920, let down by 8 of their own height.
        assertEquals(828f, wordsTop(1000f, 100f, 0.08f, lift = null), 0.01f)
    }

    @Test
    fun staysWhollyOnTheScreenAtEitherEnd() {
        assertEquals(900f, wordsTop(1000f, 100f, 0f, lift = null), 0.01f)
        assertEquals(0f, wordsTop(1000f, 100f, 1f, lift = null), 0.01f)
    }

    @Test
    fun climbsAboveTheControlsWhileTheyAreUp() {
        // Controls 300 high with the gap: the foot goes up to 300.
        assertEquals(608f, wordsTop(1000f, 100f, 0.08f, lift = 300f), 0.01f)
    }

    @Test
    fun aHeightAboveTheControlsStaysWhereItWasChosen() {
        assertEquals(wordsTop(1000f, 100f, 0.85f, lift = null), wordsTop(1000f, 100f, 0.85f, lift = 300f), 0.01f)
    }

    @Test
    fun aCustomHeightIsItsOwnShare() {
        assertEquals(0.35f, WordsLook(height = WordsHeight.Custom, heightCustom = 35).share, 0.0001f)
        assertEquals(0.08f, WordsLook(heightCustom = 35).share, 0.0001f)
    }
}
