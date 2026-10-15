// Android's numbers for profiles and levels newer than the oldest system this
// runs on are only ever compared with what a decoder reports: an older one
// never reports them, and nothing is asked of the system by naming them.
@file:SuppressLint("InlinedApi")

package app.melyxar.player

import android.annotation.SuppressLint
import android.media.MediaCodecInfo.CodecProfileLevel
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * What this device plays, said the way the server reads it.
 *
 * The same profile a browser sends with every film, except that here it is
 * read from the device's own decoders rather than asked of a browser, and
 * the names are the server's: the ones its analyser gives a codec, a profile
 * and a level. Worked out here from a plain description of the decoders, so
 * every name and number can be checked without a device.
 */

/** Everything the device plays, less the limits a viewer chooses, which the
 *  page adds when it asks for a film. */
@Serializable
data class ClientProfile(
    val containers: List<String>,
    val video: List<VideoCapability>,
    @SerialName("rebuilt_video") val rebuiltVideo: List<RebuiltCapability>,
    @SerialName("audio_codecs") val audioCodecs: List<String>,
    /** Nothing: the device mixes down whatever it is given to what it plays
     *  through, voices kept. */
    @SerialName("max_audio_channels") val maxAudioChannels: Int?,
    @SerialName("subtitle_formats") val subtitleFormats: List<String>,
    @SerialName("wide_gamut") val wideGamut: List<WideGamutCapability>,
    @SerialName("can_switch_tracks_in_container") val canSwitchTracksInContainer: Boolean,
)

@Serializable
data class VideoCapability(
    val codec: String,
    /** Empty for any profile. */
    val profiles: List<String> = emptyList(),
    /** Nothing for any level. */
    @SerialName("max_level") val maxLevel: Int? = null,
)

@Serializable
data class RebuiltCapability(
    val codec: String,
    @SerialName("max_height") val maxHeight: Int,
    @SerialName("power_efficient") val powerEfficient: Boolean,
)

@Serializable
data class WideGamutCapability(val codec: String, val curve: Curve)

@Serializable
enum class Curve {
    @SerialName("pq")
    Pq,

    @SerialName("hlg")
    Hlg,
}

/** One decoder of the device, as far as the profile needs to know it. */
data class Decoder(
    val mime: String,
    /** A chip of the device rather than software. */
    val hardware: Boolean,
    /** Every profile it decodes, each with the highest level it reaches in
     *  it, in Android's own numbers. */
    val profiles: List<ProfileLevel>,
    /** Tallest of the usual sizes it plays as fast as a film runs, or
     *  nothing when it plays none of them. */
    val tallest: Int?,
)

data class ProfileLevel(val profile: Int, val level: Int)

/** A codec of pictures, and how the server names what Android says of it. */
private class VideoCodec(
    val mime: String,
    val name: String,
    /** The server's names for each profile; empty when it names none. */
    val profiles: Map<Int, List<String>> = emptyMap(),
    /** The server's number for each level; empty when its analyser gives the
     *  codec none worth comparing. */
    val levels: Map<Int, Int> = emptyMap(),
    /** The profiles a decoder has to name to show each curve as it is. */
    val curves: Map<Curve, Set<Int>> = emptyMap(),
)

private val H264 = VideoCodec(
    mime = "video/avc",
    name = "h264",
    profiles = mapOf(
        CodecProfileLevel.AVCProfileBaseline to listOf("Baseline", "Constrained Baseline"),
        CodecProfileLevel.AVCProfileConstrainedBaseline to listOf("Constrained Baseline"),
        CodecProfileLevel.AVCProfileMain to listOf("Main"),
        CodecProfileLevel.AVCProfileExtended to listOf("Extended"),
        CodecProfileLevel.AVCProfileHigh to listOf("High"),
        CodecProfileLevel.AVCProfileHigh10 to listOf("High 10"),
        CodecProfileLevel.AVCProfileHigh422 to listOf("High 4:2:2"),
        CodecProfileLevel.AVCProfileHigh444 to listOf("High 4:4:4 Predictive"),
    ),
    levels = mapOf(
        CodecProfileLevel.AVCLevel1 to 10,
        CodecProfileLevel.AVCLevel11 to 11,
        CodecProfileLevel.AVCLevel12 to 12,
        CodecProfileLevel.AVCLevel13 to 13,
        CodecProfileLevel.AVCLevel2 to 20,
        CodecProfileLevel.AVCLevel21 to 21,
        CodecProfileLevel.AVCLevel22 to 22,
        CodecProfileLevel.AVCLevel3 to 30,
        CodecProfileLevel.AVCLevel31 to 31,
        CodecProfileLevel.AVCLevel32 to 32,
        CodecProfileLevel.AVCLevel4 to 40,
        CodecProfileLevel.AVCLevel41 to 41,
        CodecProfileLevel.AVCLevel42 to 42,
        CodecProfileLevel.AVCLevel5 to 50,
        CodecProfileLevel.AVCLevel51 to 51,
        CodecProfileLevel.AVCLevel52 to 52,
        CodecProfileLevel.AVCLevel6 to 60,
        CodecProfileLevel.AVCLevel61 to 61,
        CodecProfileLevel.AVCLevel62 to 62,
    ),
)

/** Levels of either tier: the analyser gives the level alone, thirty times
 *  its number. */
private val HEVC = VideoCodec(
    mime = "video/hevc",
    name = "hevc",
    profiles = mapOf(
        CodecProfileLevel.HEVCProfileMain to listOf("Main"),
        CodecProfileLevel.HEVCProfileMain10 to listOf("Main 10"),
        CodecProfileLevel.HEVCProfileMain10HDR10 to listOf("Main 10"),
        CodecProfileLevel.HEVCProfileMain10HDR10Plus to listOf("Main 10"),
        CodecProfileLevel.HEVCProfileMainStill to listOf("Main Still Picture"),
    ),
    levels = mapOf(
        CodecProfileLevel.HEVCMainTierLevel1 to 30,
        CodecProfileLevel.HEVCHighTierLevel1 to 30,
        CodecProfileLevel.HEVCMainTierLevel2 to 60,
        CodecProfileLevel.HEVCHighTierLevel2 to 60,
        CodecProfileLevel.HEVCMainTierLevel21 to 63,
        CodecProfileLevel.HEVCHighTierLevel21 to 63,
        CodecProfileLevel.HEVCMainTierLevel3 to 90,
        CodecProfileLevel.HEVCHighTierLevel3 to 90,
        CodecProfileLevel.HEVCMainTierLevel31 to 93,
        CodecProfileLevel.HEVCHighTierLevel31 to 93,
        CodecProfileLevel.HEVCMainTierLevel4 to 120,
        CodecProfileLevel.HEVCHighTierLevel4 to 120,
        CodecProfileLevel.HEVCMainTierLevel41 to 123,
        CodecProfileLevel.HEVCHighTierLevel41 to 123,
        CodecProfileLevel.HEVCMainTierLevel5 to 150,
        CodecProfileLevel.HEVCHighTierLevel5 to 150,
        CodecProfileLevel.HEVCMainTierLevel51 to 153,
        CodecProfileLevel.HEVCHighTierLevel51 to 153,
        CodecProfileLevel.HEVCMainTierLevel52 to 156,
        CodecProfileLevel.HEVCHighTierLevel52 to 156,
        CodecProfileLevel.HEVCMainTierLevel6 to 180,
        CodecProfileLevel.HEVCHighTierLevel6 to 180,
        CodecProfileLevel.HEVCMainTierLevel61 to 183,
        CodecProfileLevel.HEVCHighTierLevel61 to 183,
        CodecProfileLevel.HEVCMainTierLevel62 to 186,
        CodecProfileLevel.HEVCHighTierLevel62 to 186,
    ),
    curves = mapOf(
        Curve.Pq to setOf(CodecProfileLevel.HEVCProfileMain10HDR10, CodecProfileLevel.HEVCProfileMain10HDR10Plus),
        Curve.Hlg to setOf(CodecProfileLevel.HEVCProfileMain10),
    ),
)

/** The analyser rarely gives this codec a level, so none is compared. */
private val VP9 = VideoCodec(
    mime = "video/x-vnd.on2.vp9",
    name = "vp9",
    profiles = mapOf(
        CodecProfileLevel.VP9Profile0 to listOf("Profile 0"),
        CodecProfileLevel.VP9Profile1 to listOf("Profile 1"),
        CodecProfileLevel.VP9Profile2 to listOf("Profile 2"),
        CodecProfileLevel.VP9Profile2HDR to listOf("Profile 2"),
        CodecProfileLevel.VP9Profile2HDR10Plus to listOf("Profile 2"),
        CodecProfileLevel.VP9Profile3 to listOf("Profile 3"),
        CodecProfileLevel.VP9Profile3HDR to listOf("Profile 3"),
        CodecProfileLevel.VP9Profile3HDR10Plus to listOf("Profile 3"),
    ),
    curves = mapOf(
        Curve.Pq to setOf(CodecProfileLevel.VP9Profile2HDR, CodecProfileLevel.VP9Profile2HDR10Plus),
        Curve.Hlg to setOf(CodecProfileLevel.VP9Profile2),
    ),
)

/** Each of Android's levels is one bit, and the analyser gives the level's
 *  index: the place of that bit. */
private val AV1 = VideoCodec(
    mime = "video/av01",
    name = "av1",
    profiles = mapOf(
        CodecProfileLevel.AV1ProfileMain8 to listOf("Main"),
        CodecProfileLevel.AV1ProfileMain10 to listOf("Main"),
        CodecProfileLevel.AV1ProfileMain10HDR10 to listOf("Main"),
        CodecProfileLevel.AV1ProfileMain10HDR10Plus to listOf("Main"),
    ),
    levels = listOf(
        CodecProfileLevel.AV1Level2, CodecProfileLevel.AV1Level21, CodecProfileLevel.AV1Level22,
        CodecProfileLevel.AV1Level23, CodecProfileLevel.AV1Level3, CodecProfileLevel.AV1Level31,
        CodecProfileLevel.AV1Level32, CodecProfileLevel.AV1Level33, CodecProfileLevel.AV1Level4,
        CodecProfileLevel.AV1Level41, CodecProfileLevel.AV1Level42, CodecProfileLevel.AV1Level43,
        CodecProfileLevel.AV1Level5, CodecProfileLevel.AV1Level51, CodecProfileLevel.AV1Level52,
        CodecProfileLevel.AV1Level53, CodecProfileLevel.AV1Level6, CodecProfileLevel.AV1Level61,
        CodecProfileLevel.AV1Level62, CodecProfileLevel.AV1Level63, CodecProfileLevel.AV1Level7,
        CodecProfileLevel.AV1Level71, CodecProfileLevel.AV1Level72, CodecProfileLevel.AV1Level73,
    ).associateWith { Integer.numberOfTrailingZeros(it) },
    curves = mapOf(
        Curve.Pq to setOf(CodecProfileLevel.AV1ProfileMain10HDR10, CodecProfileLevel.AV1ProfileMain10HDR10Plus),
        Curve.Hlg to setOf(CodecProfileLevel.AV1ProfileMain10),
    ),
)

/** Every codec of pictures a file may hold that the device's player reads. */
private val VIDEO = listOf(
    H264,
    HEVC,
    VP9,
    AV1,
    VideoCodec(mime = "video/x-vnd.on2.vp8", name = "vp8"),
    VideoCodec(mime = "video/mpeg2", name = "mpeg2video"),
    VideoCodec(mime = "video/mp4v-es", name = "mpeg4"),
)

/** What the server may rebuild a film into, as it names them. */
private val REBUILT = listOf(AV1, HEVC, H264)

/** The containers the device's player opens, as the analyser names them. */
private val CONTAINERS = listOf("mp4", "mov", "matroska", "webm", "mpegts", "avi", "flv")

/** Codecs of sound, by the server's name, with the decoder that plays each. */
val SOUND_DECODERS = mapOf(
    "aac" to "audio/mp4a-latm",
    "mp3" to "audio/mpeg",
    "opus" to "audio/opus",
    "vorbis" to "audio/vorbis",
    "flac" to "audio/flac",
    "alac" to "audio/alac",
    "ac3" to "audio/ac3",
    "eac3" to "audio/eac3",
    "dts" to "audio/vnd.dts",
    "truehd" to "audio/true-hd",
)

/** The forms the page's player of music asks the server for, by name. */
private val MUSIC_FORMS = listOf("mp3", "aac", "flac", "opus", "vorbis", "alac")

/**
 * The forms of sound the page's player of music is told the device plays, as
 * the server reads them: those it has a decoder for, and plain wave sound,
 * which needs none.
 */
fun musicFormsOf(decoded: Set<String>): String =
    (MUSIC_FORMS.filter { SOUND_DECODERS[it] in decoded } + "wav").joinToString(",")

/** Sound carried as it is, which the player hands on without decoding. */
private val PLAIN_SOUND = listOf("pcm_s16le", "pcm_s24le")

/**
 * The profile of a device with these decoders, on a screen showing these
 * curves, playing these codecs of sound: decoded, or handed as they are to
 * what the device plays through.
 */
fun profileOf(decoders: List<Decoder>, shownCurves: Set<Curve>, playedSound: Set<String>) = ClientProfile(
    containers = CONTAINERS,
    video = VIDEO.flatMap { capabilitiesOf(it, decoders.thatPlay(it.mime)) },
    rebuiltVideo = REBUILT.mapNotNull { rebuiltOf(it, decoders.thatPlay(it.mime)) },
    audioCodecs = SOUND_DECODERS.keys.filter { it in playedSound } + PLAIN_SOUND,
    maxAudioChannels = null,
    // The page draws the words itself, from the files the server writes.
    subtitleFormats = listOf("webvtt"),
    wideGamut = VIDEO.flatMap { codec ->
        val playing = decoders.thatPlay(codec.mime)
        shownCurves.filter { curve ->
            val needed = codec.curves[curve].orEmpty()
            playing.any { decoder -> decoder.profiles.any { it.profile in needed } }
        }.map { WideGamutCapability(codec.name, it) }
    },
    // The page asks for another track by asking the server for the film
    // again, as a browser does.
    canSwitchTracksInContainer = false,
)

/** The decoders a film of this kind is played with: the device's chips when
 *  it has any, since software is only for what they do not take, and a film
 *  the chips do not take is better rebuilt than played in software. */
private fun List<Decoder>.thatPlay(mime: String): List<Decoder> {
    val all = filter { it.mime == mime }
    return all.filter { it.hardware }.ifEmpty { all }
}

private fun capabilitiesOf(codec: VideoCodec, playing: List<Decoder>): List<VideoCapability> {
    if (playing.isEmpty()) {
        return emptyList()
    }
    // The highest level reached in each profile, by any of the decoders; a
    // codec without levels keeps nothing, which is any level.
    val reached = sortedMapOf<String, Int?>()
    for (decoder in playing) {
        for (entry in decoder.profiles) {
            val level = if (codec.levels.isEmpty()) null else codec.levels[entry.level] ?: continue
            for (name in codec.profiles[entry.profile].orEmpty()) {
                val before = reached[name]
                reached[name] = if (before == null || level == null) level else maxOf(before, level)
            }
        }
    }
    if (reached.isEmpty()) {
        return listOf(VideoCapability(codec.name))
    }
    return reached.map { (name, level) -> VideoCapability(codec.name, listOf(name), level) }
}

private fun rebuiltOf(codec: VideoCodec, playing: List<Decoder>): RebuiltCapability? {
    val tallest = playing.mapNotNull { it.tallest }.maxOrNull() ?: return null
    return RebuiltCapability(codec.name, tallest, powerEfficient = playing.any { it.hardware })
}
