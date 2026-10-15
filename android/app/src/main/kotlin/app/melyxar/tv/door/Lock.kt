package app.melyxar.tv.door

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Api
import app.melyxar.server.Refusal
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.litBy
import java.io.IOException
import kotlinx.coroutines.launch

/** How many digits the code of the televisions holds, as the server keeps it. */
private const val DIGITS = 4

/** The keys of the pad, row by row; nothing under 7, and erasing under 9. */
private val PAD = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", null, "0", ERASE)

private const val ERASE = "erase"

/** The digits of a remote that has them, as the pad's own. */
private val REMOTE_DIGITS = mapOf(
    Key.Zero to '0', Key.One to '1', Key.Two to '2', Key.Three to '3', Key.Four to '4',
    Key.Five to '5', Key.Six to '6', Key.Seven to '7', Key.Eight to '8', Key.Nine to '9',
)

/**
 * The code a profile gave itself, asked before it opens again on this
 * television: typed on the pad with the arrows, or on the remote's own
 * digits, and checked by the server once the four are in.
 */
@Composable
fun CodeLock(api: Api, name: String, avatar: String?, onOpen: () -> Unit, onClose: () -> Unit) {
    val palette = LocalPalette.current
    val resources = LocalResources.current
    var typed by remember { mutableStateOf("") }
    var asking by remember { mutableStateOf(false) }
    var refusal by remember { mutableStateOf<String?>(null) }
    val shake = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val first = remember { FocusRequester() }
    val wrong = stringResource(R.string.tv_code_wrong)
    val generic = stringResource(R.string.tv_refused_generic)
    BackHandler(onBack = onClose)

    fun check(code: String) {
        asking = true
        scope.launch {
            try {
                api.checkTvCode(code)
                onOpen()
            } catch (refused: Refusal) {
                refusal = refused.waitSeconds?.let {
                    resources.getQuantityString(R.plurals.tv_refused_held_back, it, it)
                } ?: wrong
                typed = ""
                shake.shakeTheHead()
            } catch (_: IOException) {
                refusal = generic
                typed = ""
            } finally {
                asking = false
            }
        }
    }

    fun press(key: String) {
        if (asking) {
            return
        }
        refusal = null
        typed = if (key == ERASE) typed.dropLast(1) else (typed + key).take(DIGITS)
        if (typed.length == DIGITS) {
            check(typed)
        }
    }

    DoorPanel(shake) {
        Face(name, avatar, 72.dp, Modifier.clip(RoundedCornerShape(8.dp)))
        Text(
            stringResource(R.string.tv_code_of, name),
            fontSize = 19.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            textAlign = TextAlign.Center,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            repeat(DIGITS) { at ->
                Box(
                    Modifier
                        .size(14.dp)
                        .clip(CircleShape)
                        .background(if (at < typed.length) palette.accentBright else palette.text.copy(alpha = 0.18f)),
                )
            }
        }
        refusal?.let { Text(it, color = palette.trouble, fontSize = 15.sp, textAlign = TextAlign.Center) }
        Column(
            verticalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier.onPreviewKeyEvent { event ->
                val digit = REMOTE_DIGITS[event.key] ?: return@onPreviewKeyEvent false
                if (event.type == KeyEventType.KeyDown) press(digit.toString())
                true
            },
        ) {
            PAD.chunked(3).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    row.forEach { key ->
                        if (key == null) {
                            Box(Modifier.size(PAD_KEY))
                        } else {
                            PadKey(
                                key,
                                label = if (key == ERASE) stringResource(R.string.tv_code_erase) else key,
                                onClick = { press(key) },
                                modifier = if (key == "1") Modifier.focusRequester(first) else Modifier,
                            )
                        }
                    }
                }
            }
        }
    }
    LaunchedEffect(Unit) { first.requestFocus() }
}

private val PAD_KEY = 64.dp

@Composable
private fun PadKey(key: String, label: String, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier = modifier
            .size(PAD_KEY)
            .clip(shape)
            .background(palette.accent.copy(alpha = 0.10f + 0.20f * lit))
            .border(1.5.dp, palette.accentBright.copy(alpha = lit), shape)
            .clickable(interaction, indication = null, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            fontSize = if (key == ERASE) 13.sp else 24.sp,
            fontWeight = FontWeight.SemiBold,
            color = if (lit > 0.5f) palette.accentBright else palette.text,
        )
    }
}
