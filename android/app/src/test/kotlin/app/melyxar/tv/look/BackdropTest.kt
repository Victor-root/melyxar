package app.melyxar.tv.look

import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import kotlin.math.hypot

class BackdropTest {
    @Test
    fun fallsBackToTheFirstPaintingForANumberThereIsNot() {
        assertEquals(4, lightNumber(4))
        assertEquals(10, lightNumber(10))
        assertEquals(1, lightNumber(0))
        assertEquals(1, lightNumber(11))
    }

    @Test
    fun fadesTheDustAsItsMaskDoes() {
        assertEquals(1f, fadeAt(0f, 0.46f, 0.45f, 0.78f))
        assertEquals(0.45f, fadeAt(0.46f, 0.46f, 0.45f, 0.78f), 1e-6f)
        assertEquals(0.725f, fadeAt(0.23f, 0.46f, 0.45f, 0.78f), 1e-6f)
        assertEquals(0.225f, fadeAt(0.62f, 0.46f, 0.45f, 0.78f), 1e-6f)
        assertEquals(0f, fadeAt(0.78f, 0.46f, 0.45f, 0.78f))
        assertEquals(0f, fadeAt(2f, 0.46f, 0.45f, 0.78f))
    }

    @Test
    fun laysALinearGradientAcrossTheBoxAsCssDoes() {
        assertArrayEquals(floatArrayOf(0f, 270f, 960f, 270f), gradientLine(90f, 960f, 540f), 1e-3f)
        assertArrayEquals(floatArrayOf(480f, 0f, 480f, 540f), gradientLine(180f, 960f, 540f), 1e-3f)
        // At an angle, the line reaches just far enough for the corners to lie
        // on the perpendiculars through its ends.
        val line = gradientLine(152f, 960f, 540f)
        assertEquals(960f * 0.46947f + 540f * 0.88295f, hypot(line[2] - line[0], line[3] - line[1]), 0.05f)
        assertEquals(480f, (line[0] + line[2]) / 2, 1e-3f)
        assertEquals(270f, (line[1] + line[3]) / 2, 1e-3f)
        assertTrue(line[2] > line[0] && line[3] > line[1])
    }
}
