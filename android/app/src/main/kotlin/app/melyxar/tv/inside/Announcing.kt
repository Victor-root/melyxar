package app.melyxar.tv.inside

/*
 * How long a song that has just started is said in the bar: long enough to
 * be read, and when its name runs along the glass, until it has run by.
 */

/** How long a name that fits the glass is said. */
const val SAID_WHOLE_MS = 5_000L

/** How long a name waits before it runs, as the running text waits, and
 *  how long it rests once it has run by. */
const val RUN_WAITS_MS = 1_200L
private const val RESTS_MS = 800L

/** The room the running text leaves between the end of the name and its
 *  start coming round, as a share of the room it runs in. */
const val RUN_GAP = 1f / 3

/** How long a name [wide] pixels wide is said in [room] pixels of glass,
 *  running at [speed] pixels a second when it does not fit: one run by,
 *  from its start round to its start again. */
fun saidForMs(wide: Int, room: Int, speed: Float): Long {
    if (wide <= room || speed <= 0f) {
        return SAID_WHOLE_MS
    }
    val run = (wide + room * RUN_GAP) / speed * 1000
    return RUN_WAITS_MS + run.toLong() + RESTS_MS
}
