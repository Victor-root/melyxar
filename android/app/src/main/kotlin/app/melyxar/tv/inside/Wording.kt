package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import app.melyxar.R
import app.melyxar.tv.durationOf
import app.melyxar.tv.minutesLeft

/*
 * The numbers of `Readable.kt` in the words of the screen's language.
 */

/** A length in minutes as a person says it: 48 min, 2 h, 2 h 14 min. */
@Composable
fun howLong(minutes: Int): String {
    val length = durationOf(minutes)
    return when {
        length.hours == 0 -> stringResource(R.string.tv_minutes, length.minutes)
        length.minutes == 0 -> stringResource(R.string.tv_hours, length.hours)
        else -> stringResource(R.string.tv_hours_minutes, length.hours, length.minutes)
    }
}

/** "1 h 26 remaining", or nothing when nothing is left or the length is not
 *  known. */
@Composable
fun whatIsLeft(seconds: Double, length: Long?): String? =
    minutesLeft(seconds, length)?.let { stringResource(R.string.tv_left, howLong(it)) }

/** "S1E3 · The title", for an episode placed in its series. */
@Composable
fun whichEpisode(season: Int?, episode: Int?, title: String): String? {
    if (season == null || episode == null) {
        return null
    }
    return "${stringResource(R.string.tv_episode_short, season, episode)} · $title"
}

/** The name of a kind of library. */
@Composable
fun kindName(kind: String): String = stringResource(
    when (kind) {
        "series" -> R.string.tv_kind_series
        "anime" -> R.string.tv_kind_anime
        "shows" -> R.string.tv_kind_shows
        "home_media" -> R.string.tv_kind_home_media
        "music" -> R.string.tv_kind_music
        else -> R.string.tv_kind_movies
    },
)

/** The heading of the row of the newest works of a kind. */
@Composable
fun newestOfKind(kind: String): String = stringResource(
    when (kind) {
        "series" -> R.string.tv_newest_series
        "anime" -> R.string.tv_newest_anime
        "shows" -> R.string.tv_newest_shows
        "home_media" -> R.string.tv_newest_home_media
        "music" -> R.string.tv_newest_music
        else -> R.string.tv_newest_movies
    },
)
