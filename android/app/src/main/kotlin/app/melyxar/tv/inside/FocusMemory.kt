package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged

/**
 * Which piece of a screen the remote was last on, so coming back to the
 * screen puts it there again rather than at the top: a step back undoes the
 * step, and nothing more.
 */
@Stable
class FocusMemory(var last: String?) {
    private val requesters = HashMap<String, FocusRequester>()

    fun requester(key: String): FocusRequester = requesters.getOrPut(key) { FocusRequester() }

    /** Puts the remote back where it was; false when that piece is not
     *  there any more, or was never set. */
    fun restore(): Boolean {
        val requester = last?.let { requesters[it] } ?: return false
        return try {
            requester.requestFocus()
        } catch (_: IllegalStateException) {
            false
        }
    }
}

@Composable
fun rememberFocusMemory(): FocusMemory =
    rememberSaveable(saver = Saver<FocusMemory, String>({ it.last }, { FocusMemory(it) })) { FocusMemory(null) }

/** A piece the screen remembers the remote on, under a key unique on it. */
fun Modifier.remembered(memory: FocusMemory, key: String): Modifier = this
    .focusRequester(memory.requester(key))
    .onFocusChanged { if (it.isFocused) memory.last = key }

/**
 * Once the screen is drawn, puts the remote back on its last piece, or on
 * [first] when the screen is new or that piece has gone, or on [orElse] when
 * [first] cannot take it.
 */
@Composable
fun RestoreFocus(memory: FocusMemory, first: FocusRequester, ready: Boolean, orElse: FocusRequester? = null) {
    LaunchedEffect(ready) {
        if (!ready) {
            return@LaunchedEffect
        }
        // The rows are laid out a frame after the screen is composed.
        withFrameNanos {}
        val restored = memory.restore()
        val onFirst = !restored && takes(first)
        val elsewhere = !restored && !onFirst && orElse?.let(::takes) == true
        focusSaid { "screen ready: back on ${memory.last} $restored, on its first $onFirst, elsewhere $elsewhere" }
    }
}

/** Puts the remote on [requester]; false when nothing there can take it. */
private fun takes(requester: FocusRequester): Boolean = try {
    requester.requestFocus(FocusDirection.Enter)
} catch (_: IllegalStateException) {
    // Nothing focusable came: the screen shows nothing to choose.
    false
}
