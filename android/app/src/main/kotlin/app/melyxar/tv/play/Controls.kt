package app.melyxar.tv.play

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Thumbnails
import app.melyxar.server.Track
import app.melyxar.server.Work
import app.melyxar.tv.clock
import app.melyxar.tv.inside.whichEpisode
import app.melyxar.tv.languageName
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.shadowRaised
import app.melyxar.tv.soundName
import app.melyxar.tv.speakersName
import kotlinx.coroutines.delay
import kotlin.math.max
import kotlin.math.roundToInt

/** How often the clock and the bar move while the controls are up. */
private const val TICK_MS = 250L

/** How wide the picture over the bar is, and how far above it it stands. */
private val PREVIEW_ACROSS = 240.dp
private val PREVIEW_GAP = 10.dp

/** Every number of the player keeps its width as it changes. */
private val Times = TextStyle(
    fontSize = 15.sp,
    fontWeight = FontWeight.SemiBold,
    fontFeatureSettings = "tnum",
    shadow = Shadow(Color.Black.copy(alpha = 0.8f), Offset.Zero, 12f),
)

@Composable
fun Controls(
    playback: Playback,
    work: Work?,
    playing: Boolean,
    first: FocusRequester,
    onMenu: (Menu) -> Unit,
    onNext: (() -> Unit)?,
) {
    val palette = LocalPalette.current
    var at by remember { mutableDoubleStateOf(playback.seconds) }
    LaunchedEffect(playback) {
        while (true) {
            at = playback.seconds
            delay(TICK_MS)
        }
    }
    val length = playback.length
    Box(Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.72f), Color.Transparent)))
                .padding(start = Gutter, end = Gutter, top = 24.dp, bottom = 56.dp),
        ) {
            work?.let {
                val series = it.ancestry.firstOrNull { above -> above.kind == "series" }?.title
                Text(
                    series ?: it.title,
                    fontSize = 22.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = TextStyle(shadow = Shadow(Color.Black.copy(alpha = 0.6f), Offset.Zero, 16f)),
                )
                if (series != null) {
                    val season = it.ancestry.firstOrNull { above -> above.kind == "season" }?.number
                    whichEpisode(season, it.number, it.title)?.let { line ->
                        Text(line, fontSize = 15.sp, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.82f))))
                .padding(start = Gutter, end = Gutter, top = 90.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Text(clock(at), style = Times, color = palette.text)
                SeekBar(at, length, playback, Modifier.weight(1f))
                Text(length?.let { "-" + clock(it - at) } ?: "", style = Times, color = palette.text)
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier.focusRestorer(first),
            ) {
                PlayerButton(Icons.Restart, onClick = { playback.withStep(-10) })
                PlayerButton(
                    if (playing) Icons.Pause else Icons.Play,
                    onClick = playback::toggle,
                    large = true,
                    modifier = Modifier.focusRequester(first),
                )
                PlayerButton(Icons.StepOn, onClick = { playback.withStep(30) })
                onNext?.let { PlayerButton(Icons.NextEpisode, onClick = it) }
                Spacer(Modifier.weight(1f))
                if ((playback.plan?.audio?.size ?: 0) > 1) {
                    PlayerButton(Icons.Audio, onClick = { onMenu(Menu.Audio) })
                }
                if (playback.plan?.subtitles?.isNotEmpty() == true) {
                    PlayerButton(Icons.Subtitles, onClick = { onMenu(Menu.Subtitles) })
                }
            }
        }
    }
}

/**
 * Where the film is. While the remote is on the bar its arrows take it along,
 * the film's picture there shown above it as the web shows it under the hand,
 * and the film goes there once the remote rests or the middle button is
 * pressed.
 */
@Composable
private fun SeekBar(at: Double, length: Double?, playback: Playback, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val thick by animateDpAsState(if (lit > 0.5f) 14.dp else 10.dp, label = "bar")
    var target by remember { mutableStateOf<Double?>(null) }
    var moves by remember { mutableIntStateOf(0) }

    fun settle() {
        target?.let(playback::goTo)
        target = null
    }

    LaunchedEffect(moves) {
        if (target != null) {
            delay(SCRUB_SETTLES_MS)
            settle()
        }
    }
    val shareOf = { seconds: Double -> length?.takeIf { it > 0 }?.let { (seconds / it).toFloat().coerceIn(0f, 1f) } ?: 0f }
    val done = shareOf(target ?: at)
    val buffered = shareOf(playback.player.bufferedPosition / 1000.0)
    Box(
        modifier = modifier
            .height(32.dp)
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) {
                    return@onPreviewKeyEvent false
                }
                val by = when (event.key) {
                    Key.DirectionLeft -> -1
                    Key.DirectionRight -> 1
                    Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                        settle()
                        return@onPreviewKeyEvent true
                    }
                    else -> return@onPreviewKeyEvent false
                }
                val moved = (target ?: at) + by * scrubStep(event.nativeKeyEvent.repeatCount)
                target = moved.coerceIn(0.0, length ?: moved.coerceAtLeast(0.0))
                moves += 1
                true
            }
            .onFocusChanged { if (!it.isFocused) settle() }
            .focusable(interactionSource = interaction)
            .drawBehind {
                val line = thick.toPx()
                val top = (size.height - line) / 2
                val round = CornerRadius(line / 2)
                drawRoundRect(Color.White.copy(alpha = 0.2f), Offset(0f, top), Size(size.width, line), round)
                drawRoundRect(Color.White.copy(alpha = 0.32f), Offset(0f, top), Size(size.width * buffered, line), round)
                drawRoundRect(palette.accent, Offset(0f, top), Size(size.width * done, line), round)
                if (lit > 0f) {
                    val handle = Size(8.dp.toPx(), 18.dp.toPx() * 1.5f * lit)
                    drawRoundRect(
                        Color.White.copy(alpha = lit),
                        Offset(size.width * done - handle.width / 2, (size.height - handle.height) / 2),
                        handle,
                        CornerRadius(2.dp.toPx()),
                    )
                }
            },
    ) {
        target?.let { Preview(it, done, playback.plan?.thumbnails, Modifier.matchParentSize()) }
    }
}

/**
 * The picture of the film where the bar was taken, and its time under it,
 * standing over that place of the bar and never past either of its ends.
 */
@Composable
private fun Preview(seconds: Double, share: Float, thumbnails: Thumbnails?, modifier: Modifier) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(8.dp)
    Column(
        modifier = modifier.layout { measurable, constraints ->
            val shown = measurable.measure(Constraints())
            val half = shown.width / 2
            val along = (share * constraints.maxWidth).roundToInt().coerceIn(half, max(half, constraints.maxWidth - half))
            layout(constraints.maxWidth, constraints.maxHeight) {
                shown.place(along - half, -shown.height - PREVIEW_GAP.roundToPx())
            }
        },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        thumbnails?.let {
            Thumbnail(
                it,
                seconds,
                PREVIEW_ACROSS,
                Modifier
                    .boxShadow(shape, palette.shadowRaised)
                    .clip(shape)
                    .background(Color.Black)
                    .border(1.dp, Color.White.copy(alpha = 0.16f), shape),
            )
        }
        Text(clock(seconds), style = Times, color = palette.text)
    }
}

/** A button of the player: its icon alone, lit in the accent when the remote
 *  is on it. */
@Composable
private fun PlayerButton(icon: ImageVector, onClick: () -> Unit, modifier: Modifier = Modifier, large: Boolean = false) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val side = if (large) 58.dp else 48.dp
    val shape = RoundedCornerShape(8.dp)
    Box(
        modifier = modifier
            .size(side)
            .grows({ lit }, 1.06f)
            .clickable(interaction, indication = null, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .fillMaxSize()
                .padding(5.dp)
                .clip(shape)
                .background(palette.accent.copy(alpha = 0.16f * lit))
                .border(2.dp, palette.accent.copy(alpha = lit), shape),
        )
        Glyph(icon, if (lit > 0.5f) palette.accentBright else palette.text, if (large) 34.dp else 27.dp)
    }
}

/** The list of the film's audio tracks or subtitles, the one playing ticked. */
@Composable
fun TrackMenu(menu: Menu, playback: Playback, onClose: () -> Unit, modifier: Modifier) {
    val plan = playback.plan ?: return
    when (menu) {
        Menu.Audio -> ChoiceList(
            title = stringResource(R.string.tv_audio),
            choices = plan.audio.mapIndexed { at, track -> trackName(track, at, withSound = true) },
            chosen = plan.audio.indexOfFirst { it.id == playback.audio }.takeIf { it >= 0 },
            onChoose = { playback.chooseAudio(plan.audio[it]) },
            onClose = onClose,
            modifier = modifier.padding(end = Gutter, bottom = 124.dp),
        )
        Menu.Subtitles -> ChoiceList(
            title = stringResource(R.string.tv_subtitles),
            choices = listOf(stringResource(R.string.tv_subtitles_off)) +
                plan.subtitles.mapIndexed { at, track -> trackName(track, at, withSound = false) },
            chosen = if (playback.subtitle == null) 0 else plan.subtitles.indexOfFirst { it.id == playback.subtitle } + 1,
            onChoose = { playback.chooseSubtitle(if (it == 0) null else plan.subtitles[it - 1]) },
            onClose = onClose,
            modifier = modifier.padding(end = Gutter, bottom = 124.dp),
        )
    }
}

/** A track as a viewer tells it apart: its language, then what the sound is
 *  made of, then its own title. */
@Composable
private fun trackName(track: Track, at: Int, withSound: Boolean): String {
    val shownIn = LocalConfiguration.current.locales[0]
    val parts = listOfNotNull(
        languageName(track.language, shownIn),
        if (withSound) listOfNotNull(soundName(track.codec), speakersName(track.channels)).joinToString(" ").ifBlank { null } else null,
        track.title?.takeIf { it.isNotBlank() },
    )
    return parts.joinToString(" · ").ifEmpty { stringResource(R.string.tv_track_unnamed, at + 1) }
}
