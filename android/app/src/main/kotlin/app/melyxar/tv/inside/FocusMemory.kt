package app.melyxar.tv.inside

import androidx.compose.foundation.focusGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.FocusState
import androidx.compose.ui.focus.focusProperties
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
    private val noters = HashMap<String, (FocusState) -> Unit>()

    fun requester(key: String): FocusRequester = requesters.getOrPut(key) { FocusRequester() }

    /** What notes the remote coming to [key], the same each time, so the
     *  piece's modifier is too and it is not composed again for it. */
    fun noter(key: String): (FocusState) -> Unit = noters.getOrPut(key) { { if (it.isFocused) last = key } }

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
    .onFocusChanged(memory.noter(key))

/**
 * Which piece of a row the remote was last on. Coming into the row with the
 * remote's arrows lands there again, or on its first piece; a piece put back
 * by name, as a screen coming back puts its last one, is let be, where the
 * system's own restorer sends it to the first piece it knows of.
 */
@Stable
class RowMemory {
    private var last: Any? = null
    private val requesters = HashMap<Any, FocusRequester>()
    private val noters = HashMap<Any, (FocusState) -> Unit>()

    fun requester(key: Any): FocusRequester = requesters.getOrPut(key) { FocusRequester() }

    fun noter(key: Any): (FocusState) -> Unit = noters.getOrPut(key) { { if (it.hasFocus) last = key } }

    /** Puts the remote on the piece it left, or on [first]; false when
     *  neither is there. */
    fun land(first: Any?): Boolean = listOfNotNull(last, first).any { key -> requesters[key]?.let(::takes) == true }
}

/** The row holding the pieces of [row], [first] the key of its first. */
fun Modifier.landsAsLeft(row: RowMemory, first: Any?): Modifier = this
    .focusProperties {
        onEnter = {
            if (requestedFocusDirection != FocusDirection.Enter) {
                row.land(first)
            }
        }
    }
    .focusGroup()

/** A piece of [row], under a [key] unique in it. */
fun Modifier.inRow(row: RowMemory, key: Any): Modifier = this
    .focusRequester(row.requester(key))
    .onFocusChanged(row.noter(key))

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
