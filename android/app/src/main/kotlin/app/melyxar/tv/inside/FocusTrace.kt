package app.melyxar.tv.inside

import android.util.Log
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import app.melyxar.BuildConfig

/*
 * Where the remote goes, said in the log of the debug build only, to follow
 * a remote that stops where it should not.
 */

const val FOCUS_LOG = "MelyxarFocus"

/** Says in the log as the remote comes into [name] or leaves it. */
fun Modifier.focusTraced(name: String): Modifier =
    if (BuildConfig.DEBUG) onFocusChanged { Log.d(FOCUS_LOG, "$name: ${if (it.hasFocus) "holds" else "without"} the remote") } else this

/** Says [what] in the log, written only in the debug build. */
inline fun focusSaid(what: () -> String) {
    if (BuildConfig.DEBUG) {
        Log.d(FOCUS_LOG, what())
    }
}
