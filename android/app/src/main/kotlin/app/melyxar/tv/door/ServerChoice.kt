package app.melyxar.tv.door

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.KnownServer
import app.melyxar.server.KnownServers
import app.melyxar.server.Reading
import app.melyxar.server.askBranding
import app.melyxar.server.readAddress
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.theme.paletteOf
import app.melyxar.tv.look.AboveKeyboard
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.MelyxarMark
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.TvField
import app.melyxar.tv.look.TvLook
import kotlinx.coroutines.launch

/**
 * Where the address of a server is typed, with the remote and the
 * television's keyboard, and where the servers already known are offered
 * again. The same rules as on the phone: plain http only at home.
 */
@Composable
fun ServerChoice(servers: KnownServers, onChosen: (KnownServer) -> Unit, onBack: () -> Unit) {
    val typed = rememberTextFieldState()
    var asking by remember { mutableStateOf(false) }
    var refusal by remember { mutableStateOf<Int?>(null) }
    val known = remember { servers.all() }
    val scope = rememberCoroutineScope()
    val first = remember { FocusRequester() }
    BackHandler(onBack = onBack)

    fun connect() {
        when (val reading = readAddress(typed.text.toString())) {
            Reading.NotAnAddress -> refusal = R.string.connect_not_address
            Reading.OpenOverInternet -> refusal = R.string.connect_open_over_internet
            is Reading.Usable -> {
                refusal = null
                asking = true
                scope.launch {
                    val branding = askBranding(reading.origin)
                    asking = false
                    if (branding == null) {
                        refusal = R.string.connect_unreachable
                    } else {
                        onChosen(KnownServer(reading.origin.value, branding.serverName))
                    }
                }
            }
        }
    }

    TvLook(paletteOf()) {
        val palette = LocalPalette.current
        StageLight {
            AboveKeyboard {
                Column(
                    modifier = Modifier.width(520.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    MelyxarMark(84.dp)
                    Text(
                        text = stringResource(R.string.app_name).uppercase(),
                        fontFamily = MelyxarDisplay,
                        fontSize = 28.sp,
                        letterSpacing = 0.18.em,
                        color = palette.text,
                    )
                    Text(
                        stringResource(R.string.connect_title),
                        fontSize = 22.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = palette.text,
                        modifier = Modifier.padding(top = 12.dp),
                    )
                    Text(
                        stringResource(R.string.connect_lead),
                        fontSize = 15.sp,
                        color = palette.textSoft,
                        textAlign = TextAlign.Center,
                    )
                    TvField(
                        state = typed,
                        placeholder = stringResource(R.string.connect_address),
                        onDone = ::connect,
                        onTyped = { refusal = null },
                        keyboard = KeyboardType.Uri,
                        enabled = !asking,
                        modifier = Modifier
                            .padding(top = 8.dp)
                            .focusRequester(first),
                    )
                    refusal?.let { Text(stringResource(it), color = palette.trouble, fontSize = 15.sp, textAlign = TextAlign.Center) }
                    AccentButton(
                        text = stringResource(if (asking) R.string.tv_signing_in else R.string.connect_go),
                        onClick = { if (!asking && typed.text.isNotBlank()) connect() },
                        icon = null,
                    )
                    if (known.isNotEmpty()) {
                        Text(
                            stringResource(R.string.connect_known),
                            color = palette.textFaint,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.08.em,
                            modifier = Modifier.padding(top = 18.dp),
                        )
                        // Coming down to them lands on the one left, or on the first.
                        val firstKnown = remember { FocusRequester() }
                        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.focusRestorer(firstKnown)) {
                            known.forEachIndexed { at, server ->
                                PlainButton(
                                    server.name,
                                    onClick = { onChosen(server) },
                                    modifier = if (at == 0) Modifier.focusRequester(firstKnown) else Modifier,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    LaunchedEffect(Unit) { first.requestFocus() }
}
