package app.melyxar.tv.look

import androidx.compose.foundation.gestures.BringIntoViewSpec

/**
 * Where a row or a page scrolls the piece the remote comes to: always to the
 * same place, [lead] points plus a [share] of the way in, as every
 * television interface does, rather than just inside the nearest edge, where
 * the eye would have to chase it. Nowhere while [stays] says the remote is in
 * a part of the page that is shown whole.
 */
class Pivot(
    private val lead: Float,
    private val share: Float = 0f,
    private val stays: () -> Boolean = { false },
) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float =
        if (stays()) 0f else offset - (lead + containerSize * share)
}

/**
 * Where a row scrolls the piece the remote comes to when the row is short
 * enough to be read at a glance: only as far as it takes to show it whole,
 * [margin] points from the edge, and not at all while it is.
 */
class Nearest(private val margin: Float) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float {
        val before = offset - margin
        val after = offset + size - (containerSize - margin)
        return when {
            before < 0f -> before
            after > 0f -> minOf(after, before)
            else -> 0f
        }
    }
}
