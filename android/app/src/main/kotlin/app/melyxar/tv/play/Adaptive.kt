package app.melyxar.tv.play

import kotlin.math.min
import kotlin.math.pow

/*
 * How the automatic quality picks its rung, and when it moves to another, as
 * the web player's adaptive.ts decides it: the connection measured, the best
 * rung that fits in part of it, down as soon as the film is in danger and up
 * only once the connection has proved itself, never for a rung that gives the
 * same film. Every move opens the film again where it stands.
 */

/** The rungs the automatic quality moves between, lightest first. */
val ADAPTIVE_RUNGS: List<Quality> =
    QUALITIES.filter { it.bitrate != null }.reversed().map { it.copy(key = "adaptive:${it.key}") } +
        listOf(
            // Above the ladder, the film at its own size with a rate a fast
            // home connection carries, and past that the film as it is.
            Quality("adaptive:40", null, 40_000_000),
            Quality("adaptive:as-it-is", null, null),
        )

private val AS_IT_IS_RUNG = ADAPTIVE_RUNGS.lastIndex

/** What the connection has to carry for the film as it is to be offered. */
private const val AS_IT_IS_NEEDS = 80_000_000.0

/** The part of the measured speed a rung may take. */
private const val SHARE = 0.7

/** The rung taken when the connection could not be measured. */
val UNMEASURED_RUNG = ADAPTIVE_RUNGS.indexOfFirst { it.bitrate == 4_000_000L }

private fun needs(rung: Int): Double =
    if (rung == AS_IT_IS_RUNG) AS_IT_IS_NEEDS else ADAPTIVE_RUNGS[rung].bitrate?.toDouble() ?: AS_IT_IS_NEEDS

/** The best rung a connection this fast carries, the lightest when none fits. */
fun rungFor(bitsPerSecond: Double): Int =
    ADAPTIVE_RUNGS.indices.lastOrNull { needs(it) <= bitsPerSecond * SHARE } ?: 0

/**
 * What the connection carries, followed as segments arrive: a quick average
 * and a slow one, the lower believed, each measure weighing as long as it
 * took, so one segment that crawled in is believed at once.
 */
data class Speed(val quick: Double, val slow: Double) {
    val believed: Double get() = min(quick, slow)
}

private const val QUICK_HALF_S = 2.0
private const val SLOW_HALF_S = 5.0

/** The speed once one more measure of it is in, taken over [seconds]. */
fun Speed?.withMeasure(bitsPerSecond: Double, seconds: Double): Speed {
    this ?: return Speed(bitsPerSecond, bitsPerSecond)
    fun moved(half: Double) = 1 - 0.5.pow(seconds / half)
    return Speed(
        quick = quick + moved(QUICK_HALF_S) * (bitsPerSecond - quick),
        slow = slow + moved(SLOW_HALF_S) * (bitsPerSecond - slow),
    )
}

/** What the film being played is written at, as far as it is known. */
data class Film(val bitrate: Long?, val height: Int?)

/** Whether a rung lets the film through as it is, which two rungs that both
 *  do give the same film. */
fun letsThrough(film: Film?, rung: Int): Boolean {
    val (_, height, bitrate) = ADAPTIVE_RUNGS[rung]
    bitrate ?: return true
    film ?: return false
    val filmRate = film.bitrate ?: return false
    return filmRate <= bitrate && (height == null || (film.height != null && film.height <= height))
}

/** Where the automatic quality stands, and what it has been through since it
 *  last moved, times in milliseconds. */
data class Course(
    val rung: Int,
    val movedAt: Long,
    val loweredAt: Long? = null,
    val measured: Int = 0,
    val stalls: List<Long> = emptyList(),
)

/** Why the rung moved, as the journal words it. */
enum class Because(val word: String) {
    Stalls("stalls"),
    SlowerLink("slower_link"),
    FasterLink("faster_link"),
}

data class Move(val rung: Int, val because: Because)

private const val STALLS_WITHIN_MS = 60_000L
private const val STALLS_TOO_MANY = 2
private const val MEASURED_BEFORE_MOVING = 3
private const val HELD_LOW_S = 15.0
private const val KEPT_BEFORE_RISING_MS = 30_000L
private const val KEPT_AFTER_LOWERING_MS = 120_000L
private const val HELD_TO_RISE_S = 10.0

/** The highest rung under [rung] that changes the film. */
private fun lighterThan(film: Film?, rung: Int): Int {
    var lighter = rung - 1
    while (lighter > 0 && letsThrough(film, rung) && letsThrough(film, lighter)) lighter -= 1
    return lighter
}

/**
 * Where the rung goes now, or nothing when it stays. [speed] is what the
 * connection is believed to carry, nothing while none was measured;
 * [heldAhead] how many seconds of film the player holds past where it plays.
 */
fun nextMove(course: Course, speed: Double?, heldAhead: Double, now: Long, film: Film?): Move? {
    val recentStalls = course.stalls.count { now - it <= STALLS_WITHIN_MS }
    val fits = speed?.let(::rungFor)
    if (recentStalls >= STALLS_TOO_MANY && course.rung > 0) {
        return Move(min(lighterThan(film, course.rung), fits ?: course.rung), Because.Stalls)
    }
    if (fits == null || course.measured < MEASURED_BEFORE_MOVING) return null
    if (fits < course.rung && heldAhead < HELD_LOW_S && !letsThrough(film, fits)) {
        return Move(fits, Because.SlowerLink)
    }
    val settled = now - course.movedAt >= KEPT_BEFORE_RISING_MS &&
        (course.loweredAt == null || now - course.loweredAt >= KEPT_AFTER_LOWERING_MS)
    if (fits > course.rung && settled && heldAhead >= HELD_TO_RISE_S && !letsThrough(film, course.rung)) {
        return Move(fits, Because.FasterLink)
    }
    return null
}

/** The course once the rung has moved. */
fun Course.afterMoving(move: Move, now: Long) = Course(
    rung = move.rung,
    movedAt = now,
    loweredAt = if (move.rung < rung) now else loweredAt,
)
