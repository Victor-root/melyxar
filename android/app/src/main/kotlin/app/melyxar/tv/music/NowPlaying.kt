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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
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
    // On the queue for music started anew, on the side left otherwise.
    var side by rememberSaveable(listening.begun) { mutableStateOf(Side.Queue) }
    Column(Modifier.fillMaxSize().focusTraced("song page")) {
        TopBar()
        if (song == null) {
            return@Column
        }
        Row(
            modifier = Modifier.weight(1f).padding(start = Gutter, end = Gutter, top = 22.dp),
            horizontalArrangement = Arrangement.spacedBy(48.dp),
        ) {
            Column(Modifier.width(COVER), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(
                    Modifier
                        .size(COVER)
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
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 12.dp),
                )
                Text(artistsOf(song.artists), fontSize = 16.sp, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
                song.album?.let { Text(it.name, fontSize = 14.sp, color = palette.textFaint, maxLines = 1, overflow = TextOverflow.Ellipsis) }
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
                Controls(listening, song.id, first, Modifier.align(Alignment.CenterHorizontally).padding(top = 8.dp))
            }
        }
    }
    LaunchedEffect(song != null) { if (song != null) first.requestFocus() }
}

/** What stands beside the song: what comes after it, or its words. */
private enum class Side(@param:StringRes val label: Int) {
    Queue(R.string.tv_music_queue),
    Lyrics(R.string.tv_music_lyrics),
}

/** The song playing and what comes after it, only shown: the player's
 *  buttons are what moves along it. */
@Composable
private fun Coming(listening: Listening, modifier: Modifier) {
    val queue = listening.queue
    val list = rememberLazyListState()
    LazyColumn(state = list, modifier = modifier) {
        itemsIndexed(queue.order.drop(queue.at), key = { at, place -> "$at/$place" }) { at, place ->
            SongLine(
                queue.songs[place],
                number = if (at == 0) null else "$at",
                onClick = null,
            )
        }
    }
}

/** The buttons that lead the music, the remote starting on the one that
 *  plays and pauses it. */
@Composable
private fun Controls(listening: Listening, song: String, first: FocusRequester, modifier: Modifier) {
    val chrome = LocalChrome.current
    val hearts = LocalHearts.current
    val queue = listening.queue
    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier.focusRestorer(first),
    ) {
        RoundMark(Icons.Shuffle, label = stringResource(R.string.tv_music_shuffle), on = queue.shuffle, onClick = listening::shuffle)
        RoundMark(Icons.PreviousSong, label = stringResource(R.string.tv_music_previous), on = false, onClick = listening::previous)
        RoundMark(
            Icons.Restart,
            label = stringResource(R.string.tv_music_skip_back, listening.preferences.skipBackSeconds),
            on = false,
            onClick = { listening.skip(on = false) },
        )
        RoundMark(
            if (listening.playing) Icons.Pause else Icons.Play,
            label = stringResource(if (listening.playing) R.string.tv_music_pause else R.string.tv_music_play),
            on = true,
            onClick = listening::toggle,
            modifier = Modifier.focusRequester(first),
        )
        RoundMark(
            Icons.StepOn,
            label = stringResource(R.string.tv_music_skip_on, listening.preferences.skipOnSeconds),
            on = false,
            onClick = { listening.skip(on = true) },
        )
        RoundMark(Icons.NextSong, label = stringResource(R.string.tv_music_next), on = false, onClick = listening::next)
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
        Heart(song, hearts)
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
    val thick by animateDpAsState(if (lit > 0.5f) 10.dp else 6.dp, label = "rail")
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
                    val line = thick.toPx()
                    val top = (size.height - line) / 2
                    val round = CornerRadius(line / 2)
                    drawRoundRect(palette.text.copy(alpha = 0.2f), Offset(0f, top), Size(size.width, line), round)
                    drawRoundRect(palette.accent, Offset(0f, top), Size(size.width * done, line), round)
                },
        )
        Row {
            Text(clock(listening.position), fontSize = 13.sp, color = palette.textFaint, modifier = Modifier.weight(1f))
            Text(length?.let(::clock).orEmpty(), fontSize = 13.sp, color = palette.textFaint)
        }
    }
}
