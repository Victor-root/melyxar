package app.melyxar.tv.look

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicSecureTextField
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * A field to type in with the television's keyboard: the remote comes to it
 * without the keyboard covering the screen, a press of the middle button
 * opens it, and the keyboard's own key sends it.
 *
 * Read only until that press: a field that can be typed in opens the
 * keyboard as soon as it is reached, and one told not to opens it only for
 * a finger on the screen, never for the remote.
 */
@Composable
fun TvField(
    state: TextFieldState,
    placeholder: String,
    onDone: () -> Unit,
    modifier: Modifier = Modifier,
    /** Told of what the viewer types, never of what the screen writes in. */
    onTyped: () -> Unit = {},
    secret: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    enabled: Boolean = true,
) {
    val palette = LocalPalette.current
    var typing by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val textStyle = LocalTextStyle.current.copy(color = palette.text, fontSize = 18.sp)
    val keyboardOptions = KeyboardOptions(
        keyboardType = if (secret) KeyboardType.Password else keyboard,
        imeAction = ImeAction.Done,
        autoCorrectEnabled = false,
    )
    val sent = KeyboardActionHandler {
        typing = false
        onDone()
    }
    val typed = InputTransformation { onTyped() }
    val looks = modifier
        .onPreviewKeyEvent { event ->
            val opens = event.key == Key.DirectionCenter || event.key == Key.Enter || event.key == Key.NumPadEnter
            if (typing || !enabled || !opens) {
                return@onPreviewKeyEvent false
            }
            if (event.type == KeyEventType.KeyUp) {
                typing = true
            }
            true
        }
        .onFocusChanged { if (!it.isFocused) typing = false }
        .fillMaxWidth()
        .grows({ lit }, 1.02f)
        .clip(ControlShape)
        .background(palette.surfaceRaised.copy(alpha = 0.74f))
        .border(1.dp, palette.border, ControlShape)
        .rim(ControlShape, palette.accentBright, 2.dp) { lit }
    val decorator = TextFieldDecorator { field ->
        Box(Modifier.padding(horizontal = 18.dp, vertical = 14.dp)) {
            if (state.text.isEmpty()) {
                Text(placeholder, color = palette.textFaint, fontSize = 18.sp, maxLines = 1)
            }
            field()
        }
    }
    if (secret) {
        BasicSecureTextField(
            state = state,
            modifier = looks,
            enabled = enabled,
            readOnly = !typing,
            inputTransformation = typed,
            textStyle = textStyle,
            keyboardOptions = keyboardOptions,
            onKeyboardAction = sent,
            interactionSource = interaction,
            cursorBrush = SolidColor(palette.accentBright),
            decorator = decorator,
        )
    } else {
        BasicTextField(
            state = state,
            modifier = looks,
            enabled = enabled,
            readOnly = !typing,
            inputTransformation = typed,
            textStyle = textStyle,
            keyboardOptions = keyboardOptions,
            onKeyboardAction = sent,
            lineLimits = TextFieldLineLimits.SingleLine,
            interactionSource = interaction,
            cursorBrush = SolidColor(palette.accentBright),
            decorator = decorator,
        )
    }
}
