package app.melyxar.tv.door

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector1D
import androidx.compose.animation.core.keyframes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
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
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.KeptSession
import app.melyxar.server.Refusal
import app.melyxar.tv.look.AboveKeyboard
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.BoxShadow
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.TvField
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowCard
import java.io.IOException
import kotlinx.coroutines.launch

/**
 * The password of a profile, typed once on this television: from then on the
 * profile opens by choosing it. With no profile named, the name is typed too.
 */
@Composable
fun Knock(
    api: Api,
    known: String?,
    avatar: String?,
    client: String,
    onIn: (KeptSession, Account) -> Unit,
    onClose: () -> Unit,
) {
    val palette = LocalPalette.current
    val name = rememberTextFieldState(known ?: "")
    val password = rememberTextFieldState()
    var asking by remember { mutableStateOf(false) }
    var refusal by remember { mutableStateOf<String?>(null) }
    val shake = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val first = remember { FocusRequester() }
    BackHandler(onBack = onClose)

    val wrong = stringResource(R.string.tv_refused_wrong)
    val generic = stringResource(R.string.tv_refused_generic)
    val resources = LocalResources.current

    fun signIn() {
        if (asking || name.text.isBlank() || password.text.isEmpty()) {
            return
        }
        asking = true
        refusal = null
        scope.launch {
            try {
                val (account, token) = api.signIn(name.text.trim().toString(), password.text.toString(), client)
                // Only a nicer line in the list of devices: not worth failing for.
                runCatching { api.signedInAs(token).nameTheDevice() }
                onIn(KeptSession(account.name, token), account)
            } catch (refused: Refusal) {
                refusal = when {
                    refused.waitSeconds != null ->
                        resources.getQuantityString(R.plurals.tv_refused_held_back, refused.waitSeconds, refused.waitSeconds)
                    refused.signedOut -> wrong
                    else -> generic
                }
                password.clearText()
                shake.shakeTheHead()
            } catch (_: IOException) {
                refusal = generic
            } finally {
                asking = false
            }
        }
    }

    DoorPanel(shake) {
        if (known != null) {
            Face(known, avatar, 72.dp, Modifier.clip(RoundedCornerShape(8.dp)))
            Text(
                stringResource(R.string.tv_password_of, known),
                fontSize = 19.sp,
                fontWeight = FontWeight.SemiBold,
                color = palette.text,
                textAlign = TextAlign.Center,
            )
        } else {
            TvField(
                state = name,
                placeholder = stringResource(R.string.tv_name),
                onDone = {},
                modifier = Modifier.focusRequester(first),
            )
        }
        TvField(
            state = password,
            placeholder = stringResource(R.string.tv_password),
            onDone = ::signIn,
            onTyped = { refusal = null },
            secret = true,
            enabled = !asking,
            modifier = if (known != null) Modifier.focusRequester(first) else Modifier,
        )
        refusal?.let { Text(it, color = palette.trouble, fontSize = 15.sp, textAlign = TextAlign.Center) }
        AccentButton(
            text = stringResource(if (asking) R.string.tv_signing_in else R.string.tv_sign_in),
            onClick = ::signIn,
            icon = null,
        )
        Text(
            stringResource(R.string.tv_password_once),
            fontSize = 13.sp,
            color = palette.textFaint,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
    LaunchedEffect(Unit) { first.requestFocus() }
}

/**
 * A panel over the door, as the web's: the remote stays in it until it is
 * closed, and it shakes its head at a refusal by [shake].
 */
@Composable
fun DoorPanel(shake: Animatable<Float, AnimationVector1D>, content: @Composable ColumnScope.() -> Unit) {
    val palette = LocalPalette.current
    AboveKeyboard(Modifier.background(palette.ground.copy(alpha = 0.72f))) {
        val shape = RoundedCornerShape(18.dp)
        Column(
            modifier = Modifier
                .width(440.dp)
                .focusProperties { onExit = { cancelFocusChange() } }
                .focusGroup()
                .graphicsLayer { translationX = shake.value.dp.toPx() }
                .boxShadow(shape, palette.shadowCard, BoxShadow(palette.accentGlow, blur = 14.dp, spread = 2.dp))
                .clip(shape)
                .background(palette.surfaceRaised)
                .background(palette.accent.copy(alpha = 0.10f))
                .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
                .padding(28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
            content = content,
        )
    }
}

/** A panel's head shaken, as at a refusal. */
suspend fun Animatable<Float, AnimationVector1D>.shakeTheHead() {
    animateTo(
        0f,
        keyframes {
            durationMillis = 240
            -4f at 60
            2f at 160
        },
    )
}
