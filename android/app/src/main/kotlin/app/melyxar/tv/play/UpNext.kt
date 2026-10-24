package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.Alignment
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
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised
import kotlinx.coroutines.delay

/** How many seconds the next episode is announced before it starts. */
private const val COUNT_FROM_S = 10

/** How wide the card is, and the still at its left. */
private val CARD_WIDE = 640.dp
private val STILL_WIDE = 240.dp

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
fun UpNext(
    next: NextEpisode,
    /** Whether the count goes on: while the film plays, or once it is over.
     *  A film paused holds it where it is. */
    running: Boolean,
    first: FocusRequester,
    onPlay: () -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(14.dp)
    var left by remember(next.id) { mutableIntStateOf(COUNT_FROM_S) }
    val play by rememberUpdatedState(onPlay)
    LaunchedEffect(next.id, running) {
        if (!running) return@LaunchedEffect
        while (left > 0) {
            delay(1_000)
            left -= 1
        }
        play()
    }
    BackHandler(onBack = onCancel)
    Column(
        modifier = modifier
            .width(CARD_WIDE)
            .boxShadow(shape, palette.shadowRaised)
            .clip(shape)
            .background(palette.surfaceRaised)
            .border(1.dp, palette.border, shape)
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        // Its still and what it is about, as its line on the page of the
        // season shows it.
        Row(horizontalArrangement = Arrangement.spacedBy(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(STILL_WIDE, STILL_WIDE * 9 / 16)
                    .clip(RoundedCornerShape(10.dp))
                    .background(palette.surface),
            ) {
                ServerPicture(next.still, STILL_WIDE, Modifier.matchParentSize())
            }
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(stringResource(R.string.tv_next_episode), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = palette.textFaint)
                Text(
                    whichEpisode(next.season, next.episode, next.title.orEmpty()) ?: next.title.orEmpty(),
                    fontSize = 18.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.text,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                next.overview?.let {
                    Text(it, fontSize = 14.sp, lineHeight = 20.sp, color = palette.textSoft, maxLines = 3, overflow = TextOverflow.Ellipsis)
                }
                Text(stringResource(R.string.tv_next_in, left), fontSize = 14.sp, color = palette.textSoft)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End), modifier = Modifier.fillMaxWidth()) {
            AccentButton(stringResource(R.string.tv_play_now), onPlay, Modifier.focusRequester(first))
            PlainButton(stringResource(R.string.tv_cancel), onCancel)
        }
    }
}
