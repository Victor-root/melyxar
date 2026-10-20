package app.melyxar.tv.live

import android.content.Context
import androidx.core.content.edit
import kotlin.math.min
import kotlin.math.roundToInt

/** How many presses take a picture from the middle to the edge of the screen. */
const val PLACEMENT_STEPS = 20

/**
 * Where a camera's picture is moved within the black bands around it, in
 * presses from the middle on each axis: a negative [across] is to the left,
 * a negative [down] is up. At [PLACEMENT_STEPS] the picture touches that edge
 * of the screen, so it is never cut.
 */
data class Placement(val across: Int = 0, val down: Int = 0) {
    val centred: Boolean get() = across == 0 && down == 0

    fun moved(right: Int, lower: Int) = Placement(
        (across + right).coerceIn(-PLACEMENT_STEPS, PLACEMENT_STEPS),
        (down + lower).coerceIn(-PLACEMENT_STEPS, PLACEMENT_STEPS),
    )

    /** How far the picture is moved, in pixels, on a screen leaving [room]
     *  on each side of it. */
    fun offset(room: Room): Pair<Int, Int> =
        (room.across * across / PLACEMENT_STEPS).roundToInt() to (room.down * down / PLACEMENT_STEPS).roundToInt()
}

/** The black band on each side of a picture fitted in a screen, in pixels. */
data class Room(val across: Float, val down: Float)

/**
 * The room left around a picture of [pictureWidth] by [pictureHeight], its
 * pixels [pixelRatio] times as wide as high, fitted whole in a screen of
 * [screenWidth] by [screenHeight].
 */
fun roomAround(screenWidth: Int, screenHeight: Int, pictureWidth: Int, pictureHeight: Int, pixelRatio: Float): Room {
    if (screenWidth <= 0 || screenHeight <= 0 || pictureWidth <= 0 || pictureHeight <= 0) return Room(0f, 0f)
    val wide = pictureWidth * pixelRatio
    val scale = min(screenWidth / wide, screenHeight / pictureHeight.toFloat())
    return Room((screenWidth - wide * scale) / 2, (screenHeight - pictureHeight * scale) / 2)
}

/** Where each camera's picture was moved, kept on this device alone: it is
 *  this screen's bands that are being filled, not the camera's. */
class KeptPlacement(context: Context) {
    private val kept = context.getSharedPreferences("cameras", Context.MODE_PRIVATE)

    fun of(camera: String) = Placement(kept.getInt("$camera:across", 0), kept.getInt("$camera:down", 0)).moved(0, 0)

    fun keep(camera: String, placement: Placement) = kept.edit {
        putInt("$camera:across", placement.across)
        putInt("$camera:down", placement.down)
    }
}
