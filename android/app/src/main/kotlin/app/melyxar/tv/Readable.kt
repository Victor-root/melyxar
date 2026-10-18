package app.melyxar.tv

import app.melyxar.server.Card
import java.text.DateFormat
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/*
 * What the server sends, read for people, as the web reads it
 * (`web/src/readable.ts`, `watching.ts`, `screens/home.ts`). Only numbers and
 * names here: the words around them are the screens', in both languages.
 */

/** How long a play of a card lasts, in seconds: its file's own length, and
 *  the catalogue's only when the file was never measured. */
fun lengthOfAPlay(card: Card): Long? = card.resumeLengthSeconds?.takeIf { it > 0 }
    ?: card.runtimeMinutes?.takeIf { it > 0 }?.let { it * 60L }

/** How far into a play somebody is, from 0 to 1, when its length is known. */
fun howFarIn(seconds: Double, length: Long?): Float? =
    length?.takeIf { it > 0 }?.let { min(1f, (seconds / it).toFloat()) }

/** The whole minutes left of a play, or nothing when none are or the length
 *  is unknown. */
fun minutesLeft(seconds: Double, length: Long?): Int? {
    if (length == null || length <= 0) {
        return null
    }
    return max(((length - seconds) / 60).roundToInt(), 0).takeIf { it > 0 }
}

/** Hours and minutes of a length in minutes, as a person says it. */
data class Duration(val hours: Int, val minutes: Int)

fun durationOf(minutes: Int) = Duration(minutes / 60, minutes % 60)

/** How large a picture is, by whichever side says more, as every player
 *  names it; nothing below 720p, which reads as a fault on a badge. */
fun pictureName(width: Int?, height: Int?): String? {
    val across = width ?: 0
    val down = height ?: 0
    return when {
        across <= 0 && down <= 0 -> null
        across >= 3200 || down >= 2000 -> "4K"
        across >= 2400 || down >= 1400 -> "1440p"
        across >= 1800 || down >= 1000 -> "1080p"
        across >= 1200 || down >= 700 -> "720p"
        else -> null
    }
}

private val RANGE_NAMES = mapOf("hdr10" to "HDR10", "hlg" to "HLG", "dolby_vision" to "Dolby Vision")

private val SOUND_NAMES = mapOf(
    "ac3" to "Dolby Digital",
    "eac3" to "Dolby Digital+",
    "truehd" to "Dolby TrueHD",
    "dts" to "DTS",
    "aac" to "AAC",
    "flac" to "FLAC",
    "opus" to "Opus",
    "mp3" to "MP3",
    "vorbis" to "Vorbis",
)

/** A codec of sound as its makers name it. */
fun soundName(sound: String?): String? = when {
    sound == null -> null
    // Atmos is written in the profile beside whatever carries it.
    sound.contains("atmos", ignoreCase = true) -> "Dolby Atmos"
    sound.startsWith("pcm") -> "PCM"
    else -> SOUND_NAMES[sound.lowercase()] ?: sound
}

/** A range of light under its makers' own name: "Dolby Vision", "HDR10". */
fun rangeName(hdr: String): String = RANGE_NAMES[hdr] ?: hdr.uppercase()

/** The badges of what a file holds: its size, its range and its sound,
 *  under their makers' own names, never translated. */
fun fileBadges(width: Int?, height: Int?, hdr: String?, sound: String?): List<String> = listOfNotNull(
    pictureName(width, height),
    hdr?.let(::rangeName),
    soundName(sound),
)

/** How many speakers a track of sound is made for, as a box says it, when
 *  it is more than two. */
fun speakersName(channels: Int?): String? = when (channels) {
    null, 0, 1, 2 -> null
    6 -> "5.1"
    8 -> "7.1"
    else -> "$channels ch"
}

/** A moment of a film as a player's clock shows it: 1:02:33, or 4:05 under
 *  an hour. */
fun clock(seconds: Double): String {
    val whole = seconds.toLong().coerceAtLeast(0)
    val hours = whole / 3600
    val minutes = whole % 3600 / 60
    val rest = whole % 60
    return if (hours > 0) {
        String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, rest)
    } else {
        String.format(Locale.ROOT, "%d:%02d", minutes, rest)
    }
}

/** A speed of play as the web's player writes it: "1×", "1.25×". */
fun speedName(speed: Float): String = "${if (speed % 1f == 0f) speed.toInt().toString() else speed.toString()}×"

/** How far the words are moved, as the web's player writes it: "+1.5 s". */
fun offsetName(seconds: Float): String =
    (if (seconds > 0) "+" else "") + String.format(Locale.ROOT, "%.1f s", seconds)

/** The older names of languages files still carry, as the ones a device
 *  reads. */
private val TERMINOLOGY = mapOf(
    "fre" to "fra", "ger" to "deu", "dut" to "nld", "chi" to "zho", "cze" to "ces", "gre" to "ell",
    "per" to "fas", "rum" to "ron", "slo" to "slk", "alb" to "sqi", "arm" to "hye", "baq" to "eus",
    "bur" to "mya", "geo" to "kat", "ice" to "isl", "mac" to "mkd", "mao" to "mri", "may" to "msa",
    "tib" to "bod", "wel" to "cym",
)

/** The two letter name of each language with one, by its three letter
 *  name: not every device names a language from its three letters. */
private val TWO_LETTERS: Map<String, String> by lazy {
    Locale.getISOLanguages().associateBy { Locale.forLanguageTag(it).isO3Language }
}

/** A language as a track names it, in the words of [shownIn], or nothing
 *  for a language nobody named. */
fun languageName(code: String?, shownIn: Locale): String? {
    val written = code?.lowercase()?.takeIf { it.isNotBlank() && it != "und" } ?: return null
    val three = TERMINOLOGY[written] ?: written
    val name = Locale.forLanguageTag(TWO_LETTERS[three] ?: three).getDisplayLanguage(shownIn)
    return name.takeIf { it.isNotBlank() && it != three }?.replaceFirstChar { it.titlecase(shownIn) }
}

/** A day as the server writes it, year, month and day. */
private val A_DAY = Regex("""^(\d{4})-(\d{2})-(\d{2})$""")

/** The whole years from one day to another, both written year, month and
 *  day: how old somebody is, or was. */
fun yearsBetween(from: String, to: String): Int? {
    val start = A_DAY.matchEntire(from)?.destructured ?: return null
    val end = A_DAY.matchEntire(to)?.destructured ?: return null
    val (fromYear, fromMonth, fromDay) = start.toList().map { it.toInt() }
    val (toYear, toMonth, toDay) = end.toList().map { it.toInt() }
    val beforeTheBirthday = toMonth < fromMonth || (toMonth == fromMonth && toDay < fromDay)
    return toYear - fromYear - if (beforeTheBirthday) 1 else 0
}

/** A day written as people of [locale] write it, its month in words. */
fun readableDay(day: String, locale: Locale): String? {
    val (year, month, date) = A_DAY.matchEntire(day)?.destructured?.toList()?.map { it.toInt() } ?: return null
    val utc = TimeZone.getTimeZone("UTC")
    val calendar = Calendar.getInstance(utc).apply {
        clear()
        set(year, month - 1, date)
    }
    return DateFormat.getDateInstance(DateFormat.LONG, locale).apply { timeZone = utc }.format(calendar.time)
}

/** Today, written year, month and day, on the reader's own calendar. */
fun todayOf(now: Calendar): String =
    String.format(Locale.ROOT, "%04d-%02d-%02d", now.get(Calendar.YEAR), now.get(Calendar.MONTH) + 1, now.get(Calendar.DAY_OF_MONTH))

/** An instant as the server writes it, to the second, in UTC. */
private val AN_INSTANT = Regex("""^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(\.\d+)?Z$""")

/** An instant the server wrote, as milliseconds since 1970; nothing for
 *  what is not one. */
fun instantOf(written: String): Long? {
    val parts = AN_INSTANT.matchEntire(written)?.groupValues?.drop(1)?.take(6)?.map { it.toInt() } ?: return null
    return Calendar.getInstance(TimeZone.getTimeZone("UTC")).apply {
        clear()
        set(parts[0], parts[1] - 1, parts[2], parts[3], parts[4], parts[5])
    }.timeInMillis
}

/** When a film would end, in milliseconds since 1970, with [remaining]
 *  seconds of it left at [speed], from [now]; nothing when it is over or
 *  not playing forward. */
fun endsAt(remaining: Double, speed: Float, now: Long): Long? =
    if (remaining > 0 && speed > 0) now + (remaining / speed * 1000).toLong() else null

private val CONTAINER_NAMES = mapOf("matroska" to "MKV", "mov" to "MP4", "mpegts" to "MPEG-TS", "mpeg" to "MPEG-PS", "asf" to "WMV")

/** A container as somebody calls it rather than as the analyser does: the
 *  first of the names it gives. */
fun containerName(format: String): String {
    val first = format.split(",").first().trim()
    return CONTAINER_NAMES[first] ?: first.uppercase()
}

/** A rate, in the unit that keeps it to a few figures; nothing for none. */
fun readableBitrate(bits: Long?): String? = when {
    bits == null || bits <= 0 -> null
    bits >= 1_000_000 -> String.format(Locale.ROOT, "%.1f Mb/s", bits / 1_000_000.0)
    else -> "${(bits / 1000.0).roundToInt()} kb/s"
}

private val SIZE_UNITS = listOf("B", "kB", "MB", "GB", "TB")

/** A size, in the unit that keeps it to a few figures. */
fun readableSize(bytes: Long): String {
    var value = bytes.toDouble()
    var unit = 0
    while (value >= 1000 && unit < SIZE_UNITS.lastIndex) {
        value /= 1000
        unit += 1
    }
    return if (unit == 0) "$bytes ${SIZE_UNITS[0]}" else String.format(Locale.ROOT, "%.1f %s", value, SIZE_UNITS[unit])
}
