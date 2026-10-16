package app.melyxar.tv

import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import app.melyxar.R

/** How soon the back key must be pressed again to leave. */
private const val AGAIN_WITHIN_MS = 2_000L

/** Whether a press of the back key at [now] follows the one at [before]
 *  closely enough to leave. */
fun leavesNow(before: Long?, now: Long): Boolean = before != null && now - before <= AGAIN_WITHIN_MS

/**
 * Leaving the application from a screen with nothing behind it: the back
 * key pressed once says to press it again, and pressed again soon after
 * leaves, so a press too many never quits by surprise.
 */
@Composable
fun rememberLeaving(onLeave: () -> Unit): () -> Unit {
    val context = LocalContext.current
    return remember(context, onLeave) {
        var before: Long? = null
        var told: Toast? = null
        {
            val now = System.currentTimeMillis()
            if (leavesNow(before, now)) {
                told?.cancel()
                onLeave()
            } else {
                before = now
                told = Toast.makeText(context, R.string.tv_back_again_to_leave, Toast.LENGTH_SHORT).also { it.show() }
            }
        }
    }
}
