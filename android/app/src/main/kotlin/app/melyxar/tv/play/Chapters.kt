package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Chapter
import app.melyxar.tv.clock
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.litBy

/** How far apart the marks of a film that names no scenes stand. */
private const val A_STEP_OF_S = 300.0

/** How far back a moment may be read as the start of the chapter after it:
 *  a film sent to a chapter lands a hair before its first frame. */
private const val LANDED_WITHIN_S = 0.25

/** How wide a card of the strip is. */
private val CARD_ACROSS = 200.dp

/**
 * The chapters of a film, as the web's strip offers them: those its file
 * names, or marks every few minutes for one that names none, since the strip
 * is still the quickest way through it.
 */
fun chaptersOf(named: List<Chapter>, length: Double?): List<Chapter> = when {
    named.isNotEmpty() -> named
    length == null || length <= A_STEP_OF_S -> emptyList()
    else -> generateSequence(0.0) { it + A_STEP_OF_S }.takeWhile { it < length }.map { Chapter(it) }.toList()
}

/** Which chapter a moment falls inside, or -1 before the first. */
fun chapterAt(chapters: List<Chapter>, at: Double): Int = chapters.indexOfLast { it.atSecond <= at + LANDED_WITHIN_S }

/**
 * The chapters along the foot of the film, each with its picture, its name
 * and when it starts, the one playing marked: the remote starts on it, and
 * a press takes the film to the one it is on.
 */
@Composable
fun ChapterStrip(playback: Playback, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val plan = playback.plan ?: return
    val chapters = chaptersOf(plan.chapters, playback.length)
    val playing = remember { chapterAt(chapters, playback.seconds) }
    val start = remember { FocusRequester() }
    val opening = playing.coerceAtLeast(0)
    BackHandler(onBack = onClose)
    Column(
        modifier = modifier
            .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.88f))))
            .padding(top = 60.dp, bottom = 28.dp)
            .focusProperties { onExit = { cancelFocusChange() } }
            .focusGroup(),
    ) {
        Text(
            stringResource(R.string.tv_chapters),
            fontSize = 18.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            modifier = Modifier.padding(start = Gutter, bottom = 14.dp),
        )
        LazyRow(
            state = rememberLazyListState(initialFirstVisibleItemIndex = opening),
            contentPadding = PaddingValues(horizontal = Gutter),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            itemsIndexed(chapters, key = { _, chapter -> chapter.atSecond }) { at, chapter ->
                ChapterCard(
                    chapter = chapter,
                    name = chapter.title ?: stringResource(R.string.tv_chapter_number, at + 1),
                    playing = at == playing,
                    playback = playback,
                    onClick = {
                        playback.goTo(chapter.atSecond)
                        onClose()
                    },
                    modifier = if (at == opening) Modifier.focusRequester(start) else Modifier,
                )
            }
        }
    }
    LaunchedEffect(Unit) { start.requestFocus() }
}

@Composable
private fun ChapterCard(
    chapter: Chapter,
    name: String,
    playing: Boolean,
    playback: Playback,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(8.dp)
    val thumbnails = playback.plan?.thumbnails
    Column(
        modifier = modifier
            .width(CARD_ACROSS)
            .clickable(interaction, indication = null, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Box(
            Modifier
                .grows({ lit }, 1.04f)
                .clip(shape)
                .background(Color.Black)
                .border(2.dp, palette.accent.copy(alpha = if (playing) 1f else lit), shape),
        ) {
            // A film nobody has read for pictures keeps a frame of a film's
            // shape, which is what an empty scene card should look like.
            if (thumbnails != null) {
                Thumbnail(thumbnails, chapter.atSecond, CARD_ACROSS)
            } else {
                Box(Modifier.width(CARD_ACROSS).height(CARD_ACROSS * 9 / 16))
            }
            if (playing) {
                Text(
                    stringResource(R.string.tv_playing_now),
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.accentContrast,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .background(palette.accent, RoundedCornerShape(topEnd = 8.dp))
                        .padding(horizontal = 7.dp, vertical = 3.dp),
                )
            }
        }
        Text(
            name,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            color = if (lit > 0.5f) palette.accentBright else palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 4.dp),
        )
        Text(clock(chapter.atSecond), fontSize = 13.sp, color = palette.textFaint, maxLines = 1)
    }
}
