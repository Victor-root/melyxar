package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import app.melyxar.server.Refusal
import app.melyxar.tv.look.LocalServer
import kotlinx.coroutines.delay
import java.io.IOException

/** How long a broken line waits before it is opened again. */
private const val AGAIN_AFTER_MS = 5_000L

/**
 * The server's live line, held while Melyxar is on the screen, as the web's
 * page holds it: told that what the libraries hold moved, a scan bringing
 * films in or a film named, every screen reads again what it shows. Opened
 * again when it breaks, after which everything is read again too, since
 * nothing was said to it meanwhile.
 */
@Composable
fun FollowTheLiveLine(onLibrariesMoved: () -> Unit) {
    val api = LocalServer.current
    val signedOut = LocalSignedOut.current
    val moved by rememberUpdatedState(onLibrariesMoved)
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(api, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            // The screens read everything as they come on: only a line
            // opened again after breaking has anything to catch up on.
            var opened = false
            while (true) {
                try {
                    api.live().collect { word ->
                        when (word) {
                            "open" -> if (opened) moved() else opened = true
                            "libraries" -> moved()
                        }
                    }
                } catch (refused: Refusal) {
                    if (refused.signedOut) {
                        signedOut()
                        return@repeatOnLifecycle
                    }
                } catch (_: IOException) {
                    // Opened again below.
                }
                delay(AGAIN_AFTER_MS)
            }
        }
    }
}
