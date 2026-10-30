package app.melyxar.tv.inside

import android.os.SystemClock
import android.text.format.DateFormat
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.LiveWord
import app.melyxar.server.MaintenanceSeen
import app.melyxar.server.maintenance
import app.melyxar.server.serverJson
import app.melyxar.tv.clock
import app.melyxar.tv.door.ServerMark
import app.melyxar.tv.instantOf
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.glassPiece
import app.melyxar.tv.look.rim
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.SerializationException
import java.io.IOException
import java.util.Date

/** Maintenance as the television holds it, with when an announced one takes
 *  hold on this television's own steady clock. */
data class Standing(val state: String, val startsAtMs: Long?, val until: String?, val message: String?) {
    /** Where it stands at [nowMs]: an announcement whose time has come is in
     *  force, whether or not the server has said so yet. */
    fun stateAt(nowMs: Long): String =
        if (state == "announced" && startsAtMs != null && nowMs >= startsAtMs) "on" else state

    companion object {
        val OFF = Standing("off", null, null, null)
    }
}

/** What the server said, held from the moment it was heard. */
fun standingOf(seen: MaintenanceSeen, heardAtMs: Long) = Standing(
    state = seen.state,
    startsAtMs = seen.startsInSeconds?.takeIf { seen.state == "announced" }?.let { heardAtMs + it * 1000 },
    until = seen.until,
    message = seen.message,
)

/** How often it is asked again while this television is turned away. */
private const val ASKED_AGAIN_EVERY_MS = 10_000L

/**
 * Where maintenance stands for this television, as the web's page holds it:
 * asked as Melyxar comes on and whenever the live line opens, told on the
 * line whenever it moves, and asked again every little while for as long as
 * it turns this account away, when the screens that would hear it are gone.
 */
class Maintenance(private val api: Api, private val scope: CoroutineScope) {
    var standing by mutableStateOf(Standing.OFF)
        private set

    private fun heard(seen: MaintenanceSeen) {
        standing = standingOf(seen, SystemClock.elapsedRealtime())
    }

    fun ask() {
        scope.launch {
            try {
                heard(api.maintenance())
            } catch (_: IOException) {
                // A server out of reach says nothing about maintenance.
            }
        }
    }

    /** A word of the live line: the line opening, or maintenance moving. */
    fun heard(word: LiveWord) {
        when (word.name) {
            "open" -> ask()
            "maintenance" -> try {
                heard(serverJson.decodeFromString(MaintenanceSeen.serializer(), word.data))
            } catch (_: SerializationException) {
                ask()
            }
        }
    }

    /** The announcement's time has come. */
    fun takesHold() {
        if (standing.stateAt(SystemClock.elapsedRealtime()) == "on") {
            standing = standing.copy(state = "on", startsAtMs = null)
        }
    }
}

/** Counts down to an announced maintenance, and asks again while it turns
 *  this television away. */
@Composable
fun FollowMaintenance(maintenance: Maintenance, turnedAway: Boolean) {
    val startsAt = maintenance.standing.startsAtMs
    LaunchedEffect(startsAt) {
        startsAt ?: return@LaunchedEffect
        delay((startsAt - SystemClock.elapsedRealtime()).coerceAtLeast(0))
        maintenance.takesHold()
    }
    LaunchedEffect(turnedAway) {
        while (turnedAway) {
            delay(ASKED_AGAIN_EVERY_MS)
            maintenance.ask()
        }
    }
}

/** When it is expected to end, as this television writes an hour. */
@Composable
private fun backAt(standing: Standing): String? {
    val context = LocalContext.current
    val hours = remember(context) { DateFormat.getTimeFormat(context) }
    return standing.until?.let(::instantOf)?.let { hours.format(Date(it)) }
}

/**
 * The screen of a maintenance in force, standing in for every screen of an
 * account that is not an administrator's. The remote starts on the way to
 * another profile, so an administrator can come in on this television.
 */
@Composable
fun MaintenanceScreen(branding: Branding, standing: Standing, onChangeProfile: () -> Unit) {
    val palette = LocalPalette.current
    val change = remember { FocusRequester() }
    val back = backAt(standing)
    Column(
        modifier = Modifier.fillMaxSize().padding(Gutter),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
    ) {
        ServerMark(branding, 96.dp)
        Text(
            stringResource(R.string.tv_maintenance_title),
            fontSize = 34.sp,
            fontWeight = FontWeight.Bold,
            color = palette.text,
            modifier = Modifier.padding(top = 10.dp),
        )
        Text(
            stringResource(R.string.tv_maintenance_closed, branding.serverName),
            fontSize = 17.sp,
            color = palette.textSoft,
            textAlign = TextAlign.Center,
        )
        standing.message?.let {
            Text(it, fontSize = 18.sp, color = palette.text, textAlign = TextAlign.Center, modifier = Modifier.widthIn(max = 640.dp))
        }
        Text(
            back?.let { stringResource(R.string.tv_maintenance_back_at, it) } ?: stringResource(R.string.tv_maintenance_back_soon),
            fontSize = 17.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.attention,
        )
        Text(
            stringResource(R.string.tv_maintenance_comes_back),
            fontSize = 15.sp,
            color = palette.textSoft,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(bottom = 14.dp),
        )
        PlainButton(stringResource(R.string.tv_change_profile), onChangeProfile, Modifier.focusRequester(change))
    }
    LaunchedEffect(Unit) { change.requestFocus() }
}

/**
 * The band over every screen, a film's included: the time left before an
 * announced maintenance takes hold, and to an administrator, that one is in
 * force. Never in the way of the remote.
 */
@Composable
fun MaintenanceBand(standing: Standing, administrator: Boolean, modifier: Modifier = Modifier) {
    val startsAt = standing.startsAtMs
    when {
        standing.state == "announced" && startsAt != null -> {
            var now by remember { mutableLongStateOf(SystemClock.elapsedRealtime()) }
            LaunchedEffect(startsAt) {
                while (true) {
                    now = SystemClock.elapsedRealtime()
                    delay(1_000)
                }
            }
            val left = ((startsAt - now + 999) / 1000).coerceAtLeast(0)
            Band(
                stringResource(R.string.tv_maintenance_announced, clock(left.toDouble())),
                standing.message ?: stringResource(R.string.tv_maintenance_announced_detail),
                modifier,
            )
        }
        standing.state == "on" && administrator -> Band(
            stringResource(R.string.tv_maintenance_in_force),
            backAt(standing)?.let { stringResource(R.string.tv_maintenance_back_at, it) },
            modifier,
        )
    }
}

@Composable
private fun Band(title: String, detail: String?, modifier: Modifier) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(14.dp)
    Row(
        modifier = modifier
            // Under the bar, beside the notifications.
            .padding(top = 84.dp)
            .widthIn(max = 760.dp)
            .glassPiece(shape)
            .rim(shape, palette.attention, 1.5.dp)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Glyph(Icons.Clock, palette.attention, 20.dp)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
            detail?.let { Text(it, fontSize = 13.sp, color = palette.textSoft, maxLines = 2) }
        }
    }
}
