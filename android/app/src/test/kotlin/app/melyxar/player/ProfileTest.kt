package app.melyxar.player

import android.media.MediaCodecInfo.CodecProfileLevel
import kotlinx.serialization.json.Json
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ProfileTest {
    private fun decoder(mime: String, hardware: Boolean = true, tallest: Int? = 2160, vararg profiles: Pair<Int, Int>) =
        Decoder(mime, hardware, profiles.map { (profile, level) -> ProfileLevel(profile, level) }, tallest)

    @Test
    fun namesProfilesAndLevelsAsTheAnalyserDoes() {
        val profile = profileOf(
            listOf(
                decoder(
                    "video/avc",
                    profiles = arrayOf(
                        CodecProfileLevel.AVCProfileHigh to CodecProfileLevel.AVCLevel51,
                        CodecProfileLevel.AVCProfileBaseline to CodecProfileLevel.AVCLevel31,
                    ),
                ),
                decoder(
                    "video/hevc",
                    profiles = arrayOf(CodecProfileLevel.HEVCProfileMain10 to CodecProfileLevel.HEVCHighTierLevel51),
                ),
                decoder(
                    "video/av01",
                    profiles = arrayOf(CodecProfileLevel.AV1ProfileMain10 to CodecProfileLevel.AV1Level51),
                ),
            ),
            emptySet(),
            emptySet(),
        )
        assertEquals(
            listOf(
                VideoCapability("h264", listOf("Baseline"), 31),
                VideoCapability("h264", listOf("Constrained Baseline"), 31),
                VideoCapability("h264", listOf("High"), 51),
                VideoCapability("hevc", listOf("Main 10"), 153),
                VideoCapability("av1", listOf("Main"), 13),
            ),
            profile.video,
        )
    }

    @Test
    fun keepsTheHighestLevelOfEachProfile() {
        val profile = profileOf(
            listOf(
                decoder("video/avc", profiles = arrayOf(CodecProfileLevel.AVCProfileMain to CodecProfileLevel.AVCLevel4)),
                decoder("video/avc", profiles = arrayOf(CodecProfileLevel.AVCProfileMain to CodecProfileLevel.AVCLevel52)),
            ),
            emptySet(),
            emptySet(),
        )
        assertEquals(listOf(VideoCapability("h264", listOf("Main"), 52)), profile.video)
    }

    @Test
    fun leavesSoftwareAsideWhenTheDeviceHasAChip() {
        val profile = profileOf(
            listOf(
                decoder(
                    "video/hevc",
                    hardware = true,
                    tallest = 1080,
                    profiles = arrayOf(CodecProfileLevel.HEVCProfileMain to CodecProfileLevel.HEVCMainTierLevel41),
                ),
                decoder(
                    "video/hevc",
                    hardware = false,
                    tallest = 2160,
                    profiles = arrayOf(CodecProfileLevel.HEVCProfileMain10 to CodecProfileLevel.HEVCMainTierLevel51),
                ),
                decoder(
                    "video/av01",
                    hardware = false,
                    tallest = 1080,
                    profiles = arrayOf(CodecProfileLevel.AV1ProfileMain8 to CodecProfileLevel.AV1Level4),
                ),
            ),
            emptySet(),
            emptySet(),
        )
        assertEquals(
            listOf(VideoCapability("hevc", listOf("Main"), 123), VideoCapability("av1", listOf("Main"), 8)),
            profile.video,
        )
        assertEquals(
            listOf(RebuiltCapability("av1", 1080, false), RebuiltCapability("hevc", 1080, true)),
            profile.rebuiltVideo,
        )
    }

    @Test
    fun takesAnyProfileOfACodecItCannotName() {
        val profile = profileOf(
            listOf(
                decoder("video/x-vnd.on2.vp9", profiles = arrayOf(CodecProfileLevel.VP9Profile0 to CodecProfileLevel.VP9Level5)),
                decoder("video/mpeg2", profiles = arrayOf(1 to 1)),
                decoder("video/avc"),
            ),
            emptySet(),
            emptySet(),
        )
        assertEquals(
            listOf(
                VideoCapability("h264"),
                VideoCapability("vp9", listOf("Profile 0"), null),
                VideoCapability("mpeg2video"),
            ),
            profile.video,
        )
    }

    @Test
    fun showsWideGamutOnlyWhereScreenAndDecoderBothDo() {
        val decoders = listOf(
            decoder(
                "video/hevc",
                profiles = arrayOf(
                    CodecProfileLevel.HEVCProfileMain10 to CodecProfileLevel.HEVCMainTierLevel51,
                    CodecProfileLevel.HEVCProfileMain10HDR10 to CodecProfileLevel.HEVCMainTierLevel51,
                ),
            ),
            decoder(
                "video/av01",
                profiles = arrayOf(CodecProfileLevel.AV1ProfileMain10 to CodecProfileLevel.AV1Level51),
            ),
        )
        assertEquals(
            listOf(
                WideGamutCapability("hevc", Curve.Pq),
                WideGamutCapability("hevc", Curve.Hlg),
                WideGamutCapability("av1", Curve.Hlg),
            ),
            profileOf(decoders, setOf(Curve.Pq, Curve.Hlg), emptySet()).wideGamut,
        )
        assertTrue(profileOf(decoders, emptySet(), emptySet()).wideGamut.isEmpty())
    }

    @Test
    fun writesWhatTheServerReads() {
        val profile = profileOf(
            listOf(decoder("video/avc", tallest = 1080)),
            setOf(Curve.Pq),
            setOf("aac", "eac3"),
        )
        assertEquals(
            """{"containers":["mp4","mov","matroska","webm","mpegts","avi","flv"],""" +
                """"video":[{"codec":"h264"}],""" +
                """"rebuilt_video":[{"codec":"h264","max_height":1080,"power_efficient":true}],""" +
                """"audio_codecs":["aac","eac3","pcm_s16le","pcm_s24le"],"max_audio_channels":null,""" +
                """"subtitle_formats":["webvtt"],"wide_gamut":[],"can_switch_tracks_in_container":false}""",
            Json.encodeToString(profile),
        )
    }
}
