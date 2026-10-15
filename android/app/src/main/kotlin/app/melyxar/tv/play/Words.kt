package app.melyxar.tv.play

import android.view.View
import androidx.annotation.OptIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.res.ResourcesCompat
import androidx.media3.common.C
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.text.CuesWithTiming
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.extractor.text.webvtt.WebvttParser
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView
import app.melyxar.R
import kotlinx.coroutines.delay

/** How often the words are matched to the film. */
private const val WORDS_TICK_MS = 50L

/** How far up the words stand while the controls cover the foot of the
 *  film, as a share of its height. */
private const val ABOVE_THE_CONTROLS = 0.3f

/** One stretch of the film over which the same words are shown. */
class Stretch(val fromUs: Long, val untilUs: Long, val cues: List<Cue>)

/**
 * The cues of a subtitle laid end to end: where several overlap, each
 * stretch between two of their edges shows all of those over it, so the
 * words of any moment are one look away.
 */
@OptIn(UnstableApi::class)
fun stretchesOf(timings: List<CuesWithTiming>): List<Stretch> {
    val edges = timings.flatMap { listOf(it.startTimeUs, it.endTimeUs) }.filter { it != C.TIME_UNSET }.distinct().sorted()
    val starting = timings.sortedBy { it.startTimeUs }
    val over = mutableListOf<CuesWithTiming>()
    var next = 0
    val stretches = mutableListOf<Stretch>()
    for (at in 0 until edges.size - 1) {
        val from = edges[at]
        while (next < starting.size && starting[next].startTimeUs <= from) {
            over += starting[next]
            next += 1
        }
        over.removeAll { it.endTimeUs != C.TIME_UNSET && it.endTimeUs <= from }
        if (over.isNotEmpty()) {
            stretches += Stretch(from, edges[at + 1], over.flatMap { it.cues })
        }
    }
    return stretches
}

/** The words shown [atUs] into the film, none between two stretches. */
fun List<Stretch>.cuesAt(atUs: Long): List<Cue> {
    var low = 0
    var high = size - 1
    var found = -1
    while (low <= high) {
        val middle = (low + high) / 2
        if (this[middle].fromUs <= atUs) {
            found = middle
            low = middle + 1
        } else {
            high = middle - 1
        }
    }
    return getOrNull(found)?.takeIf { atUs < it.untilUs }?.cues ?: emptyList()
}

/** A subtitle of words as the server hands it over, read into its cues. */
@OptIn(UnstableApi::class)
fun wordsOf(file: ByteArray): List<Stretch> {
    val timings = mutableListOf<CuesWithTiming>()
    WebvttParser().parse(file, 0, file.size, SubtitleParser.OutputOptions.allCues()) { timings += it }
    return stretchesOf(timings)
}

/**
 * The words of the film over it, in Melyxar's letters, moved by the offset
 * the viewer chose, and raised above the controls while they are up.
 */
@OptIn(UnstableApi::class)
@Composable
fun WordsOverFilm(playback: Playback, raised: Boolean, modifier: Modifier = Modifier) {
    var view by remember { mutableStateOf<SubtitleView?>(null) }
    val words = playback.words
    AndroidView(
        factory = { context ->
            SubtitleView(context).apply {
                setStyle(
                    CaptionStyleCompat(
                        android.graphics.Color.WHITE,
                        android.graphics.Color.TRANSPARENT,
                        android.graphics.Color.TRANSPARENT,
                        CaptionStyleCompat.EDGE_TYPE_DROP_SHADOW,
                        android.graphics.Color.BLACK,
                        ResourcesCompat.getFont(context, R.font.inter_semibold),
                    ),
                )
                isFocusable = false
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            }.also { view = it }
        },
        update = {
            it.setBottomPaddingFraction(if (raised) ABOVE_THE_CONTROLS else SubtitleView.DEFAULT_BOTTOM_PADDING_FRACTION)
        },
        modifier = modifier,
    )
    LaunchedEffect(words, view) {
        val shownOn = view ?: return@LaunchedEffect
        var shown: List<Cue>? = null
        while (true) {
            val at = playback.player.currentPosition * 1000 - (playback.wordsOffset * 1_000_000).toLong()
            val now = words.cuesAt(at)
            if (now !== shown) {
                shownOn.setCues(now)
                shown = now
            }
            delay(WORDS_TICK_MS)
        }
    }
}
