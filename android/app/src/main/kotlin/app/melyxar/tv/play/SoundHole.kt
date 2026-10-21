package app.melyxar.tv.play

/**
 * A film whose sound stops for a while in the file itself: the player has
 * nothing to play there and stands still at it, picture held ahead and
 * nothing moving, as the web's `sound-hole.ts` sees it in a browser. Seen
 * standing still that way for long enough, the server is asked where the
 * sound comes back and the film goes on from there.
 */

/** How long the film stands still before it is helped. */
const val STILL_FOR_MS = 2_000L

/** How much film must be held past where it stands: without it the film is
 *  simply waiting for the network. */
const val HELD_AHEAD_MS = 1_000L

/** Where the film was last seen, and since when it has stood there. */
data class StillWatch(val at: Long, val since: Long)

/** Where the film now stands as [watch], and whether it is stuck at a hole. */
data class StillSeen(val watch: StillWatch, val stuck: Boolean)

/** Takes in where the film stands [now], with [heldAhead] of it held past. */
fun watchStill(watch: StillWatch?, at: Long, now: Long, heldAhead: Long): StillSeen =
    if (watch == null || watch.at != at) {
        StillSeen(StillWatch(at, now), stuck = false)
    } else {
        StillSeen(watch, stuck = now - watch.since >= STILL_FOR_MS && heldAhead > HELD_AHEAD_MS)
    }
