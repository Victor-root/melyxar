package app.melyxar.tv.door

import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.KeptSession
import app.melyxar.server.LetInYet
import app.melyxar.server.LinkAsked
import app.melyxar.server.Refusal
import app.melyxar.tv.clock
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.theme.MelyxarDisplay
import java.io.IOException
import kotlinx.coroutines.delay

/** How often the server is asked whether somebody let this television in. */
private const val ASKED_EVERY_MS = 2_000L

/** The squares of empty margin a reader needs around a QR code. */
private const val QR_MARGIN = 4

/**
 * Signing in from a telephone: a code and the QR code that brings it, for an
 * account signed in elsewhere to let this television in, so nothing is typed
 * with the remote. A code over is replaced on its own; the server is asked
 * every couple of seconds whether somebody said yes, and the answer that
 * does is the account and the session this television now holds.
 */
@Composable
fun LetInFromATelephone(api: Api, client: String, onIn: (KeptSession, Account) -> Unit, onClose: () -> Unit) {
    val palette = LocalPalette.current
    var asked by remember { mutableStateOf<LinkAsked?>(null) }
    var left by remember { mutableIntStateOf(0) }
    var failed by remember { mutableStateOf(false) }
    var round by remember { mutableIntStateOf(0) }
    val shake = remember { Animatable(0f) }
    val back = remember { FocusRequester() }
    BackHandler(onBack = onClose)

    LaunchedEffect(round) {
        asked = null
        failed = false
        try {
            val answer = api.askToBeLetIn(api.origin.value, Build.MODEL, client)
            asked = answer
            left = answer.lastsSeconds
        } catch (_: IOException) {
            failed = true
        }
    }
    LaunchedEffect(asked) {
        val waiting = asked ?: return@LaunchedEffect
        while (true) {
            delay(ASKED_EVERY_MS)
            try {
                when (val standing = api.letInYet(waiting.secret)) {
                    is LetInYet.Waiting -> left = standing.secondsLeft
                    is LetInYet.In -> {
                        val token = standing.session
                        // Only a nicer line in the list of devices: not worth failing for.
                        runCatching { api.signedInAs(token).nameTheDevice() }
                        onIn(KeptSession(standing.account.name, token), standing.account)
                        return@LaunchedEffect
                    }
                    LetInYet.Gone -> {
                        round += 1
                        return@LaunchedEffect
                    }
                }
            } catch (_: Refusal) {
                // Asked again at the next beat.
            } catch (_: IOException) {
                // Asked again at the next beat.
            }
        }
    }
    LaunchedEffect(asked) {
        while (asked != null && left > 0) {
            delay(1_000)
            left -= 1
        }
    }

    DoorPanel(shake, width = 760.dp) {
        Row(horizontalArrangement = Arrangement.spacedBy(32.dp), verticalAlignment = Alignment.CenterVertically) {
            asked?.let { QrSquares(it.squares, 220.dp) }
            Column(verticalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.width(420.dp)) {
                Text(stringResource(R.string.tv_link_title), fontSize = 21.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
                Text(stringResource(R.string.tv_link_how), fontSize = 15.sp, color = palette.textSoft)
                when {
                    failed -> Text(stringResource(R.string.tv_refused_generic), fontSize = 15.sp, color = palette.trouble)
                    else -> {
                        Text(
                            asked?.code?.let { "${it.take(3)} ${it.drop(3)}" } ?: "··· ···",
                            fontFamily = MelyxarDisplay,
                            fontSize = 48.sp,
                            letterSpacing = 6.sp,
                            color = palette.text,
                        )
                        if (asked != null) {
                            Text(stringResource(R.string.tv_link_left, clock(left.toDouble())), fontSize = 14.sp, color = palette.textFaint)
                        }
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (failed) {
                        PlainButton(stringResource(R.string.tv_retry), { round += 1 })
                    }
                    PlainButton(stringResource(R.string.tv_link_back), onClose, Modifier.focusRequester(back))
                }
            }
        }
    }
    LaunchedEffect(Unit) { back.requestFocus() }
}

/** A QR code as the server made it, dark on light whatever the theme: a
 *  camera reads contrast, not colours. */
@Composable
private fun QrSquares(squares: List<String>, size: Dp) {
    Canvas(
        Modifier
            .size(size)
            .clip(RoundedCornerShape(12.dp))
            .background(Color.White),
    ) {
        val across = squares.size + 2 * QR_MARGIN
        val square = this.size.width / across
        squares.forEachIndexed { y, row ->
            row.forEachIndexed { x, dark ->
                if (dark == '1') {
                    drawRect(
                        Color.Black,
                        topLeft = Offset((x + QR_MARGIN) * square, (y + QR_MARGIN) * square),
                        size = Size(square + 0.5f, square + 0.5f),
                    )
                }
            }
        }
    }
}
