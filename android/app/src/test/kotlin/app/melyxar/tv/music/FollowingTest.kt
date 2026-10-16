package app.melyxar.tv.music

import app.melyxar.server.LyricLine
import app.melyxar.server.SongSpectrum
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class FollowingTest {
    private val lines = listOf(LyricLine(1_000, "Over the harbour"), LyricLine(4_000, "the lanterns sway"), LyricLine(9_000, "and morning comes"))

    @Test
    fun findsTheLastLineWhoseMomentHasCome() {
        assertEquals(-1, lineAt(lines, 999))
        assertEquals(0, lineAt(lines, 1_000))
        assertEquals(1, lineAt(lines, 8_999))
        assertEquals(2, lineAt(lines, 60_000))
        assertEquals(-1, lineAt(emptyList(), 5_000))
    }

    @Test
    fun litsALineAQuarterOfASecondAhead() {
        assertEquals(0, litLineAt(lines, 3.7))
        assertEquals(1, litLineAt(lines, 3.75))
    }

    // Two bands, two readings a second: 0 to 255 in the first band, 255 to 0 in the second.
    private val spectrum = SongSpectrum(2, 2.0, byteArrayOf(0, 255.toByte(), 255.toByte(), 0))

    @Test
    fun readsTheLevelsBetweenTheReadingsAtTheMiddleOfEach() {
        val into = FloatArray(2)
        levelsAt(spectrum, 0.25, into)
        assertArrayEquals(floatArrayOf(0f, 1f), into, 0.001f)
        levelsAt(spectrum, 0.5, into)
        assertArrayEquals(floatArrayOf(0.5f, 0.5f), into, 0.001f)
        levelsAt(spectrum, 5.0, into)
        assertArrayEquals(floatArrayOf(1f, 0f), into, 0.001f)
        levelsAt(spectrum, 0.0, into)
        assertArrayEquals(floatArrayOf(0f, 1f), into, 0.001f)
    }

    @Test
    fun glidesTowardsTheTargetsUntilSettled() {
        val levels = floatArrayOf(0f, 1f)
        val targets = floatArrayOf(1f, 1f)
        assertFalse(approach(levels, targets))
        assertEquals(0.22f, levels[0], 0.001f)
        repeat(40) { approach(levels, targets) }
        assertTrue(approach(levels, targets))
    }

    @Test
    fun runsOnFromWhatThePlayerSaidButNotFarPastIt() {
        assertEquals(10.2, momentOf(10.0, 200), 0.0001)
        assertEquals(10.5, momentOf(10.0, 3_000), 0.0001)
        assertEquals(10.0, momentOf(10.0, -5), 0.0001)
    }

    @Test
    fun liftsEachBarByTheLoudestOfItsShareOfTheBands() {
        val bars = FloatArray(BARS)
        barsOf(floatArrayOf(0.1f, 0.4f, 0.2f, 0.2f, 0.9f, 0.3f, 0f, 0.5f), bars)
        assertArrayEquals(floatArrayOf(0.4f, 0.2f, 0.9f, 0.5f), bars, 0.0001f)
        barsOf(FloatArray(0), bars)
        assertArrayEquals(FloatArray(BARS), bars, 0.0001f)
    }

    @Test
    fun theMeterJumpsUpAtOnceAndFallsSlowly() {
        val meter = BarMeter()
        meter.step(floatArrayOf(0f, 0f, 0f, 0f), 33)
        assertEquals(0.15f, meter.shown[0], 0.001f)
        meter.step(floatArrayOf(1f, 0f, 0f, 0f), 33)
        // Up seven tenths of the way from the floor at once.
        assertEquals(0.15f + 0.85f * 0.7f, meter.shown[0], 0.001f)
        assertEquals(0.15f, meter.shown[1], 0.001f)
        meter.step(floatArrayOf(0f, 0f, 0f, 0f), 100)
        // Down by 1.6 heights a second, never below the floor.
        assertEquals(0.15f + 0.85f * 0.7f - 0.16f, meter.shown[0], 0.001f)
    }

    @Test
    fun keepsBeatsOfItsOwnThereAndBack() {
        assertEquals(0.25f, ownBeat(0, 0), 0.001f)
        assertEquals(1f, ownBeat(0, 360), 0.001f)
        assertEquals(0.85f, ownBeat(0, 900), 0.001f)
        assertEquals(0.25f, ownBeat(0, 1_800), 0.001f)
        assertTrue(ownBeat(1, 0) in 0.25f..1f)
    }
}
