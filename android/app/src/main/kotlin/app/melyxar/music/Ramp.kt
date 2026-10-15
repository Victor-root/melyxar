package app.melyxar.music

/**
 * A factor on the sound that moves to where it is asked, a frame at a time,
 * in a straight line over as many frames as it is given: the level of a song
 * settling, or a whole fade.
 */
class Ramp(start: Float = 1f) {
    var value = start
        private set
    private var target = start
    private var step = 0f
    private var framesLeft = 0

    /** Towards `goal` over `frames` frames, or there at once with none. */
    fun aim(goal: Float, frames: Int) {
        target = goal
        if (frames <= 0) {
            value = goal
            framesLeft = 0
            step = 0f
            return
        }
        framesLeft = frames
        step = (goal - value) / frames
    }

    /** The factor for the next frame. */
    fun next(): Float {
        if (framesLeft > 0) {
            framesLeft -= 1
            value = if (framesLeft == 0) target else value + step
        }
        return value
    }

    /** Whether it stands still, at one, and there is nothing to do. */
    val untouched: Boolean
        get() = framesLeft == 0 && value == 1f
}
