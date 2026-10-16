package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Opening
import app.melyxar.server.Sessions
import app.melyxar.theme.parseHex
import app.melyxar.tv.languageName
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.listLine
import app.melyxar.tv.music.LocalListening
import app.melyxar.tv.play.passSoundOn
import app.melyxar.tv.play.soundPassedOn
import java.util.Locale
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** The accent colours offered, as the web offers them, the usual first. */
private val ACCENTS = listOf("#c81e1e", "#d9480f", "#c2a100", "#2f9e44", "#0c8599", "#1c7ed6", "#6741d9", "#c2255c")

private val ACCENT_NAMES = listOf(
    R.string.tv_accent_red, R.string.tv_accent_orange, R.string.tv_accent_yellow, R.string.tv_accent_green,
    R.string.tv_accent_teal, R.string.tv_accent_blue, R.string.tv_accent_violet, R.string.tv_accent_pink,
)

private val LIGHT_NAMES = listOf(
    R.string.tv_light_1, R.string.tv_light_2, R.string.tv_light_3, R.string.tv_light_4, R.string.tv_light_5,
    R.string.tv_light_6, R.string.tv_light_7, R.string.tv_light_8, R.string.tv_light_9, R.string.tv_light_10,
)

/** The lengths offered for a step of the player, or the way back on
 *  resuming, as long as the server lets them be. */
private val STEPS = listOf(5, 10, 15, 20, 30, 45, 60, 90)
private val REWINDS = listOf(0, 3, 5, 10, 15, 30)

/** The lengths offered for the music's fades and steps. */
private val CROSSFADES = listOf(0, 2, 4, 6, 8, 10, 12)
private val MUSIC_STEPS = listOf(5, 10, 15, 30)

/** A setting being chosen, in its list over the page. */
private class Asked(
    val title: String,
    val names: List<String>,
    val chosen: Int?,
    val swatches: List<Color>,
    val onChoose: (Int) -> Unit,
)

/**
 * What the account chooses for itself, as the web's settings less what a
 * television has no use for, and what this television opens on: each
 * setting a line saying what it is on, choosing it opening the list of what
 * it can be. A choice shows everywhere at once.
 */
@Composable
fun SettingsScreen(sessions: Sessions) {
    val palette = LocalPalette.current
    val resources = LocalResources.current
    val api = LocalServer.current
    val preferences = LocalAccountPreferences.current
    val listening = LocalListening.current
    val chosen = preferences.now
    val music = listening.preferences
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val scope = rememberCoroutineScope()
    var asked by remember { mutableStateOf<Asked?>(null) }
    var opening by remember { mutableStateOf(sessions.opening(api.origin)) }
    val context = LocalContext.current
    var passedOn by remember { mutableStateOf(soundPassedOn(context)) }
    val profiles = rememberLoaded("door:names") { names().names.map { it.name } }.value.ready.orEmpty()
        .plus(sessions.of(api.origin).map { it.name })
        .distinct()
    val list = rememberLazyListState()

    fun close() {
        asked = null
        scope.launch {
            withFrameNanos {}
            memory.restore()
        }
    }

    /** Opens the list of [values], said as [names], under [title], [now]
     *  ticked when it is one of them. */
    fun <T> ask(title: String, values: List<T>, names: List<String>, now: T, swatches: List<Color> = emptyList(), choose: (T) -> Unit) {
        asked = Asked(title, names, values.indexOf(now).takeIf { it >= 0 }, swatches) { choose(values[it]) }
    }

    fun seconds(count: Int) = resources.getQuantityString(R.plurals.tv_seconds, count, count)

    /** [offered] as long as [longest] lets them be, with [now] among them. */
    fun lengths(offered: List<Int>, now: Int, longest: Int) = (offered.filter { it <= longest } + now).distinct().sorted()

    fun language(code: String?) = code?.let { languageName(it, Locale.getDefault()) ?: it }
        ?: resources.getString(R.string.tv_language_none)

    Box(Modifier.fillMaxSize()) {
        ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = 40.dp), modifier = Modifier.fillMaxSize()) {
                item("top") {
                    Box(Modifier.head(head, "top")) {
                        TopBar()
                    }
                }
                item("title") {
                    ReadingColumn(Modifier.head(head, "title")) {
                        Row(
                            modifier = Modifier.padding(top = 26.dp, bottom = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Glyph(Icons.Gear, palette.accentBright, 26.dp)
                            Text(
                                stringResource(R.string.tv_settings),
                                fontSize = 28.sp,
                                fontWeight = FontWeight.Bold,
                                letterSpacing = (-0.02).em,
                                color = palette.text,
                            )
                        }
                    }
                }
                if (chosen == null) {
                    return@LazyColumn
                }
                group("look", R.string.tv_settings_look, Icons.Eye, Modifier.head(head, "look")) {
                    val themes = listOf("system", "dark", "light")
                    val themeNames = listOf(R.string.tv_theme_system, R.string.tv_theme_dark, R.string.tv_theme_light).map { stringResource(it) }
                    val title = stringResource(R.string.tv_setting_theme)
                    Line("theme", title, themeNames.getOrNull(themes.indexOf(chosen.themeMode)), memory, first) {
                        ask(title, themes, themeNames, chosen.themeMode) { mode ->
                            preferences.change({ it.copy(themeMode = mode) }, buildJsonObject { put("theme_mode", mode) })
                        }
                    }
                }
                group("accent", null, null, Modifier.head(head, "accent")) {
                    val names = ACCENT_NAMES.map { stringResource(it) }
                    val title = stringResource(R.string.tv_setting_accent)
                    val now = chosen.accentColor.lowercase()
                    Line("accent", title, names.getOrNull(ACCENTS.indexOf(now)) ?: stringResource(R.string.tv_accent_own), memory, swatch = parseHex(now)?.let(::Color)) {
                        ask(title, ACCENTS, names, now, ACCENTS.mapNotNull { hex -> parseHex(hex)?.let(::Color) }) { colour ->
                            preferences.change({ it.copy(accentColor = colour) }, buildJsonObject { put("accent_color", colour) })
                        }
                    }
                }
                group("backdrop", null, null, Modifier.head(head, "backdrop")) {
                    val backdrops = listOf("light", "library", "none")
                    val names = listOf(R.string.tv_backdrop_light, R.string.tv_backdrop_library, R.string.tv_backdrop_none).map { stringResource(it) }
                    val title = stringResource(R.string.tv_setting_backdrop)
                    Line("backdrop", title, names.getOrNull(backdrops.indexOf(chosen.backdrop)), memory) {
                        ask(title, backdrops, names, chosen.backdrop) { backdrop ->
                            preferences.change({ it.copy(backdrop = backdrop) }, buildJsonObject { put("backdrop", backdrop) })
                        }
                    }
                }
                if (chosen.backdrop == "light") {
                    group("light", null, null, Modifier.head(head, "light")) {
                        val lights = (1..LIGHT_NAMES.size).toList()
                        val names = LIGHT_NAMES.map { stringResource(it) }
                        val title = stringResource(R.string.tv_setting_backdrop_light)
                        Line("light", title, names.getOrNull(chosen.backdropLight - 1), memory) {
                            ask(title, lights, names, chosen.backdropLight) { light ->
                                preferences.change({ it.copy(backdropLight = light) }, buildJsonObject { put("backdrop_light", light) })
                            }
                        }
                    }
                }
                group("playing", R.string.tv_settings_playing, Icons.Play) {
                    val title = stringResource(R.string.tv_setting_step_back)
                    val steps = lengths(STEPS, chosen.stepBackSeconds, chosen.longestStep)
                    Line("step-back", title, seconds(chosen.stepBackSeconds), memory) {
                        ask(title, steps, steps.map(::seconds), chosen.stepBackSeconds) { step ->
                            preferences.change({ it.copy(stepBackSeconds = step) }, buildJsonObject { put("step_back_seconds", step) })
                        }
                    }
                }
                group("step-on", null, null) {
                    val title = stringResource(R.string.tv_setting_step_on)
                    val steps = lengths(STEPS, chosen.stepOnSeconds, chosen.longestStep)
                    Line("step-on", title, seconds(chosen.stepOnSeconds), memory) {
                        ask(title, steps, steps.map(::seconds), chosen.stepOnSeconds) { step ->
                            preferences.change({ it.copy(stepOnSeconds = step) }, buildJsonObject { put("step_on_seconds", step) })
                        }
                    }
                }
                group("rewind", null, null) {
                    val title = stringResource(R.string.tv_setting_resume_rewind)
                    val off = stringResource(R.string.tv_resume_rewind_off)
                    val rewinds = lengths(REWINDS, chosen.resumeRewindSeconds, chosen.longestStep)
                    val names = rewinds.map { if (it == 0) off else seconds(it) }
                    Line("rewind", title, names[rewinds.indexOf(chosen.resumeRewindSeconds)], memory) {
                        ask(title, rewinds, names, chosen.resumeRewindSeconds) { rewind ->
                            preferences.change({ it.copy(resumeRewindSeconds = rewind) }, buildJsonObject { put("resume_rewind_seconds", rewind) })
                        }
                    }
                }
                group("hdr", null, null) {
                    val title = stringResource(R.string.tv_setting_hdr)
                    val names = chosen.wideGamutChoices.map { stringResource(hdrName(it)) }
                    Line("hdr", title, stringResource(hdrName(chosen.wideGamut)), memory) {
                        ask(title, chosen.wideGamutChoices, names, chosen.wideGamut) { gamut ->
                            preferences.change({ it.copy(wideGamut = gamut) }, buildJsonObject { put("wide_gamut", gamut) })
                        }
                    }
                }
                group("audio-language", R.string.tv_settings_tracks, Icons.Subtitles) {
                    val title = stringResource(R.string.tv_setting_audio_language)
                    val codes = listOf(null) + chosen.audioLanguages
                    Line("audio-language", title, language(chosen.preferredAudioLanguage), memory) {
                        ask(title, codes, codes.map(::language), chosen.preferredAudioLanguage) { code ->
                            preferences.change(
                                { it.copy(preferredAudioLanguage = code) },
                                buildJsonObject { put("preferred_audio_language", JsonPrimitive(code.orEmpty())) },
                            )
                        }
                    }
                }
                group("subtitle-language", null, null) {
                    val title = stringResource(R.string.tv_setting_subtitle_language)
                    val codes = listOf(null) + chosen.subtitleLanguages
                    Line("subtitle-language", title, language(chosen.preferredSubtitleLanguage), memory) {
                        ask(title, codes, codes.map(::language), chosen.preferredSubtitleLanguage) { code ->
                            preferences.change(
                                { it.copy(preferredSubtitleLanguage = code) },
                                buildJsonObject { put("preferred_subtitle_language", JsonPrimitive(code.orEmpty())) },
                            )
                        }
                    }
                }
                group("subtitle-mode", null, null) {
                    val title = stringResource(R.string.tv_setting_subtitle_mode)
                    val names = chosen.subtitleModes.map { stringResource(subtitleModeName(it)) }
                    Line("subtitle-mode", title, stringResource(subtitleModeName(chosen.subtitleMode)), memory) {
                        ask(title, chosen.subtitleModes, names, chosen.subtitleMode) { mode ->
                            preferences.change({ it.copy(subtitleMode = mode) }, buildJsonObject { put("subtitle_mode", mode) })
                        }
                    }
                }
                group("crossfade", R.string.tv_settings_music, Icons.Music) {
                    val title = stringResource(R.string.tv_setting_crossfade)
                    val none = stringResource(R.string.tv_crossfade_none)
                    val fades = (CROSSFADES + music.crossfadeSeconds).distinct().sorted()
                    val names = fades.map { if (it == 0) none else seconds(it) }
                    Line("crossfade", title, names[fades.indexOf(music.crossfadeSeconds)], memory) {
                        ask(title, fades, names, music.crossfadeSeconds) { fade ->
                            listening.changePreferences(music.copy(crossfadeSeconds = fade), buildJsonObject { put("crossfade_seconds", fade) })
                        }
                    }
                }
                group("volume", null, null) {
                    val title = stringResource(R.string.tv_setting_volume)
                    val modes = listOf("track", "album", "off")
                    val names = listOf(R.string.tv_volume_track, R.string.tv_volume_album, R.string.tv_volume_off).map { stringResource(it) }
                    Line("volume", title, names.getOrNull(modes.indexOf(music.volumeMode)), memory) {
                        ask(title, modes, names, music.volumeMode) { mode ->
                            listening.changePreferences(music.copy(volumeMode = mode), buildJsonObject { put("volume_mode", mode) })
                        }
                    }
                }
                group("music-step-back", null, null) {
                    val title = stringResource(R.string.tv_setting_step_back)
                    val steps = (MUSIC_STEPS + music.skipBackSeconds).distinct().sorted()
                    Line("music-step-back", title, seconds(music.skipBackSeconds), memory) {
                        ask(title, steps, steps.map(::seconds), music.skipBackSeconds) { step ->
                            listening.changePreferences(music.copy(skipBackSeconds = step), buildJsonObject { put("skip_back_seconds", step) })
                        }
                    }
                }
                group("music-step-on", null, null) {
                    val title = stringResource(R.string.tv_setting_step_on)
                    val steps = (MUSIC_STEPS + music.skipOnSeconds).distinct().sorted()
                    Line("music-step-on", title, seconds(music.skipOnSeconds), memory) {
                        ask(title, steps, steps.map(::seconds), music.skipOnSeconds) { step ->
                            listening.changePreferences(music.copy(skipOnSeconds = step), buildJsonObject { put("skip_on_seconds", step) })
                        }
                    }
                }
                group("opening", R.string.tv_settings_television, Icons.Television) {
                    val title = stringResource(R.string.tv_setting_opening)
                    val openings = listOf(Opening.Ask, Opening.Last) + profiles.map { Opening.Always(it) }
                    val names = listOf(stringResource(R.string.tv_opening_ask), stringResource(R.string.tv_opening_last)) + profiles
                    val now = names.getOrNull(openings.indexOf(opening)) ?: (opening as? Opening.Always)?.name
                    Line("opening", title, now, memory) {
                        ask(title, openings, names, opening) { chosenOpening ->
                            sessions.openOn(api.origin, chosenOpening)
                            opening = chosenOpening
                        }
                    }
                }
                group("sound-out", null, null) {
                    val title = stringResource(R.string.tv_setting_sound_out)
                    val ways = listOf(false, true)
                    val names = listOf(stringResource(R.string.tv_sound_decoded), stringResource(R.string.tv_sound_passed_on))
                    Line("sound-out", title, names[ways.indexOf(passedOn)], memory) {
                        ask(title, ways, names, passedOn) { passed ->
                            passSoundOn(context, passed)
                            passedOn = passed
                        }
                    }
                }
            }
        }
        asked?.let {
            ChoiceList(
                title = it.title,
                choices = it.names,
                chosen = it.chosen,
                onChoose = it.onChoose,
                onClose = ::close,
                modifier = Modifier.align(Alignment.Center),
                swatches = it.swatches,
            )
        }
    }
    RestoreFocus(memory, first, ready = chosen != null)
}

private fun hdrName(gamut: String): Int = when (gamut) {
    "always_convert" -> R.string.tv_hdr_always_convert
    "never_convert" -> R.string.tv_hdr_never_convert
    else -> R.string.tv_hdr_automatic
}

private fun subtitleModeName(mode: String): Int = when (mode) {
    "always" -> R.string.tv_subtitles_always
    "only_forced" -> R.string.tv_subtitles_only_forced
    "from_the_file" -> R.string.tv_subtitles_from_the_file
    "never" -> R.string.tv_subtitles_never
    else -> R.string.tv_subtitles_smart
}

/** A line of the settings, under the name of its group when it opens one;
 *  [modifier] makes it part of the page's head. */
private fun LazyListScope.group(key: String, name: Int?, icon: ImageVector?, modifier: Modifier = Modifier, line: @Composable () -> Unit) {
    item(key) {
        ReadingColumn(modifier) {
            name?.let { GroupHead(stringResource(it), icon) }
            line()
        }
    }
}

@Composable
private fun GroupHead(name: String, icon: ImageVector?) {
    val palette = LocalPalette.current
    Row(
        modifier = Modifier.padding(top = 30.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        icon?.let { Glyph(it, palette.accentBright, 20.dp) }
        Text(name, fontSize = 19.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
    }
}

/** A setting: its name, what it is on, and the way into its list; the
 *  first of the page takes [first]. */
@Composable
private fun Line(
    key: String,
    name: String,
    value: String?,
    memory: FocusMemory,
    first: FocusRequester? = null,
    swatch: Color? = null,
    onClick: () -> Unit,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = Modifier
            .padding(top = 8.dp)
            .fillMaxWidth()
            .remembered(memory, key)
            .then(first?.let { Modifier.focusRequester(it) } ?: Modifier)
            .listLine { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            name,
            fontSize = 16.sp,
            fontWeight = FontWeight.Medium,
            color = palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        swatch?.let { Box(Modifier.padding(end = 10.dp).size(16.dp).clip(CircleShape).background(it)) }
        value?.let { Text(it, fontSize = 15.sp, color = palette.textSoft, maxLines = 1) }
        Glyph(Icons.ChevronRight, palette.textFaint, 18.dp, Modifier.padding(start = 8.dp))
    }
}
