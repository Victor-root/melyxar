package app.melyxar.tv.inside

import androidx.compose.runtime.MutableState
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.server.Api
import app.melyxar.server.Preferences
import app.melyxar.server.Refusal
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import java.io.IOException

/**
 * The account's preferences as every screen reads them: a change shows at
 * once everywhere, before the server answers, and goes back as it was if
 * the server refuses it.
 */
@Stable
class AccountPreferences(
    private val api: Api,
    private val scope: CoroutineScope,
    private val signedOut: () -> Unit,
    private val held: MutableState<Loaded<Preferences>>,
) {
    val now: Preferences? get() = held.value.ready

    /** Shows [shown] at once and sends [sent], named as the server names
     *  what changes. */
    fun change(shown: (Preferences) -> Preferences, sent: JsonObject) {
        val before = now ?: return
        held.value = Loaded.Ready(shown(before))
        scope.launch {
            held.value = Loaded.Ready(
                try {
                    api.changePreferences(sent)
                } catch (refused: Refusal) {
                    if (refused.signedOut) signedOut()
                    before
                } catch (_: IOException) {
                    before
                },
            )
        }
    }
}

val LocalAccountPreferences = staticCompositionLocalOf<AccountPreferences> { error("no preferences") }
