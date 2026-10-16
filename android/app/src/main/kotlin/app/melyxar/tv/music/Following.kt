package app.melyxar.tv.music

import app.melyxar.server.LyricLine
import app.melyxar.server.SongSpectrum
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.min

/*
 * What the page of the song playing follows as the song plays, by the web
 * player's own sums: the line of its words being sung, and the wave of its
 * sound behind the player.
 */

/** How far ahead of the clock a line is lit: a line lit a moment late reads
 *  as one lit wrong. */
private const val LIT_AHEAD_MS = 250

/** The last line whose moment has come, or -1 before the first. The lines
 *  are in the order they are sung, so the search halves the list each step. */
fun lineAt(lines: List<LyricLine>, positionMs: Long): Int {
    var low = 0
    var high = lines.size - 1
    var found = -1
    while (low <= high) {
        val middle = (low + high) ushr 1
        if (lines[middle].atMs <= positionMs) {
            found = middle
            low = middle + 1
        } else {
            high = middle - 1
        }
    }
    return found
}

/** The line lit when the song is [seconds] in. */
fun litLineAt(lines: List<LyricLine>, seconds: Double): Int = lineAt(lines, (seconds * 1000).toLong() + LIT_AHEAD_MS)

/** How much of the way to its target a level of the wave goes in one
 *  drawing, and below what it is taken for settled. */
private const val GLIDE = 0.22f
private const val SETTLED = 0.004f

/**
 * The levels of the wave at this moment of the song, each from nought to
 * one, between the two readings it falls between. A reading stands for the
 * stretch it was made over, so it is read at the middle of that stretch;
 * before the first and after the last, the nearest holds.
 */
fun levelsAt(spectrum: SongSpectrum, seconds: Double, into: FloatArray) {
    val frames = spectrum.levels.size / spectrum.bands
    val at = (seconds * spectrum.framesASecond - 0.5).coerceIn(0.0, (frames - 1).toDouble())
    val before = floor(at).toInt()
    val after = min(frames - 1, before + 1)
    val share = (at - before).toFloat()
    for (band in 0 until spectrum.bands) {
        val first = spectrum.levels[before * spectrum.bands + band].toInt() and 0xFF
        val second = spectrum.levels[after * spectrum.bands + band].toInt() and 0xFF
        into[band] = (first + (second - first) * share) / 255f
    }
}

/** One step of every level towards its target, and whether all have
 *  settled there. */
fun approach(levels: FloatArray, targets: FloatArray): Boolean {
    var settled = true
    for (band in levels.indices) {
        levels[band] += (targets[band] - levels[band]) * GLIDE
        if (abs(targets[band] - levels[band]) > SETTLED) {
            settled = false
        }
    }
    return settled
}

/** The longest the wave runs on past what the player last said: it says
 *  where the song is four times a second, so twice that is a player gone
 *  quiet, and the wave waits for it. */
private const val RUNS_ON_AT_MOST_MS = 500L

/** Where the song is, to the frame, from where the player last said it was
 *  and how long ago it said it. */
fun momentOf(position: Double, sinceItWasSaidMs: Long): Double =
    position + sinceItWasSaidMs.coerceIn(0L, RUNS_ON_AT_MOST_MS) / 1000.0

/** How many bars the mark of the song playing has. */
const val BARS = 4

/** The heights of the bars of the mark of the song playing, from the
 *  levels of the wave: the bands shared out between them in order, the lows
 *  first, each the loudest of its share, so whatever stands out of the song
 *  lifts its bar. */
fun barsOf(levels: FloatArray, into: FloatArray) {
    for (bar in 0 until BARS) {
        into[bar] = 0f
        if (levels.isEmpty()) {
            continue
        }
        val first = bar * levels.size / BARS
        val last = maxOf(first + 1, (bar + 1) * levels.size / BARS)
        for (band in first until min(last, levels.size)) {
            into[bar] = maxOf(into[bar], levels[band])
        }
    }
}

/** How long a bar takes to forget the loudest and the quietest it has seen,
 *  which is what it is spread between, and the least it is spread over, so
 *  a song holding still does not make noise of nothing. */
private const val GAIN_MEMORY_MS = 4_000f
private const val LEAST_SPREAD = 0.12f

/** How much of the way up a bar goes in one step, and how fast it falls, in
 *  heights a second: up at once, down slowly, as a level meter does; and
 *  the lowest it stands, never quite flat. */
private const val ATTACK = 0.7f
private const val FALL_A_SECOND = 1.6f
private const val FLOOR = 0.15f

/**
 * The bars of the mark of the song playing as they move: each spread between
 * the loudest and the quietest of its own recent seconds, so a song that
 * keeps to a narrow range still moves them all the way, then jumping up at
 * once and falling back slowly.
 */
class BarMeter {
    private val low = FloatArray(BARS) { 1f }
    private val high = FloatArray(BARS)

    /** The heights shown now, from nought to one. */
    val shown = FloatArray(BARS) { FLOOR }

    /** One step, from the bars read from the wave and the time since the
     *  last step. */
    fun step(read: FloatArray, sinceMs: Long) {
        val forget = min(1f, sinceMs / GAIN_MEMORY_MS)
        for (bar in 0 until BARS) {
            val level = read[bar]
            high[bar] = if (level > high[bar]) level else high[bar] + (level - high[bar]) * forget
            low[bar] = if (level < low[bar]) level else low[bar] + (level - low[bar]) * forget
            val spread = maxOf(high[bar] - low[bar], LEAST_SPREAD)
            val wanted = ((level - low[bar]) / spread).coerceIn(FLOOR, 1f)
            val now = shown[bar]
            shown[bar] = if (wanted > now) now + (wanted - now) * ATTACK else maxOf(wanted, now - FALL_A_SECOND * sinceMs / 1000f)
        }
    }
}

/** The beats the bars keep without a wave to follow, as the web's: each
 *  its own length and its own start, going there and back. */
private val OWN_BEAT_MS = longArrayOf(900, 640, 1_100, 760)
private val OWN_BEAT_AHEAD_MS = longArrayOf(0, 300, 520, 180)
private val BEAT_AT = floatArrayOf(0f, 0.4f, 0.7f, 1f)
private val BEAT_HEIGHT = floatArrayOf(0.25f, 1f, 0.55f, 0.85f)

/** How high [bar] stands [ms] into its own beats, from nought to one. */
fun ownBeat(bar: Int, ms: Long): Float {
    val length = OWN_BEAT_MS[bar]
    val at = ms + OWN_BEAT_AHEAD_MS[bar]
    val there = (at % length).toFloat() / length
    val way = if ((at / length) % 2 == 0L) there else 1 - there
    val step = (1 until BEAT_AT.size).first { way <= BEAT_AT[it] }
    val within = (way - BEAT_AT[step - 1]) / (BEAT_AT[step] - BEAT_AT[step - 1])
    val eased = within * within * (3 - 2 * within)
    return BEAT_HEIGHT[step - 1] + (BEAT_HEIGHT[step] - BEAT_HEIGHT[step - 1]) * eased
}
