package app.melyxar.tv.music

import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.LyricLine
import app.melyxar.server.Refusal
import app.melyxar.server.SongLyrics
import app.melyxar.server.lyrics
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import java.io.IOException
import kotlinx.coroutines.launch

/** What was read of a song's words: nothing when it has none. */
private class Read(val lyrics: SongLyrics?)

/** How far one press of an arrow moves words written without moments. */
private val PRESS_SCROLLS = 120.dp

/**
 * The words of [song], as the web's page of what is playing shows them:
 * stamped words follow the song, the line being sung lit and brought up a
 * third of the way down; plain words are shown as they are, the arrows
 * moving through them.
 */
@Composable
fun LyricsPanel(song: String, modifier: Modifier) {
    val api = LocalServer.current
    val read by produceState<Read?>(null, song) {
        value = null
        value = Read(
            try {
                api.lyrics(song)
            } catch (_: IOException) {
                null
            } catch (_: Refusal) {
                null
            },
        )
    }
    val lyrics = read?.lyrics
    when {
        read == null -> Note(stringResource(R.string.tv_music_lyrics_loading), modifier)
        lyrics == null || (lyrics.instrumental && lyrics.plain.isEmpty()) -> Note(
            stringResource(if (lyrics?.instrumental == true) R.string.tv_music_lyrics_instrumental else R.string.tv_music_lyrics_none),
            modifier,
        )
        lyrics.lines.isEmpty() -> Plain(lyrics.plain, modifier)
        else -> Following(lyrics.lines, modifier)
    }
}

@Composable
private fun Note(text: String, modifier: Modifier) {
    Text(text, fontSize = 16.sp, color = LocalPalette.current.textSoft, modifier = modifier)
}

/** Words without moments, as written; the arrows move through them. */
@Composable
private fun Plain(text: String, modifier: Modifier) {
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    val press = with(LocalDensity.current) { PRESS_SCROLLS.toPx() }
    Text(
        text,
        fontSize = 20.sp,
        lineHeight = 32.sp,
        color = LocalPalette.current.text,
        modifier = modifier
            .onPreviewKeyEvent { event ->
                val by = when (event.key) {
                    Key.DirectionDown -> press
                    Key.DirectionUp -> -press
                    else -> return@onPreviewKeyEvent false
                }
                // At either end, the arrow goes on to what is beyond.
                val moves = if (by > 0) scroll.canScrollForward else scroll.canScrollBackward
                if (moves && event.type == KeyEventType.KeyDown) {
                    scope.launch { scroll.animateScrollBy(by) }
                }
                moves
            }
            .focusable()
            .verticalScroll(scroll),
    )
}

/** Stamped words: the line being sung lit, and kept a third of the way down. */
@Composable
private fun Following(lines: List<LyricLine>, modifier: Modifier) {
    val palette = LocalPalette.current
    val listening = LocalListening.current
    val list = rememberLazyListState()
    val active by remember(lines) { derivedStateOf { litLineAt(lines, listening.position) } }
    LaunchedEffect(active) {
        if (active >= 0) {
            list.animateScrollToItem(active, -list.layoutInfo.viewportSize.height / 3)
        }
    }
    LazyColumn(
        state = list,
        userScrollEnabled = false,
        contentPadding = PaddingValues(bottom = 200.dp),
        modifier = modifier,
    ) {
        itemsIndexed(lines) { at, line ->
            Text(
                // A line left empty is a pause in the singing.
                line.text.ifBlank { "♪" },
                fontSize = 22.sp,
                lineHeight = 30.sp,
                fontWeight = if (at == active) FontWeight.Bold else FontWeight.SemiBold,
                color = if (at == active) palette.text else palette.textFaint,
                modifier = Modifier.padding(vertical = 7.dp),
            )
        }
    }
}
