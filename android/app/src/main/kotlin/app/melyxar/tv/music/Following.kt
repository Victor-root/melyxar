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
