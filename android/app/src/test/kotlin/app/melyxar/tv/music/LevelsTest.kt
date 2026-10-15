package app.melyxar.tv.music

import app.melyxar.server.Song
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class LevelsTest {
    private fun song(lufs: Double?, peak: Double? = null, album: Double? = null) =
        Song(id = "s", title = "Quiet Harbour", lufs = lufs, peakDbfs = peak, albumLufs = album)

    @Test
    fun aLoudSongIsBroughtDown() {
        assertEquals(0.5012, levelOf(song(-8.0), "track").toDouble(), 0.001)
    }

    @Test
    fun aQuietSongIsRaisedOnlyAsFarAsItsPeakAllows() {
        assertEquals(1.4125, levelOf(song(-20.0, peak = -4.0), "track").toDouble(), 0.001)
        assertEquals(1f, levelOf(song(-20.0), "track"))
    }

    @Test
    fun theAlbumsLevelIsUsedForTheAlbum() {
        assertEquals(0.5012, levelOf(song(-20.0, album = -8.0), "album").toDouble(), 0.001)
        assertEquals(1f, levelOf(song(-8.0), "off"))
        assertEquals(1f, levelOf(song(-80.0), "track"))
    }

    @Test
    fun aFadeIsNeverMoreThanAThirdOfASong() {
        assertEquals(6.0, fadeBetween(8.0, 18.0, 300.0))
        assertEquals(8.0, fadeBetween(8.0, null, null))
    }

    @Test
    fun aSongCountsHalfwayOrFourMinutesIn() {
        assertTrue(countsAsListened(100.0, 200.0))
        assertFalse(countsAsListened(99.0, 200.0))
        assertTrue(countsAsListened(240.0, 900.0))
        assertFalse(countsAsListened(20.0, 25.0))
    }
}
