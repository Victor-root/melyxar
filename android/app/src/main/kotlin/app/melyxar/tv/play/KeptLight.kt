package app.melyxar.tv.play

import android.content.Context
import androidx.core.content.edit
import app.melyxar.player.LIGHTS

/** How bright and how contrasted this television shows a picture in
 *  standard range, kept on it alone: it is the screen that is too dark, not
 *  the film nor whoever watches it. */
class KeptLight(context: Context) {
    private val kept = context.getSharedPreferences("player", Context.MODE_PRIVATE)

    fun brightness(): Float = read(BRIGHTNESS)

    fun contrast(): Float = read(CONTRAST)

    fun keep(brightness: Float, contrast: Float) = kept.edit {
        putFloat(BRIGHTNESS, brightness)
        putFloat(CONTRAST, contrast)
    }

    /** A step kept, or the picture as it comes for anything else. */
    private fun read(name: String): Float = kept.getFloat(name, 1f).takeIf { it in LIGHTS } ?: 1f

    private companion object {
        const val BRIGHTNESS = "brightness"
        const val CONTRAST = "contrast"
    }
}
