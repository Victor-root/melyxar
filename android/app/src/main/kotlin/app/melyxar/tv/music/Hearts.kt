package app.melyxar.tv.music

import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.Refusal
import app.melyxar.server.musicHearts
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import java.io.IOException

/**
 * The albums, artists and songs the account hearted, held once for every
 * screen as the web's store holds them: a press changes the heart at once
 * everywhere it shows, and puts it back if the server refuses.
 */
@Stable
class Hearts(private val api: Api, private val scope: CoroutineScope, private val signedOut: () -> Unit) {
    private val hearted = mutableStateMapOf<String, Boolean>()

    fun of(id: String): Boolean = hearted[id] == true

    /** Reads again what the server holds. */
    suspend fun read() {
        try {
            val now = api.musicHearts().toSet()
            hearted.keys.retainAll(now)
            now.forEach { hearted[it] = true }
        } catch (refused: Refusal) {
            if (refused.signedOut) signedOut()
        } catch (_: IOException) {
            // The hearts stay as they were.
        }
    }

    fun set(id: String, on: Boolean) {
        val before = of(id)
        hearted[id] = on
        scope.launch {
            try {
                api.mark(id, "favourite", on)
            } catch (refused: Refusal) {
                hearted[id] = before
                if (refused.signedOut) signedOut()
            } catch (_: IOException) {
                hearted[id] = before
            }
        }
    }
}

val LocalHearts = staticCompositionLocalOf<Hearts> { error("no hearts") }
