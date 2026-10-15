package app.melyxar.tv.music

import app.melyxar.server.Song
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

/*
 * How loud each song is played and how one gives way to the next, by the
 * web player's own rules.
 */

/** The level every song is brought to, as the services people listen on
 *  bring theirs. */
private const val TARGET_LUFS = -14.0

/** Room left under full scale once a song is raised, so its loudest moment
 *  never clips. */
private const val HEADROOM_DB = 1.0

/** Quieter than this is silence, or a measure gone wrong: left alone. */
private const val TOO_QUIET_LUFS = -60.0

/** The gain, as a factor on the sound, for [song] in [mode]: off, track or
 *  album. A song never measured is left as it is, and a song is raised only
 *  as far as its loudest moment allows. */
fun levelOf(song: Song, mode: String): Float {
    if (mode == "off") {
        return 1f
    }
    val loudness = (if (mode == "album") song.albumLufs ?: song.lufs else song.lufs)
        ?.takeIf { it >= TOO_QUIET_LUFS }
        ?: return 1f
    var gainDb = TARGET_LUFS - loudness
    if (gainDb > 0) {
        gainDb = song.peakDbfs?.let { min(gainDb, max(0.0, -HEADROOM_DB - it)) } ?: 0.0
    }
    return 10.0.pow(gainDb / 20).toFloat()
}

/** The crossfade a song can take: never more than a third of either song,
 *  so a short one is never all fade. */
fun fadeBetween(crossfadeSeconds: Double, fromSeconds: Double?, toSeconds: Double?): Double {
    val shortest = min(fromSeconds ?: Double.POSITIVE_INFINITY, toSeconds ?: Double.POSITIVE_INFINITY)
    return max(0.0, min(crossfadeSeconds, shortest / 3))
}

/** Whether a song heard [seconds] in counts as listened to: halfway through,
 *  or four minutes in for a long one, and never for one under half a minute. */
fun countsAsListened(seconds: Double, length: Double?): Boolean {
    val whole = length ?: return false
    return whole >= SHORTEST_COUNTED_S && seconds >= min(whole / 2, LONGEST_WAIT_S)
}

private const val SHORTEST_COUNTED_S = 30.0
private const val LONGEST_WAIT_S = 240.0
