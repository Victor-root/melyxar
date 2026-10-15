package app.melyxar.tv.play

/** How long the remote has to rest on the bar before the film goes where it
 *  was taken. */
const val SCRUB_SETTLES_MS = 700L

/**
 * How far one press of an arrow takes the bar, in seconds: ten at a time,
 * then further the longer the arrow is held, so the far end of a long film is
 * a few seconds away rather than a minute.
 */
fun scrubStep(repeats: Int): Int = when {
    repeats < 10 -> 10
    repeats < 30 -> 30
    else -> 60
}
