package app.melyxar.tv.look

import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalDensity

/**
 * Where a row or a page scrolls the piece the remote comes to, once it has
 * to move at all: always to the same place, [lead] points plus a [share] of
 * the way in, as every television interface does, rather than just inside
 * the nearest edge, where the eye would have to chase it. Nowhere while the
 * piece is already on the screen whole, nor while [stays] says the remote is
 * in a part of the page that is shown whole.
 */
class Pivot(
    private val lead: Float,
    private val share: Float = 0f,
    private val stays: () -> Boolean = { false },
) : BringIntoViewSpec {
    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float = when {
        stays() -> 0f
        // Already on the screen whole: nothing to bring into view.
        offset >= 0f && offset + size <= containerSize -> 0f
        else -> offset - (lead + containerSize * share)
    }
}

/**
 * Where a row scrolls the piece the remote comes to: only as far as it takes
 * to show it whole, [margin] points from the edge, and not at all while it
 * is, so a row only moves once the remote reaches its end.
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

/** How every row of the inside scrolls, keeping the page's margin. */
@Composable
fun rememberRowScrolling(): Nearest {
    val margin = with(LocalDensity.current) { Gutter.toPx() }
    return remember(margin) { Nearest(margin) }
}
