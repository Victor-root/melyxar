package app.melyxar.tv.inside

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.tv.door.CodePad
import app.melyxar.tv.door.DoorPanel
import app.melyxar.tv.door.pressed
import app.melyxar.tv.door.shakeTheHead
import app.melyxar.tv.door.whole
import app.melyxar.tv.look.LocalPalette
import kotlinx.coroutines.launch

/**
 * A new code for this account's televisions, typed twice on the pad the
 * door asks it with, so a slip of the remote is never the code kept.
 * [onChosen] with the four digits once both agree.
 */
@Composable
fun NewTvCode(onChosen: (String) -> Unit, onClose: () -> Unit) {
    val palette = LocalPalette.current
    var first by remember { mutableStateOf<String?>(null) }
    var typed by remember { mutableStateOf("") }
    var refusal by remember { mutableStateOf<String?>(null) }
    val shake = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val mismatch = stringResource(R.string.tv_tv_code_mismatch)
    BackHandler(onBack = onClose)

    fun press(key: String) {
        refusal = null
        typed = pressed(typed, key)
        if (!whole(typed)) {
            return
        }
        val before = first
        when {
            before == null -> first = typed
            before == typed -> onChosen(typed)
            else -> {
                first = null
                refusal = mismatch
                scope.launch { shake.shakeTheHead() }
            }
        }
        typed = ""
    }

    DoorPanel(shake) {
        Text(
            stringResource(if (first == null) R.string.tv_tv_code_new else R.string.tv_tv_code_again),
            fontSize = 19.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            textAlign = TextAlign.Center,
        )
        // Drawn afresh for the second time, so the remote starts again on the
        // pad's first key.
        key(first == null) {
            CodePad(typed, refusal, ::press)
        }
    }
}
