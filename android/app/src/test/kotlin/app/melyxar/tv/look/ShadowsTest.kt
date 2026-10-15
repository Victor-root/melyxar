package app.melyxar.tv.look

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ShadowsTest {
    /** The deviation Android works a blur radius out to. */
    private fun deviationOf(radius: Float) = 0.57735f * radius + 0.5f

    @Test
    fun blursByHalfTheWebsRadiusAsCssDoes() {
        listOf(8f, 16f, 26f, 40f).forEach { css ->
            assertEquals(css / 2, deviationOf(blurOf(css)!!), 0.001f)
        }
    }

    @Test
    fun leavesOutABlurTooSmallToShow() {
        assertNull(blurOf(0f))
        assertNull(blurOf(1f))
    }
}
