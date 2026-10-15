package app.melyxar.connect

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.KnownServer
import app.melyxar.server.KnownServers
import app.melyxar.server.Reading
import app.melyxar.server.askBranding
import app.melyxar.server.readAddress
import app.melyxar.theme.Melyxar
import app.melyxar.theme.MelyxarTheme
import app.melyxar.theme.drawUnderTheBars
import app.melyxar.tv.TvActivity
import app.melyxar.tv.isTelevision
import app.melyxar.web.WebActivity
import kotlinx.coroutines.launch

/**
 * The one screen before any server: where its address is typed, and where the
 * servers already known are offered again. Opening the application goes
 * straight to the last one, unless this screen was asked for to change.
 *
 * On a television, the application is the television's own instead, which
 * keeps its own choice of server.
 */
class ConnectActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (isTelevision()) {
            startActivity(Intent(this, TvActivity::class.java))
            finish()
            return
        }
        val servers = KnownServers(this)
        val current = servers.current()
        if (current != null && !intent.getBooleanExtra(CHOOSING, false)) {
            open(current)
            return
        }
        drawUnderTheBars()
        setContent {
            MelyxarTheme {
                ConnectScreen(servers, onOpen = { server ->
                    servers.open(server)
                    open(server)
                })
            }
        }
    }

    private fun open(server: KnownServer) {
        startActivity(WebActivity.opening(this, server))
        finish()
    }

    companion object {
        private const val CHOOSING = "choosing"

        /** This screen, to choose another server than the one open. */
        fun choosing(context: Context) = Intent(context, ConnectActivity::class.java)
            .putExtra(CHOOSING, true)
    }
}

@Composable
private fun ConnectScreen(servers: KnownServers, onOpen: (KnownServer) -> Unit) {
    var typed by remember { mutableStateOf("") }
    var asking by remember { mutableStateOf(false) }
    var refusal by remember { mutableStateOf<Int?>(null) }
    var known by remember { mutableStateOf(servers.all()) }
    val scope = rememberCoroutineScope()

    fun connect() {
        when (val reading = readAddress(typed)) {
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
                        onOpen(KnownServer(reading.origin.value, branding.serverName))
                    }
                }
            }
        }
    }

    Surface(modifier = Modifier.fillMaxSize(), color = Melyxar.ground) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .safeDrawingPadding()
                .verticalScroll(rememberScrollState()),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = Modifier
                    .widthIn(max = 460.dp)
                    .padding(horizontal = 24.dp, vertical = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Image(
                    painter = painterResource(R.drawable.melyxar_mark),
                    contentDescription = null,
                    modifier = Modifier.size(84.dp),
                )
                Text(
                    text = stringResource(R.string.app_name).uppercase(),
                    color = Melyxar.text,
                    fontSize = 24.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = 4.sp,
                )
                Text(
                    text = stringResource(R.string.connect_title),
                    color = Melyxar.text,
                    fontSize = 18.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(top = 10.dp),
                )
                Text(
                    text = stringResource(R.string.connect_lead),
                    color = Melyxar.textSoft,
                    fontSize = 14.sp,
                    textAlign = TextAlign.Center,
                )
                OutlinedTextField(
                    value = typed,
                    onValueChange = {
                        typed = it
                        refusal = null
                    },
                    label = { Text(stringResource(R.string.connect_address)) },
                    singleLine = true,
                    enabled = !asking,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { connect() }),
                    modifier = Modifier.fillMaxWidth(),
                )
                refusal?.let { said ->
                    Text(text = stringResource(said), color = Melyxar.trouble, fontSize = 13.sp)
                }
                Button(
                    onClick = { connect() },
                    enabled = !asking && typed.isNotBlank(),
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(50.dp),
                ) {
                    if (asking) {
                        CircularProgressIndicator(
                            color = Melyxar.accentContrast,
                            strokeWidth = 2.dp,
                            modifier = Modifier.size(20.dp),
                        )
                    } else {
                        Text(stringResource(R.string.connect_go), fontSize = 16.sp)
                    }
                }
                if (known.isNotEmpty()) {
                    KnownList(
                        known = known,
                        onOpen = onOpen,
                        onForget = { server ->
                            servers.forget(server.origin)
                            known = servers.all()
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun KnownList(
    known: List<KnownServer>,
    onOpen: (KnownServer) -> Unit,
    onForget: (KnownServer) -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth().padding(top = 18.dp)) {
        Text(
            text = stringResource(R.string.connect_known),
            color = Melyxar.textFaint,
            fontSize = 12.sp,
            fontWeight = FontWeight.SemiBold,
            letterSpacing = 1.sp,
            modifier = Modifier.padding(bottom = 6.dp),
        )
        known.forEach { server ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpen(server) }
                    .padding(vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(server.name, color = Melyxar.text, fontSize = 15.sp, fontWeight = FontWeight.Medium)
                    Text(server.origin, color = Melyxar.textFaint, fontSize = 12.sp)
                }
                TextButton(onClick = { onForget(server) }) {
                    Text(stringResource(R.string.connect_forget), color = Melyxar.textSoft)
                }
            }
        }
    }
}
