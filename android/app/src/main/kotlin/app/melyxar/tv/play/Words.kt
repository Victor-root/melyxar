package app.melyxar.tv.play

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.media3.common.C
import androidx.media3.common.text.Cue
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.text.CuesWithTiming
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.extractor.text.webvtt.WebvttParser
import app.melyxar.R
import kotlin.math.roundToInt
import kotlinx.coroutines.delay

/** How often the words are matched to the film. */
private const val WORDS_TICK_MS = 50L

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
 * How far up the screen what stands at the foot of the film reaches, the
 * controls or a sheet over them, in pixels, without the shading above it:
 * read where it is drawn, as the web measures its bar of controls, for the
 * words to stand clear of it.
 */
@Stable
class Foot {
    var reach by mutableIntStateOf(0)
}

val LocalFoot = staticCompositionLocalOf { Foot() }

/** Says how far up this, standing at the foot of the film, reaches: its
 *  height but the [shading] at its top, which is meant to be seen through. */
@Composable
fun Modifier.standsAtTheFoot(shading: Dp): Modifier {
    val foot = LocalFoot.current
    val shaded = with(LocalDensity.current) { shading.roundToPx() }
    return onSizeChanged { foot.reach = it.height - shaded }
}

/** The space between two lines of words, and around the words inside the
 *  box a background draws, as the web's. */
private val BETWEEN_LINES = 2.dp
private val BOX_ACROSS = 8.dp
private val BOX_DOWN = 2.dp

/** The widest the words may run, as a share of the screen's width. */
private const val WIDEST = 0.9f

/** The lines shown, each cue's text cut at its own line ends: the words
 *  alone, laid out by Melyxar whatever the file said of where or how, as
 *  the web draws them. */
private fun linesOf(cues: List<Cue>): List<String> = cues.flatMap { it.text?.toString().orEmpty().split('\n') }

/**
 * The words of the film over it, as the web player draws them: in the look
 * chosen on this television, centred at the height chosen, moved by the
 * offset the viewer chose, and standing clear of the controls while they
 * cover the foot of the film, [lift] pixels from the bottom of the screen.
 */
@Composable
fun WordsOverFilm(playback: Playback, look: WordsLook, lift: Float?, modifier: Modifier = Modifier) {
    val words = playback.words
    var shown by remember { mutableStateOf<List<String>>(emptyList()) }
    LaunchedEffect(words) {
        // Without words, nothing to follow the film for.
        if (words.isEmpty()) {
            shown = emptyList()
            return@LaunchedEffect
        }
        var cues: List<Cue>? = null
        while (true) {
            val at = playback.player.currentPosition * 1000 - (playback.wordsOffset * 1_000_000).toLong()
            val now = words.cuesAt(at)
            if (now !== cues) {
                shown = linesOf(now)
                cues = now
            }
            delay(WORDS_TICK_MS)
        }
    }
    if (shown.isEmpty()) {
        return
    }
    BoxWithConstraints(modifier) {
        val screen = constraints.maxHeight.toFloat()
        val density = LocalDensity.current
        // The web's CSS pixel, which an edge and a shadow are measured in.
        val pixel = with(density) { 1.dp.toPx() }
        val style = wordsStyle(look, with(density) { (look.size.ofHeight * screen).toSp() }, pixel)
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(BETWEEN_LINES),
            modifier = Modifier
                .wordsPlaced(screen, look.share, lift)
                .widthIn(max = maxWidth * WIDEST),
        ) {
            shown.forEach { line -> WordsLine(line, look, style, pixel) }
        }
    }
}

/** How large the sample's words are at the usual size, the size of the
 *  page's own text, as the web's sample. */
private val SAMPLE_WORDS = 20.sp

/**
 * One line of words in the look chosen, standing on a dark picture at the
 * height chosen, for a page of settings to show what each choice does.
 */
@Composable
fun WordsSample(look: WordsLook, line: String, modifier: Modifier = Modifier) {
    val pixel = with(LocalDensity.current) { 1.dp.toPx() }
    val style = wordsStyle(look, SAMPLE_WORDS * (look.size.ofHeight / WordsSize.Normal.ofHeight), pixel)
    BoxWithConstraints(
        modifier
            .clip(RoundedCornerShape(10.dp))
            .background(Brush.linearGradient(0f to Color(0xFF1C2530), 0.6f to Color(0xFF05070A), 1f to Color(0xFF241A12))),
    ) {
        Box(Modifier.wordsPlaced(constraints.maxHeight.toFloat(), look.share, lift = null)) {
            WordsLine(line, look, style, pixel)
        }
    }
}

/** Puts the words across the middle of the screen and at their height. */
private fun Modifier.wordsPlaced(screen: Float, share: Float, lift: Float?) = layout { measurable, constraints ->
    val block = measurable.measure(constraints.copy(minWidth = 0, minHeight = 0))
    layout(constraints.maxWidth, constraints.maxHeight) {
        val across = (constraints.maxWidth - block.width) / 2
        block.place(across, wordsTop(screen, block.height.toFloat(), share, lift).roundToInt())
    }
}

/** Inter, in the weight of the web's words. */
private val Letters = FontFamily(Font(R.font.inter_regular))

/** The letters of the words: Inter as the web's, at the chosen size, in the
 *  chosen colour, a line and a half apart. */
private fun wordsStyle(look: WordsLook, size: TextUnit, pixel: Float) = TextStyle(
    color = Color(look.colour.argb),
    fontFamily = Letters,
    fontSize = size,
    lineHeight = 1.5.em,
    textAlign = TextAlign.Center,
    shadow = if (look.edge == WordsEdge.Shadow) Shadow(Color.Black, Offset(2 * pixel, 2 * pixel), 3 * pixel) else null,
)

/** One line of words, in its own box when a background is chosen, outlined
 *  in black when the outline is. */
@Composable
private fun WordsLine(line: String, look: WordsLook, style: TextStyle, pixel: Float) {
    Box(
        modifier = Modifier
            .then(look.background.argb?.let { Modifier.background(Color(it)) } ?: Modifier)
            .padding(horizontal = BOX_ACROSS, vertical = BOX_DOWN),
    ) {
        if (look.edge == WordsEdge.Outline) {
            // Drawn under the letters, a pixel out on every side, as the
            // web's four black shadows draw it.
            Text(line, style = style.copy(color = Color.Black, drawStyle = Stroke(width = 2 * pixel, join = StrokeJoin.Round)))
        }
        Text(line, style = style)
    }
}
