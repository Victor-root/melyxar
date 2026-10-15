package app.melyxar.tv.look

import android.util.Log
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.max
import app.melyxar.BuildConfig

/** How much of the screen a television's keyboard takes, for the keyboards
 *  that do not say: most cover the lower half and tell the application
 *  nothing. */
private const val KEYBOARD_SHARE = 0.5f

/** Whether a field of the screen is being typed in, said by the field. */
@Stable
class KeyboardRoom {
    var typing by mutableStateOf(false)
}

val LocalKeyboardRoom = staticCompositionLocalOf<KeyboardRoom?> { null }

/**
 * What a screen with a field to type in shows, in the middle of the screen,
 * and in the room left above the keyboard while a field is typed in. What no
 * longer fits scrolls, the field typed in kept in sight; the ground behind
 * stays where it is.
 */
@Composable
fun AboveKeyboard(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val room = remember { KeyboardRoom() }
    val said = WindowInsets.ime.asPaddingValues().calculateBottomPadding()
    if (BuildConfig.DEBUG) {
        LaunchedEffect(said, room.typing) { Log.d("Melyxar", "keyboard: says $said, typing ${room.typing}") }
    }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val lift by animateDpAsState(if (room.typing) max(said, maxHeight * KEYBOARD_SHARE) else said, calm(), label = "lift")
        CompositionLocalProvider(LocalKeyboardRoom provides room) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = lift)
                    .verticalScroll(rememberScrollState())
                    .heightIn(min = maxHeight - lift),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
                content = content,
            )
        }
    }
}
