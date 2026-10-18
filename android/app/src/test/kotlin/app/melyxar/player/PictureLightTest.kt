package app.melyxar.player

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class PictureLightTest {
    @Test
    fun stretchesTheContrastAsMuchAsThePageDoes() {
        assertEquals(0f, contrastOf(1f))
        // Media3 stretches by (1 + c) / (1 - c): the factor asked for, back.
        listOf(0.8f, 1.2f, 1.5f).forEach { factor ->
            val c = contrastOf(factor)
            assertEquals(factor, (1 + c) / (1 - c), 1e-5f)
        }
    }

    @Test
    fun leavesThePictureAloneAtAHundredPercent() {
        assertEquals(emptyList<Any>(), lightEffects(1f, 1f))
    }
}
