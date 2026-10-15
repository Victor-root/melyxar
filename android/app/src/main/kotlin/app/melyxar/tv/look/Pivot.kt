package app.melyxar.tv.look

import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.tween
import androidx.compose.foundation.gestures.BringIntoViewSpec

/**
 * Where a row or a page scrolls the piece the remote comes to: always to the
 * same place, [lead] points plus a [share] of the way in, as every
 * television interface does, rather than just inside the nearest edge, where
 * the eye would have to chase it.
 */
class Pivot(private val lead: Float, private val share: Float = 0f) : BringIntoViewSpec {
    override val scrollAnimationSpec: AnimationSpec<Float> = tween(360, easing = Ease)

    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float =
        offset - (lead + containerSize * share)
}
