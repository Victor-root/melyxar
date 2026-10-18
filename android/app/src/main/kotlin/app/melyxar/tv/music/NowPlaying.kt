package app.melyxar.tv.music

import androidx.annotation.StringRes
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.tv.clock
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.focusTraced
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.GlassBar
import app.melyxar.tv.look.GlassTab
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.RoundMark
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.litPast
import app.melyxar.tv.look.rail
import app.melyxar.tv.look.seekHandle
import app.melyxar.tv.look.shadowRaised

/** How wide the cover of the song playing is. */
private val COVER = 270.dp

/**
 * The song playing, as the web's page of it on a large screen: the cover
 * large with what the song is beside the queue or the words, a tab choosing
 * which, and along the foot the player, the song's wave behind it.
 */
@Composable
fun NowPlayingScreen() {
    val listening = LocalListening.current
    val palette = LocalPalette.current
    val song = listening.queue.current
    val first = remember { FocusRequester() }
    var side by rememberSaveable { mutableStateOf(Side.Queue) }
    Column(Modifier.fillMaxSize().focusTraced("song page")) {
        TopBar()
        if (song == null) {
            return@Column
        }
        Row(
            modifier = Modifier.weight(1f).padding(start = Gutter, end = Gutter, top = 22.dp),
            horizontalArrangement = Arrangement.spacedBy(48.dp),
        ) {
            // The cover gives up its room to the words under it, which are
            // all shown, as the web's: a long name makes it smaller.
            Column(
                Modifier.width(COVER).fillMaxHeight(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(
                    Modifier
                        .weight(1f, fill = false)
                        .aspectRatio(1f, matchHeightConstraintsFirst = true)
                        .boxShadow(CardShape, palette.shadowRaised)
                        .clip(CardShape)
                        .background(palette.surfaceRaised),
                ) {
                    ServerPicture(song.cover, COVER, Modifier.fillMaxSize())
                }
                Text(
                    song.title,
                    fontSize = 24.sp,
                    lineHeight = 28.sp,
                    fontWeight = FontWeight.Bold,
                    color = palette.text,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 12.dp),
                )
                Text(artistsOf(song.artists), fontSize = 16.sp, color = palette.textSoft, textAlign = TextAlign.Center)
                song.album?.let { Text(it.name, fontSize = 14.sp, color = palette.textFaint, textAlign = TextAlign.Center) }
            }
            Column(Modifier.weight(1f)) {
                GlassBar {
                    Side.entries.forEach { one ->
                        GlassTab(stringResource(one.label), on = side == one, onClick = { side = one })
                    }
                }
                val beside = Modifier.fillMaxWidth().weight(1f).padding(top = 16.dp)
                when (side) {
                    Side.Queue -> Coming(listening, beside)
                    Side.Lyrics -> LyricsPanel(song.id, beside)
                }
            }
        }
        Box(Modifier.fillMaxWidth()) {
            SongWave(Modifier.matchParentSize())
            Column(Modifier.padding(start = Gutter, end = Gutter, top = 30.dp, bottom = 22.dp)) {
                Rail(listening)
                Controls(listening, song.id, first, Modifier.padding(top = 8.dp))
            }
        }
    }
}

/** What stands beside the song: what comes after it, or its words. */
private enum class Side(@param:StringRes val label: Int) {
    Queue(R.string.tv_music_queue),
    Lyrics(R.string.tv_music_lyrics),
}

/** The whole queue, as the web's: each song by its place in it, those
 *  played faded, the one playing in the accent and brought to the top;
 *  only shown, the player's buttons being what moves along it. */
@Composable
private fun Coming(listening: Listening, modifier: Modifier) {
    val queue = listening.queue
    val list = rememberLazyListState(initialFirstVisibleItemIndex = queue.at)
    LaunchedEffect(queue.at) { list.animateScrollToItem(queue.at) }
    LazyColumn(state = list, modifier = modifier) {
        itemsIndexed(queue.order, key = { at, place -> "$at/$place" }) { at, place ->
            SongLine(
                queue.songs[place],
                number = "${at + 1}",
                onClick = null,
                modifier = if (at < queue.at) Modifier.alpha(PLAYED) else Modifier,
                afterAnother = at > 0,
            )
        }
    }
}

/** How faded a song already played stands in the queue. */
private const val PLAYED = 0.5f

/** The buttons that lead the music, laid as the phone's player lays them:
 * the large disc that plays and pauses in the middle, a step back and on
 * either side of it, then the song before and after; the ways it plays at
 * the left, the heart and the stop at the right, well apart. The remote
 * comes into them on the disc; it is left where it was as the page opens,
 * on the bar it was opened from. */
@Composable
private fun Controls(listening: Listening, song: String, first: FocusRequester, modifier: Modifier) {
    val chrome = LocalChrome.current
    val hearts = LocalHearts.current
    val queue = listening.queue
    val skips = listening.preferences
    Box(modifier.fillMaxWidth().focusRestorer(first)) {
        Row(Modifier.align(Alignment.CenterStart), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            RoundMark(Icons.Shuffle, label = stringResource(R.string.tv_music_shuffle), on = queue.shuffle, onClick = listening::shuffle)
            RoundMark(
                if (queue.repeat == Repeat.One) Icons.RepeatOne else Icons.Repeat,
                label = stringResource(
                    when (queue.repeat) {
                        Repeat.Off -> R.string.tv_music_repeat_off
                        Repeat.All -> R.string.tv_music_repeat_all
                        Repeat.One -> R.string.tv_music_repeat_one
                    },
                ),
                on = queue.repeat != Repeat.Off,
                onClick = listening::repeat,
            )
        }
        Row(
            Modifier.align(Alignment.Center),
            horizontalArrangement = Arrangement.spacedBy(18.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RoundMark(Icons.PreviousSong, label = stringResource(R.string.tv_music_previous), on = false, onClick = listening::previous)
            Step(Icons.Restart, skips.skipBackSeconds, stringResource(R.string.tv_music_skip_back, skips.skipBackSeconds)) { listening.skip(on = false) }
            RoundMark(
                if (listening.playing) Icons.Pause else Icons.Play,
                label = stringResource(if (listening.playing) R.string.tv_music_pause else R.string.tv_music_play),
                on = true,
                onClick = listening::toggle,
                modifier = Modifier.focusRequester(first),
                main = true,
            )
            Step(Icons.StepOn, skips.skipOnSeconds, stringResource(R.string.tv_music_skip_on, skips.skipOnSeconds)) { listening.skip(on = true) }
            RoundMark(Icons.NextSong, label = stringResource(R.string.tv_music_next), on = false, onClick = listening::next)
        }
        Row(Modifier.align(Alignment.CenterEnd), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Heart(song, hearts)
            // Stopped, there is nothing left for this page to show.
            RoundMark(
                Icons.Stop,
                label = stringResource(R.string.tv_music_stop),
                on = false,
                onClick = {
                    listening.stop()
                    chrome.back()
                },
            )
        }
    }
}

/** A step back or on through the song, by [seconds], said inside its arc
 *  as the film player says it. */
@Composable
private fun Step(arc: ImageVector, seconds: Int, label: String, onClick: () -> Unit) {
    Box(contentAlignment = Alignment.Center) {
        RoundMark(arc, label = label, on = false, onClick = onClick)
        Text("$seconds", fontSize = 9.sp, fontWeight = FontWeight.Bold, color = LocalPalette.current.text, modifier = Modifier.padding(top = 3.dp))
    }
}

/**
 * Where the song is and how long it lasts, the remote's arrows moving it
 * back and on by the account's steps while the remote is on the bar.
 */
@Composable
private fun Rail(listening: Listening, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val thick by animateDpAsState(if (litPast({ lit })) 10.dp else 6.dp, label = "rail")
    val length = listening.length
    val done = length?.takeIf { it > 0 }?.let { (listening.position / it).toFloat().coerceIn(0f, 1f) } ?: 0f
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(22.dp)
                .onPreviewKeyEvent { event ->
                    if (event.type != KeyEventType.KeyDown) {
                        return@onPreviewKeyEvent false
                    }
                    when (event.key) {
                        Key.DirectionLeft -> listening.skip(on = false)
                        Key.DirectionRight -> listening.skip(on = true)
                        else -> return@onPreviewKeyEvent false
                    }
                    true
                }
                .focusable(interactionSource = interaction)
                .drawBehind {
                    rail(thick.toPx(), palette.text.copy(alpha = 0.2f), palette.accent to done)
                    seekHandle(palette.text, done, lit)
                },
        )
        Row {
            Text(clock(listening.position), fontSize = 13.sp, color = palette.textFaint, modifier = Modifier.weight(1f))
            Text(length?.let(::clock).orEmpty(), fontSize = 13.sp, color = palette.textFaint)
        }
    }
}
