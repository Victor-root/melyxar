package app.melyxar.tv.play

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.melyxar.R
import app.melyxar.player.LIGHTS
import app.melyxar.server.Track
import app.melyxar.tv.languageName
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.offsetName
import app.melyxar.tv.soundName
import app.melyxar.tv.speakersName
import app.melyxar.tv.speedName
import java.text.NumberFormat

/** Which list is open over the film, and the list it leads back to. */
enum class Menu(val from: Menu? = null) {
    /** The sheets of what the film is and who is in it, drawn by the
     *  player itself over the film's foot. */
    Info,
    Cast,
    Audio,
    Subtitles,
    Chapters,
    Settings,
    Speed(Settings),
    Quality(Settings),
    WideGamut(Settings),
    Brightness(Settings),
    Contrast(Settings),
    WordsOffset(Settings),
    /** What is happening to the film, drawn by the player itself. */
    Diagnostics(Settings),
}

/** How far the words can be moved, half a second at a time, as the web
 *  moves them. */
private val OFFSETS = (-30..30).map { it * 0.5f }

/** What can be done with a film's HDR for this film alone, the account's
 *  own choice first, as the web offers it. */
private val WIDE_GAMUT_HERE = listOf(
    null to R.string.tv_wide_gamut_account,
    "never_convert" to R.string.tv_wide_gamut_keep,
    "always_convert" to R.string.tv_wide_gamut_convert,
)

/**
 * The list open over the film, as the web player's panels: the tracks, the
 * settings, and each list a setting opens, which leads back to the settings
 * once chosen or on the back key. [left] is the list just closed, which the
 * settings start on again.
 */
@Composable
fun PlayerMenu(menu: Menu, left: Menu?, playback: Playback, onMenu: (Menu?) -> Unit, modifier: Modifier) {
    val plan = playback.plan ?: return
    val back = { onMenu(menu.from) }
    val placed = modifier.padding(end = Gutter, bottom = 124.dp)
    val asItIs = stringResource(R.string.tv_quality_as_it_is)
    when (menu) {
        Menu.Audio -> ChoiceList(
            title = stringResource(R.string.tv_audio),
            choices = plan.audio.mapIndexed { at, track -> trackName(track, at, withSound = true) },
            chosen = plan.audio.indexOfFirst { it.id == playback.audio }.takeIf { it >= 0 },
            onChoose = { playback.chooseAudio(plan.audio[it]) },
            onClose = back,
            modifier = placed,
        )
        Menu.Subtitles -> ChoiceList(
            title = stringResource(R.string.tv_subtitles),
            choices = listOf(stringResource(R.string.tv_subtitles_off)) +
                plan.subtitles.mapIndexed { at, track -> trackName(track, at, withSound = false) },
            chosen = if (playback.subtitle == null) 0 else plan.subtitles.indexOfFirst { it.id == playback.subtitle } + 1,
            onChoose = { playback.chooseSubtitle(if (it == 0) null else plan.subtitles[it - 1]) },
            onClose = back,
            modifier = placed,
        )
        Menu.Chapters -> ChapterStrip(playback, onClose = back, modifier = modifier.fillMaxWidth())
        Menu.Info, Menu.Cast, Menu.Diagnostics -> Unit
        Menu.Settings -> {
            val wideGamut = plan.wideGamut
            val lines = listOfNotNull(
                Menu.Speed to speedName(playback.speed),
                Menu.Quality to playback.quality.name(asItIs),
                wideGamut?.let {
                    Menu.WideGamut to stringResource(if (it.converted) R.string.tv_wide_gamut_converted else R.string.tv_wide_gamut_kept)
                },
                (Menu.Brightness to lightName(playback.brightness)).takeIf { playback.lightsThePicture },
                (Menu.Contrast to lightName(playback.contrast)).takeIf { playback.lightsThePicture },
                Menu.WordsOffset to offsetName(playback.wordsOffset),
                // Opens a window rather than a list, with nothing to be on.
                Menu.Diagnostics to "",
            )
            ChoiceList(
                title = stringResource(R.string.tv_settings),
                choices = lines.map { (line, _) -> settingName(line) },
                chosen = null,
                onChoose = { onMenu(lines[it].first) },
                onClose = back,
                modifier = placed,
                values = lines.map { it.second },
                startAt = lines.indexOfFirst { it.first == left }.coerceAtLeast(0),
                leadsOn = true,
            )
        }
        Menu.Speed -> ChoiceList(
            title = settingName(menu),
            choices = SPEEDS.map(::speedName),
            chosen = SPEEDS.indexOf(playback.speed),
            onChoose = { playback.chooseSpeed(SPEEDS[it]) },
            onClose = back,
            modifier = placed,
        )
        Menu.Quality -> ChoiceList(
            title = settingName(menu),
            choices = QUALITIES.map { it.name(asItIs) },
            chosen = QUALITIES.indexOf(playback.quality),
            onChoose = { playback.chooseQuality(QUALITIES[it]) },
            onClose = back,
            modifier = placed,
        )
        Menu.Brightness -> ChoiceList(
            title = settingName(menu),
            choices = LIGHTS.map(::lightName),
            chosen = LIGHTS.indexOf(playback.brightness),
            onChoose = { playback.chooseLight(LIGHTS[it], playback.contrast) },
            onClose = back,
            modifier = placed,
        )
        Menu.Contrast -> ChoiceList(
            title = settingName(menu),
            choices = LIGHTS.map(::lightName),
            chosen = LIGHTS.indexOf(playback.contrast),
            onChoose = { playback.chooseLight(playback.brightness, LIGHTS[it]) },
            onClose = back,
            modifier = placed,
        )
        Menu.WordsOffset -> ChoiceList(
            title = settingName(menu),
            choices = OFFSETS.map(::offsetName),
            chosen = OFFSETS.indexOf(playback.wordsOffset),
            onChoose = { playback.chooseWordsOffset(OFFSETS[it]) },
            onClose = back,
            modifier = placed,
            note = stringResource(R.string.tv_words_offset_why),
        )
        Menu.WideGamut -> {
            val handling = plan.wideGamut
            if (handling != null && !handling.followsChoice) {
                // Whatever is picked, the film comes out the same: a choice
                // that changes nothing is worse than none.
                ChoiceList(
                    title = settingName(menu),
                    choices = emptyList(),
                    chosen = null,
                    onChoose = {},
                    onClose = back,
                    modifier = placed,
                    note = stringResource(
                        if (handling.converted) R.string.tv_wide_gamut_imposed_converted else R.string.tv_wide_gamut_imposed_kept,
                    ),
                )
            } else {
                ChoiceList(
                    title = settingName(menu),
                    choices = WIDE_GAMUT_HERE.map { stringResource(it.second) },
                    chosen = WIDE_GAMUT_HERE.indexOfFirst { it.first == playback.wideGamut },
                    onChoose = { playback.chooseWideGamut(WIDE_GAMUT_HERE[it].first) },
                    onClose = back,
                    modifier = placed,
                    note = stringResource(R.string.tv_wide_gamut_why),
                )
            }
        }
    }
}

@Composable
private fun settingName(menu: Menu): String = stringResource(
    when (menu) {
        Menu.Quality -> R.string.tv_quality
        Menu.WideGamut -> R.string.tv_wide_gamut
        Menu.WordsOffset -> R.string.tv_words_offset
        Menu.Brightness -> R.string.tv_brightness
        Menu.Contrast -> R.string.tv_contrast
        Menu.Diagnostics -> R.string.tv_facts
        else -> R.string.tv_speed
    },
)

/** A step of light as the reader's language writes a share: "120%". */
private fun lightName(light: Float): String = NumberFormat.getPercentInstance().format(light)

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
