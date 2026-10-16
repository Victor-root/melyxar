package app.melyxar.tv.play

/*
 * How far there is left to wait for a film the server rebuilds, as one
 * number, and only from what has been measured, as the web player counts it
 * (player/loading.ts): the server writing the first piece of film, the few
 * real events around it each moving the number a little.
 */

/** Where the wait has got, in the order it goes through. */
enum class LoadingStage(
    /** Where the stage puts the number, at the least. */
    val floor: Float,
) {
    Opening(0f),
    SessionOpened(3f),
    ManifestParsed(6f),
    Producing(WRITING_FROM),
    Produced(CARRYING_FROM),
    Done(100f),
}

/** The stretch the server writing the first piece fills. */
private const val WRITING_FROM = 8f

/** Where the first piece written leaves the number until the film plays. */
private const val CARRYING_FROM = 85f

/** How much of the way to what was measured one step covers. */
private const val GLIDE = 0.3f

/** The number what is known is worth: the stage's floor, or how much of the
 *  first piece the server has [written], from nought to one. */
fun percentOf(stage: LoadingStage, written: Float): Float =
    maxOf(stage.floor, if (written > 0) WRITING_FROM + (CARRYING_FROM - WRITING_FROM) * written.coerceIn(0f, 1f) else 0f)

/** One step of the number shown toward what was measured, never back. */
fun glideToward(shown: Float, measured: Float): Float {
    if (measured <= shown) {
        return shown
    }
    val next = shown + (measured - shown) * GLIDE
    return if (measured - next < 0.2f) measured else next
}
