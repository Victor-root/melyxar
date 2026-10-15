package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.NextEpisode
import app.melyxar.tv.inside.whichEpisode
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised
import kotlinx.coroutines.delay

/** How many seconds the next episode is announced before it starts. */
private const val COUNT_FROM_S = 10

/** How often the film is looked at for its credits. */
private const val LOOKED_AT_EVERY_MS = 500L

/** Whether the film is in its credits, when its file says where they are:
 *  the next episode is announced from there rather than from its very end. */
@Composable
fun rememberInCredits(playback: Playback): State<Boolean> = produceState(false, playback.plan) {
    val credits = playback.plan?.segments?.firstOrNull { it.kind == "outro" } ?: return@produceState
    while (true) {
        value = playback.seconds >= credits.fromSecond
        delay(LOOKED_AT_EVERY_MS)
    }
}

/**
 * The next episode, announced over the end of this one and started once the
 * count is over, or at once on its button. Cancelled, it is not offered
 * again, and the end of the film leads back to where it came from.
 */
@Composable
fun UpNext(next: NextEpisode, first: FocusRequester, onPlay: () -> Unit, onCancel: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(14.dp)
    var left by remember(next.id) { mutableIntStateOf(COUNT_FROM_S) }
    val play by rememberUpdatedState(onPlay)
    LaunchedEffect(next.id) {
        while (left > 0) {
            delay(1_000)
            left -= 1
        }
        play()
    }
    BackHandler(onBack = onCancel)
    Column(
        modifier = modifier
            .width(400.dp)
            .boxShadow(shape, palette.shadowRaised)
            .clip(shape)
            .background(palette.surfaceRaised)
            .border(1.dp, palette.border, shape)
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(stringResource(R.string.tv_next_episode), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = palette.textFaint)
        Text(
            whichEpisode(next.season, next.episode, next.title.orEmpty()) ?: next.title.orEmpty(),
            fontSize = 18.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Text(stringResource(R.string.tv_next_in, left), fontSize = 14.sp, color = palette.textSoft)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(top = 10.dp)) {
            AccentButton(stringResource(R.string.tv_play_now), onPlay, Modifier.focusRequester(first))
            PlainButton(stringResource(R.string.tv_cancel), onCancel)
        }
    }
}
