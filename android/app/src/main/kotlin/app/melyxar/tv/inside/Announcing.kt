package app.melyxar.tv.inside

/*
 * How long a song that has just started is said in the bar: long enough to
 * be read, and when its name is longer than the glass, until it has run
 * along to its end and that end has been read.
 */

/** How long a name that fits the glass is said. */
const val SAID_WHOLE_MS = 5_000L

/** How long a name waits before it runs, and how long its end stays once it
 *  has run to it. */
const val RUN_WAITS_MS = 1_200L
private const val END_STAYS_MS = 1_500L

/** How long a name running [beyond] pixels past the glass is said, running
 *  at [speed] pixels a second: the wait, the run to its end, and its end
 *  read. */
fun saidForMs(beyond: Int, speed: Float): Long {
    if (beyond <= 0 || speed <= 0f) {
        return SAID_WHOLE_MS
    }
    return RUN_WAITS_MS + runForMs(beyond, speed) + END_STAYS_MS
}

/** How long a name takes to run [beyond] pixels at [speed] pixels a second. */
fun runForMs(beyond: Int, speed: Float): Long = (beyond / speed * 1000).toLong()
