package app.melyxar.tv.look

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.edit

/**
 * Whether this television draws its screens lighter, for one whose graphics
 * chip cannot keep up with the full look: chosen in its settings alone and
 * kept on it. Lighter, the painting the account chose behind the pages is
 * left out for the page's plain ground, nothing blurred is drawn round the
 * pieces, neither shadow nor glow, the banner ends on that ground instead of
 * fading into it and turns from one work to the next at once, and pictures
 * come without fading in. Every piece is still there and works the same.
 */
object Lighter {
    private const val KEPT = "television"
    private const val LIGHTER = "drawn_lighter"

    private var kept: SharedPreferences? = null

    /** Read where it draws, so choosing it shows at once. */
    var on by mutableStateOf(false)
        private set

    fun keep(context: Context) {
        kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE).also { on = it.getBoolean(LIGHTER, false) }
    }

    fun choose(lighter: Boolean) {
        on = lighter
        kept?.edit { putBoolean(LIGHTER, lighter) }
    }
}
