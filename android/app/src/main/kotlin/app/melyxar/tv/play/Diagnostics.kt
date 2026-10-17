package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.media3.common.util.UnstableApi
import app.melyxar.R
import app.melyxar.server.Producing
import app.melyxar.server.Refusal
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import java.io.IOException
import java.util.Locale
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** How often what the player and the server say is read again. */
private const val LOOK_EVERY_MS = 1_000L

/** How far one press up or down scrolls the window. */
private const val STEP_PX = 160f

/**
 * What is happening to this film while it plays, as the web player's
 * diagnostics say it: how it is sent and why, what the file holds and what
 * is done to its picture and its sound, what this device received and
 * decodes it with, and how hard the server works at it. The arrows up and
 * down scroll it, the back key closes it.
 */
@OptIn(UnstableApi::class)
@Composable
fun Diagnostics(playback: Playback, onClose: () -> Unit, modifier: Modifier) {
    val plan = playback.plan ?: return
    val api = LocalServer.current
    val palette = LocalPalette.current
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    val held = remember { FocusRequester() }
    var looked by remember { mutableIntStateOf(0) }
    var working by remember { mutableStateOf<Producing?>(null) }
    BackHandler(onBack = onClose)
    // Looked at on a rhythm: pictures shown and dropped only ever climb, and
    // nothing says when they do. The server is asked only while this is open.
    LaunchedEffect(playback) {
        while (true) {
            looked += 1
            working = playback.streamId?.let { id ->
                try {
                    api.preparation(id).producing
                } catch (_: Refusal) {
                    null
                } catch (_: IOException) {
                    null
                }
            }
            delay(LOOK_EVERY_MS)
        }
    }
    val film = plan.film
    val player = playback.player
    // Read again on each look.
    val seen = remember(looked) {
        Seen(
            arrived = player.videoFormat?.let { "${it.width}x${it.height}" + (it.sampleMimeType?.substringAfter('/')?.let { kind -> " · ${kind.uppercase()}" } ?: "") },
            shown = player.videoDecoderCounters?.renderedOutputBufferCount,
            dropped = player.videoDecoderCounters?.droppedBufferCount,
            ahead = ((player.bufferedPosition - player.currentPosition) / 1000).toInt().coerceAtLeast(0),
        )
    }
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier = modifier
            .width(600.dp)
            .heightIn(max = 470.dp)
            .clip(shape)
            .background(palette.surfaceRaised.copy(alpha = 0.94f))
            .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
            .focusRequester(held)
            .onPreviewKeyEvent { event ->
                val by = when (event.key) {
                    Key.DirectionDown -> STEP_PX
                    Key.DirectionUp -> -STEP_PX
                    else -> return@onPreviewKeyEvent false
                }
                if (event.type == KeyEventType.KeyDown) {
                    scope.launch { scroll.animateScrollBy(by) }
                }
                true
            }
            .focusable()
            .verticalScroll(scroll)
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Text(stringResource(R.string.tv_facts), fontSize = 20.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
        Block(stringResource(R.string.tv_facts_stream)) {
            Line(stringResource(R.string.tv_facts_method), methodName(plan.method))
            Line(stringResource(R.string.tv_facts_why), reasonCodes(plan.reasons).map { reasonName(it) }.joinToString(" · ").ifEmpty { null })
            working?.let {
                val speed = speedSaid(it.speed)
                Line(
                    stringResource(R.string.tv_facts_working),
                    if (it.picturesASecond > 0) {
                        stringResource(R.string.tv_facts_working_at, String.format(Locale.ROOT, "%.1f", it.picturesASecond), speed)
                    } else {
                        stringResource(R.string.tv_facts_working_speed, speed)
                    },
                    behind = it.speed < KEEPING_UP,
                )
            }
        }
        film?.picture?.let { picture ->
            Block(stringResource(R.string.tv_facts_picture)) {
                Line(stringResource(R.string.tv_facts_held), pictureHeld(picture))
                if (picture.frameWidth != null && picture.frameHeight != null) {
                    Line(stringResource(R.string.tv_facts_frame), "${picture.frameWidth}x${picture.frameHeight}")
                }
                val rebuild = plan.rebuild
                Line(
                    stringResource(R.string.tv_facts_done),
                    pictureDone(
                        rebuild,
                        stringResource(if (rebuild?.by == "card") R.string.tv_rebuilt_by_card else R.string.tv_rebuilt_by_processor),
                        stringResource(R.string.tv_facts_carried_over),
                    ),
                )
                Line(stringResource(R.string.tv_facts_arrived), seen.arrived)
                Line(stringResource(R.string.tv_facts_decoder), playback.videoDecoder)
                if (seen.shown != null && seen.dropped != null) {
                    Line(
                        stringResource(R.string.tv_facts_pictures),
                        pluralStringResource(R.plurals.tv_facts_displayed, seen.shown, seen.shown) + ", " +
                            pluralStringResource(R.plurals.tv_facts_dropped, seen.dropped, seen.dropped),
                    )
                }
                Line(stringResource(R.string.tv_facts_held_ahead), stringResource(R.string.tv_facts_seconds, seen.ahead))
            }
        }
        film?.sound?.let { sound ->
            val channels = pluralStringResource(R.plurals.tv_facts_channels, sound.channels, sound.channels)
            Block(stringResource(R.string.tv_facts_sound)) {
                Line(stringResource(R.string.tv_facts_held), soundHeld(sound) { channels })
                Line(
                    stringResource(R.string.tv_facts_done),
                    stringResource(if (soundRebuilt(plan.method)) R.string.tv_facts_rebuilt_sound else R.string.tv_facts_carried_over),
                )
                Line(stringResource(R.string.tv_facts_decoder), playback.audioDecoder ?: stringResource(R.string.tv_facts_passed_through))
            }
        }
        film?.let {
            Block(stringResource(R.string.tv_facts_file)) {
                Line(stringResource(R.string.tv_facts_container), it.container?.let(::containerName))
                Line(stringResource(R.string.tv_facts_size), asSize(it.sizeBytes))
                Line(stringResource(R.string.tv_facts_rate), asRate(it.overallBitrate))
            }
        }
    }
    LaunchedEffect(Unit) { held.requestFocus() }
}

/** What the player says of the picture it shows, read on each look. */
private class Seen(val arrived: String?, val shown: Int?, val dropped: Int?, val ahead: Int)

@Composable
private fun Block(title: String, lines: @Composable () -> Unit) {
    val palette = LocalPalette.current
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.accentBright)
        lines()
    }
}

/** One line: a name and what it is, or nothing at all; the one number that
 *  can be bad news, [behind], in the colour of trouble. */
@Composable
private fun Line(name: String, value: String?, behind: Boolean = false) {
    if (value.isNullOrEmpty()) {
        return
    }
    val palette = LocalPalette.current
    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Text(name, fontSize = 14.sp, color = palette.textFaint, modifier = Modifier.width(170.dp))
        Text(value, fontSize = 14.sp, color = if (behind) palette.trouble else palette.text, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun methodName(method: String): String = stringResource(
    when (method) {
        "direct_play" -> R.string.tv_playback_direct_play
        "remux" -> R.string.tv_playback_remux
        "transcode_audio" -> R.string.tv_playback_transcode_audio
        else -> R.string.tv_playback_full_transcode
    },
)

/** A reason in words; one this device has no words for, by its code. */
@Composable
private fun reasonName(code: String): String = when (code) {
    "everything_supported" -> stringResource(R.string.tv_reason_everything_supported)
    "container_not_supported" -> stringResource(R.string.tv_reason_container_not_supported)
    "video_codec_not_supported" -> stringResource(R.string.tv_reason_video_codec_not_supported)
    "video_profile_not_supported" -> stringResource(R.string.tv_reason_video_profile_not_supported)
    "resolution_too_high" -> stringResource(R.string.tv_reason_resolution_too_high)
    "wide_gamut_not_supported" -> stringResource(R.string.tv_reason_wide_gamut_not_supported)
    "wide_gamut_conversion_chosen" -> stringResource(R.string.tv_reason_wide_gamut_conversion_chosen)
    "wide_gamut_rebuilt_as_standard" -> stringResource(R.string.tv_reason_wide_gamut_rebuilt_as_standard)
    "dolby_vision_without_base_layer" -> stringResource(R.string.tv_reason_dolby_vision_without_base_layer)
    "interlaced_picture" -> stringResource(R.string.tv_reason_interlaced_picture)
    "bitrate_too_high" -> stringResource(R.string.tv_reason_bitrate_too_high)
    "audio_codec_not_supported" -> stringResource(R.string.tv_reason_audio_codec_not_supported)
    "too_many_audio_channels" -> stringResource(R.string.tv_reason_too_many_audio_channels)
    "downmix_requested" -> stringResource(R.string.tv_reason_downmix_requested)
    "loudness_levelling_requested" -> stringResource(R.string.tv_reason_loudness_levelling_requested)
    "subtitle_must_be_burned_in" -> stringResource(R.string.tv_reason_subtitle_must_be_burned_in)
    "subtitle_format_not_drawn_by_client" -> stringResource(R.string.tv_reason_subtitle_format_not_drawn_by_client)
    "non_default_track_selected" -> stringResource(R.string.tv_reason_non_default_track_selected)
    else -> code
}
