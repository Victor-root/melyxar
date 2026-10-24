package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import kotlinx.coroutines.delay

/** How many seconds the next episode is announced before it starts. */
private const val COUNT_FROM_S = 10

/** How wide the card is, and the still at its left. */
private val CARD_WIDE = 740.dp
private val STILL_WIDE = 320.dp

/** What the card stands on: the shade of the player's sheets. */
private val SHADE = Color.Black.copy(alpha = 0.72f)

/** How wide the ring of the count is, and how thick its line. */
private val RING_WIDE = 44.dp
private val RING_LINE = 3.5.dp

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
    // Its still with the buttons under it, and beside them what it is about,
    // whole, the card growing with it rather than cutting it.
    Row(
        modifier = modifier
            .width(CARD_WIDE)
            // The shade the sheets of the player stand on, the film showing
            // through it, and no rim or shadow of its own. Nothing blurred:
            // the film is drawn on a surface of its own that nothing over it
            // can blur, and a flat shade costs nothing to draw.
            .clip(shape)
            .background(SHADE)
            .padding(20.dp),
        horizontalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        // In the middle of the height the words give it; the words keep to
        // the top.
        Column(
            Modifier.width(STILL_WIDE).align(Alignment.CenterVertically),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Box(
                Modifier
                    .size(STILL_WIDE, STILL_WIDE * 9 / 16)
                    .clip(RoundedCornerShape(10.dp))
                    .background(palette.surface),
            ) {
                ServerPicture(next.still, STILL_WIDE, Modifier.matchParentSize())
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AccentButton(stringResource(R.string.tv_play_now), onPlay, Modifier.focusRequester(first), icon = null)
                PlainButton(stringResource(R.string.tv_cancel), onCancel)
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    stringResource(R.string.tv_next_episode),
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.textFaint,
                    modifier = Modifier.weight(1f),
                )
                Countdown(left, stringResource(R.string.tv_next_in, left))
            }
            Text(
                whichEpisode(next.season, next.episode, next.title.orEmpty()) ?: next.title.orEmpty(),
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = palette.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            next.overview?.let {
                Text(it, fontSize = 14.sp, lineHeight = 20.sp, color = palette.textSoft)
            }
        }
    }
}

/** The seconds left, in a ring of the accent going out with them a second at
 *  a time, smoothly. */
@Composable
private fun Countdown(left: Int, said: String) {
    val palette = LocalPalette.current
    val share by animateFloatAsState(
        (left.toFloat() / COUNT_FROM_S).coerceIn(0f, 1f),
        animationSpec = tween(1_000, easing = LinearEasing),
        label = "count",
    )
    Box(
        Modifier
            .size(RING_WIDE)
            .semantics { contentDescription = said }
            .drawBehind {
                val line = RING_LINE.toPx()
                val inset = Offset(line / 2, line / 2)
                val round = Size(size.width - line, size.height - line)
                drawArc(palette.text.copy(alpha = 0.12f), 0f, 360f, false, inset, round, style = Stroke(line))
                drawArc(palette.accent, -90f, 360f * share, false, inset, round, style = Stroke(line, cap = StrokeCap.Round))
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(left.coerceAtLeast(0).toString(), fontSize = 15.sp, fontWeight = FontWeight.Bold, color = palette.text)
    }
}
