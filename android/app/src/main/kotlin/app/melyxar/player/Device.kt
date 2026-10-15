package app.melyxar.player

import android.content.Context
import android.media.MediaCodecInfo
import android.media.MediaCodecList
import android.os.Build
import android.view.Display
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioCapabilities

/*
 * What this device plays, read from the device itself: its decoders, its
 * screen, and what it plays sound through.
 */

/** The usual sizes of a film, tallest first, as across and down. */
private val SIZES = listOf(3840 to 2160, 2560 to 1440, 1920 to 1080, 1280 to 720, 854 to 480)

/** A film's pace, asked of a decoder with room to spare. */
private const val PICTURES_PER_SECOND = 30.0

/** Codecs of sound a receiver plugged to the device may be handed as they
 *  are, by the server's name. */
private val PASSED_THROUGH = mapOf(
    "ac3" to C.ENCODING_AC3,
    "eac3" to C.ENCODING_E_AC3,
    "dts" to C.ENCODING_DTS,
    "truehd" to C.ENCODING_DOLBY_TRUEHD,
)

@OptIn(UnstableApi::class)
fun measuredProfile(context: Context, sound: AudioAttributes): ClientProfile {
    val decoders = decoders()
    val decoded = decoders.map { it.mime }.toSet()
    // Only codecs handed on as they are matter here, which a spatial mix of
    // the sound has nothing to do with.
    val passing = AudioCapabilities.getCapabilities(context, sound, null, emptyList())
    val playedSound = SOUND_DECODERS.filter { (name, mime) ->
        mime in decoded || PASSED_THROUGH[name]?.let(passing::supportsEncoding) == true
    }.keys
    return profileOf(decoders, shownCurves(context), playedSound)
}

/** The forms of sound the page's player of music can be sent as they are. */
fun measuredMusicForms(): String = musicFormsOf(decoders().map { it.mime }.toSet())

private fun decoders(): List<Decoder> = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos
    .filter { info -> !info.isEncoder && !info.name.endsWith(".secure") && !isAlias(info) }
    .flatMap { info ->
        info.supportedTypes.mapNotNull { mime ->
            val capabilities = try {
                info.getCapabilitiesForType(mime)
            } catch (_: IllegalArgumentException) {
                return@mapNotNull null
            }
            val pictures = capabilities.videoCapabilities
            Decoder(
                mime = mime,
                hardware = isHardware(info),
                profiles = capabilities.profileLevels.map { ProfileLevel(it.profile, it.level) },
                tallest = pictures?.let { sizes ->
                    SIZES.firstOrNull { (across, down) ->
                        sizes.areSizeAndRateSupported(across, down, PICTURES_PER_SECOND)
                    }?.second
                },
            )
        }
    }

/** Another name for a decoder listed under its own, counted once. */
private fun isAlias(info: MediaCodecInfo) = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && info.isAlias

/** Before Android 10 a decoder does not say, and its name is read the way
 *  Media3 reads it. */
private fun isHardware(info: MediaCodecInfo): Boolean {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        return info.isHardwareAccelerated
    }
    val name = info.name.lowercase()
    if (name.startsWith("arc.")) {
        return true
    }
    val software = name.startsWith("omx.google.") ||
        name.startsWith("omx.ffmpeg.") ||
        (name.startsWith("omx.sec.") && name.contains(".sw.")) ||
        name == "omx.qcom.video.decoder.hevcswvdec" ||
        name.startsWith("c2.android.") ||
        name.startsWith("c2.google.") ||
        (!name.startsWith("omx.") && !name.startsWith("c2."))
    return !software
}

/** The curves of wide gamut the screen shows as they are. */
private fun shownCurves(context: Context): Set<Curve> {
    val display = ContextCompat.getDisplayOrDefault(context)
    val types = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        display.mode.supportedHdrTypes
    } else {
        @Suppress("DEPRECATION")
        display.hdrCapabilities?.supportedHdrTypes ?: intArrayOf()
    }
    return buildSet {
        if (Display.HdrCapabilities.HDR_TYPE_HDR10 in types || Display.HdrCapabilities.HDR_TYPE_HDR10_PLUS in types) {
            add(Curve.Pq)
        }
        if (Display.HdrCapabilities.HDR_TYPE_HLG in types) {
            add(Curve.Hlg)
        }
    }
}
