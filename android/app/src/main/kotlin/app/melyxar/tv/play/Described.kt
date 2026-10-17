package app.melyxar.tv.play

import app.melyxar.server.PictureHeld
import app.melyxar.server.PictureRebuild
import app.melyxar.server.SoundHeld
import java.util.Locale
import kotlin.math.roundToInt
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive

/*
 * What a playback is, in words, by the web's own rules (player/describe.ts):
 * what the file holds, what is being done to it, and how hard the machine
 * works at it. What is worded in the screen's language is handed in.
 */

/** Below this the machine is producing the film more slowly than it plays. */
const val KEEPING_UP = 1.0

/** A rate in bits a second, as somebody reads one. */
fun asRate(bits: Long?): String? = when {
    bits == null || bits <= 0 -> null
    bits >= 1_000_000 -> String.format(Locale.ROOT, if (bits >= 10_000_000) "%.0f Mb/s" else "%.1f Mb/s", bits / 1_000_000.0)
    else -> "${(bits / 1_000.0).roundToInt()} kb/s"
}

/** A size in bytes, as somebody reads one. */
fun asSize(bytes: Long): String {
    val giga = bytes / 1_000_000_000.0
    return if (giga >= 1) String.format(Locale.ROOT, "%.1f GB", giga) else "${(bytes / 1_000_000.0).roundToInt()} MB"
}

/** How many times real time the machine works at, as the web says it. */
fun speedSaid(speed: Double): String =
    if (speed >= 10) speed.roundToInt().toString() else String.format(Locale.ROOT, "%.2f", speed)

/** A container as somebody calls it rather than as the analyser does. */
fun containerName(format: String): String {
    val first = format.split(",").first().trim()
    return when (first) {
        "matroska" -> "MKV"
        "mov" -> "MP4"
        "mpegts" -> "MPEG-TS"
        "mpeg" -> "MPEG-PS"
        "asf" -> "WMV"
        else -> first.uppercase()
    }
}

private fun hdrName(hdr: String): String = when (hdr) {
    "hdr10" -> "HDR10"
    "hlg" -> "HLG"
    "dolby_vision" -> "Dolby Vision"
    else -> hdr.uppercase()
}

/** The picture the file holds. */
fun pictureHeld(picture: PictureHeld): String = listOfNotNull(
    picture.codec.uppercase(),
    picture.profile,
    "${picture.width}x${picture.height}",
    picture.bitDepth?.let { "$it bit" },
    picture.hdr?.let(::hdrName),
    picture.frameRate?.let { String.format(Locale.ROOT, "%.3f fps", it) },
    asRate(picture.bitrate),
).joinToString(" · ")

/** What becomes of the picture on its way: [byWhom] names what rebuilds it,
 *  and [carriedOver] says it goes as it is. */
fun pictureDone(rebuild: PictureRebuild?, byWhom: String, carriedOver: String): String =
    if (rebuild == null) {
        carriedOver
    } else {
        listOfNotNull(byWhom, rebuild.codec.uppercase(), rebuild.height?.let { "${it}p" }, asRate(rebuild.bitrate)).joinToString(" · ")
    }

/** The soundtrack being played, as the file holds it; [channels] words how
 *  many channels a file without a layout says it has. */
fun soundHeld(sound: SoundHeld, channels: (Int) -> String): String = listOfNotNull(
    sound.codec.uppercase(),
    sound.channelLayout ?: channels(sound.channels),
    sound.sampleRate?.let { String.format(Locale.ROOT, "%.1f kHz", it / 1000.0) },
    asRate(sound.bitrate),
).joinToString(" · ")

/** Whether the sound is rebuilt on its way, by the way the film is played. */
fun soundRebuilt(method: String): Boolean = method == "full_transcode" || method == "transcode_audio"

/** The codes of the reasons behind a plan, in its order. */
fun reasonCodes(reasons: List<JsonElement>): List<String> =
    reasons.mapNotNull { (it as? JsonObject)?.get("code")?.jsonPrimitive?.contentOrNull }
