package app.melyxar.tv.play

import app.melyxar.server.PictureHeld
import app.melyxar.server.PictureRebuild
import app.melyxar.server.SoundHeld
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class DescribedTest {
    @Test
    fun ratesAndSizesReadAsTheWebReadsThem() {
        assertNull(asRate(null))
        assertNull(asRate(0))
        assertEquals("640 kb/s", asRate(640_000))
        assertEquals("4.5 Mb/s", asRate(4_500_000))
        assertEquals("25 Mb/s", asRate(24_600_000))
        assertEquals("1.4 GB", asSize(1_400_000_000))
        assertEquals("700 MB", asSize(700_000_000))
    }

    @Test
    fun theSpeedKeepsTwoFiguresUntilItIsLarge() {
        assertEquals("0.85", speedSaid(0.85))
        assertEquals("12", speedSaid(12.4))
    }

    @Test
    fun aContainerIsCalledWhatPeopleCallIt() {
        assertEquals("MKV", containerName("matroska,webm"))
        assertEquals("MP4", containerName("mov,mp4,m4a,3gp,3g2,mj2"))
        assertEquals("AVI", containerName("avi"))
    }

    @Test
    fun thePictureAndTheSoundAreSaidInOneLine() {
        val picture = PictureHeld("hevc", "Main 10", 3840, 2160, frameRate = 23.976, bitrate = 40_000_000, hdr = "hdr10", bitDepth = 10)
        assertEquals("HEVC · Main 10 · 3840x2160 · 10 bit · HDR10 · 23.976 fps · 40 Mb/s", pictureHeld(picture))
        assertEquals("AC3 · 6 channels · 48.0 kHz", soundHeld(SoundHeld("ac3", 6, sampleRate = 48_000)) { "$it channels" })
        assertEquals("EAC3 · 5.1(side)", soundHeld(SoundHeld("eac3", 6, channelLayout = "5.1(side)")) { "$it channels" })
    }

    @Test
    fun whatBecomesOfThePictureSaysWhoRebuildsIt() {
        assertEquals("as it is", pictureDone(null, "by the card", "as it is"))
        assertEquals("by the card · H264 · 1080p · 8.0 Mb/s", pictureDone(PictureRebuild("card", "h264", 1080, 8_000_000), "by the card", "as it is"))
    }

    @Test
    fun theSoundIsRebuiltOnlyWhenTheWayItIsPlayedSaysSo() {
        assertTrue(soundRebuilt("transcode_audio"))
        assertTrue(soundRebuilt("full_transcode"))
        assertFalse(soundRebuilt("remux"))
    }

    @Test
    fun theReasonsAreReadByTheirCodes() {
        val reasons = Json.parseToJsonElement("""[{"code":"audio_codec_not_supported","codec":"truehd"},{"other":1},{"code":"downmix_requested"}]""") as JsonArray
        assertEquals(listOf("audio_codec_not_supported", "downmix_requested"), reasonCodes(reasons))
    }
}
