package app.melyxar.tv.look

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.edit

/**
 * Whether this television draws its screens lighter, for a box that cannot
 * keep up with the full look, whatever the look loses: chosen in its
 * settings alone and kept on it.
 *
 * Lighter, what the remote does no longer runs through frames: it lights a
 * piece, a row or a page goes to that piece, and what stands over a film or
 * a photo comes and goes, each at once. The painting the account chose
 * behind the pages is left out for the page's plain ground, and nothing is
 * drawn round the pieces, neither shadow nor glow nor crisp edge. The banner
 * ends on that ground instead of fading into it, and turns from one work to
 * the next at once and only when the remote turns it. The tiles of the
 * libraries lose their pools of light, the sheen of their posters and the
 * floor they are reflected in. Pictures are fetched a size smaller and come
 * without fading in, and the probe no longer reads the main thread after
 * each key. Every piece is still there and answers the remote the same.
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
