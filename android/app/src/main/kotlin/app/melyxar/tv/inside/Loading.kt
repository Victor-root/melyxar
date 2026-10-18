package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.Refusal
import app.melyxar.tv.look.LocalServer
import java.io.IOException

/** What a screen asked the server, while it waits and once it is answered. */
sealed interface Loaded<out T> {
    data object Waiting : Loaded<Nothing>

    data object Failed : Loaded<Nothing>

    data class Ready<T>(val value: T) : Loaded<T>
}

val <T> Loaded<T>.ready: T? get() = (this as? Loaded.Ready<T>)?.value

/** Told when the account's session has ended, which sends the television
 *  back to its door. */
val LocalSignedOut = staticCompositionLocalOf<() -> Unit> { {} }

/** The last answer to each question, kept while the screen that asked it is
 *  under another: going back shows it at once, where it was left, while it
 *  is asked again. */
val LocalAnswers = staticCompositionLocalOf<MutableMap<String, Any?>> { HashMap() }

/**
 * Asks the server [question], named so its last answer is shown straight
 * away when the screen comes back, and asks again whenever the screen comes
 * on, one of [keys] changes, or, unless it does not [followsRows], what the
 * server's rows hold moved.
 */
@Composable
fun <T> rememberLoaded(question: String, vararg keys: Any?, followsRows: Boolean = true, ask: suspend Api.() -> T): MutableState<Loaded<T>> {
    val api = LocalServer.current
    val signedOut = LocalSignedOut.current
    val answers = LocalAnswers.current
    val moved = if (followsRows) LocalMarks.current.rowsMoved else 0
    val loaded = remember(question) {
        @Suppress("UNCHECKED_CAST")
        val kept = answers[question] as Loaded.Ready<T>?
        mutableStateOf<Loaded<T>>(kept ?: Loaded.Waiting)
    }
    LaunchedEffect(api, question, moved, *keys) {
        try {
            val answer = Loaded.Ready(api.ask())
            answers[question] = answer
            loaded.value = answer
        } catch (refused: Refusal) {
            if (refused.signedOut) {
                signedOut()
            } else if (loaded.value !is Loaded.Ready) {
                loaded.value = Loaded.Failed
            }
        } catch (_: IOException) {
            if (loaded.value !is Loaded.Ready) {
                loaded.value = Loaded.Failed
            }
        }
    }
    return loaded
}
