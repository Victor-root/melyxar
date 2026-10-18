package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Chapter
import app.melyxar.server.Plan
import app.melyxar.tv.clock
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.WideWidth
import app.melyxar.tv.play.FetchingThumbnails
import app.melyxar.tv.play.Thumbnail
import kotlin.math.ceil

/** The distances an invented chapter may stand at, in seconds, from the
 *  shortest: round figures, so the times on the cards read as a rhythm. */
private val ROUND_STEPS = doubleArrayOf(300.0, 600.0, 900.0, 1200.0, 1800.0, 3600.0)

/** The most cards a row of them is allowed, and the fewest that make a row. */
private const val MOST_CARDS = 12
private const val FEWEST_CARDS = 3

/** How close to the end of the film a moment may not be: a card on the
 *  credits' last seconds shows nothing worth a press. */
private const val CLEAR_OF_THE_END = 60.0

/** Where invented chapters stand along a film of this length, as the web's
 *  page lays them: at its start and then every round distance, the shortest
 *  that keeps them to a row of a reasonable size. None where the film is too
 *  short to make a row. */
fun inventedMoments(length: Double): List<Double> {
    if (!(length > 0)) {
        return emptyList()
    }
    val step = ROUND_STEPS.firstOrNull { ceil(length / it) <= MOST_CARDS } ?: ROUND_STEPS.last()
    val moments = generateSequence(0.0) { it + step }.takeWhile { it < length - CLEAR_OF_THE_END }.toList()
    return if (moments.size >= FEWEST_CARDS) moments else emptyList()
}

/** How long the film is, as far as the plan knows: the pictures cover the
 *  film as far as they go, and the length the file says is kept when it is
 *  shorter. */
private fun lengthOf(plan: Plan): Double {
    val covered = plan.thumbnails?.let { it.counted * it.everySeconds } ?: 0.0
    val said = plan.durationMinutes?.let { it * 60 } ?: 0.0
    return if (said > 0 && covered > 0) minOf(said, covered) else maxOf(said, covered)
}

/** The chapters a film's page shows: those its file names, or invented ones
 *  when it names none and its pictures are there to put on them. */
fun chaptersShownOf(plan: Plan): List<Chapter> = when {
    plan.chapters.isNotEmpty() -> plan.chapters
    plan.thumbnails == null -> emptyList()
    else -> inventedMoments(lengthOf(plan)).map { Chapter(it) }
}

/** The thumbnails of the chapters of the film of [source] fetched as its
 *  page opens, rather than when the remote reaches their row. */
@Composable
fun ChaptersAhead(source: String) {
    val plan = rememberLoaded("plan:$source") { planOf(source) }.value.ready ?: return
    val thumbnails = plan.thumbnails ?: return
    FetchingThumbnails(thumbnails, chaptersShownOf(plan).map { it.atSecond })
}

/**
 * The places the film of [source] changes scene, as a row of stills, as the
 * web's page shows them: each with the little picture of its moment, cut out
 * of the sheets the bar of the player already uses. A press starts the film
 * there.
 */
@Composable
fun WorkChapters(source: String, memory: FocusMemory, onPlay: (Double) -> Unit) {
    val plan = rememberLoaded("plan:$source") { planOf(source) }.value.ready ?: return
    val chapters = chaptersShownOf(plan)
    if (chapters.isEmpty()) {
        return
    }
    val palette = LocalPalette.current
    val thumbnails = plan.thumbnails
    val across = WideWidth
    val down = across * 9 / 16
    CardRow(stringResource(R.string.tv_chapters), Icons.Film, chapters.withIndex().toList(), key = { it.value.atSecond.toString() }) { (at, chapter) ->
        PictureCard(
            across = across,
            down = down,
            title = chapter.title ?: stringResource(R.string.tv_chapter_number, at + 1),
            note = clock(chapter.atSecond),
            trailing = null,
            ground = palette.surface,
            onClick = { onPlay(chapter.atSecond) },
            modifier = Modifier.remembered(memory, "chapter/${chapter.atSecond}"),
        ) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                if (thumbnails != null && thumbnails.height > 0) {
                    // The film's own shape inside the card's, with bands where
                    // they differ, as a screen shows a film.
                    Thumbnail(thumbnails, chapter.atSecond, minOf(across, down * thumbnails.width / thumbnails.height))
                } else {
                    Text((at + 1).toString(), fontSize = 28.sp, fontWeight = FontWeight.SemiBold, color = palette.textFaint)
                }
            }
        }
    }
}
