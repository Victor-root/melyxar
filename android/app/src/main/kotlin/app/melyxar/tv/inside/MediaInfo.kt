package app.melyxar.tv.inside

import android.text.format.DateFormat
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.AudioTrack
import app.melyxar.server.SubtitleTrack
import app.melyxar.server.Version
import app.melyxar.server.VideoTrack
import app.melyxar.tv.containerName
import app.melyxar.tv.fileBadges
import app.melyxar.tv.instantOf
import app.melyxar.tv.languageName
import app.melyxar.tv.look.CardGap
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.listLine
import app.melyxar.tv.look.SectionGap
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rememberRowScrolling
import app.melyxar.tv.rangeName
import app.melyxar.tv.readableBitrate
import app.melyxar.tv.readableSize
import java.text.NumberFormat
import java.util.Date
import java.util.Locale

/** How wide every panel is, the file's with its path as a track's. */
private val PANEL_WIDE = 340.dp

/** The room round the panels for their edge to light in. */
private val PANELS_ROOM = 8.dp

/** One panel: what it is about, a line saying which, its facts, each a
 *  label and its value, those the analysis found nothing for left out, and
 *  what is wrong with it. */
private class Panel(
    val key: String,
    val icon: ImageVector,
    val title: String,
    val lead: String?,
    val facts: List<Pair<String, String>>,
    val warnings: List<String> = emptyList(),
)

/**
 * What the file of [version] holds, at the very end of a work's page, as the
 * web shows it to its administrators: the file first, where it is and what
 * it is, then a panel for each track, side by side, everything the analysis
 * recorded. Each panel takes the remote in turn, so the page reaches them.
 *
 * Every panel as wide and as tall as the tallest: of their own sizes, the
 * row jumped up and down as the remote went along it. They are few, so laid
 * out all at once, which is what lets them be measured against each other.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun MediaInfo(version: Version, memory: FocusMemory) {
    val panels = panelsOf(version)
    val row = remember { RowMemory() }
    Column(Modifier.padding(bottom = SectionGap - PANELS_ROOM)) {
        RowHead(stringResource(R.string.tv_media_info), Icons.Info)
        CompositionLocalProvider(LocalBringIntoViewSpec provides rememberRowScrolling()) {
            Row(
                modifier = Modifier
                    .landsAsLeft(row, panels.first().key)
                    .horizontalScroll(rememberScrollState())
                    .height(IntrinsicSize.Max)
                    .padding(horizontal = Gutter, vertical = PANELS_ROOM),
                horizontalArrangement = Arrangement.spacedBy(CardGap),
            ) {
                panels.forEach { panel ->
                    PanelOf(
                        panel,
                        Modifier
                            .fillMaxHeight()
                            .inRow(row, panel.key)
                            .remembered(memory, "media/${version.id}/${panel.key}"),
                    )
                }
            }
        }
    }
}

@Composable
private fun panelsOf(version: Version): List<Panel> =
    listOf(filePanel(version)) +
        version.video.mapIndexed { at, track -> videoPanel(at, track) } +
        version.audio.mapIndexed { at, track -> audioPanel(at, track) } +
        version.subtitles.mapIndexed { at, track -> subtitlePanel(at, track) }

@Composable
private fun filePanel(version: Version): Panel {
    val context = LocalContext.current
    val added = version.addedAt?.let(::instantOf)?.let {
        "${DateFormat.getMediumDateFormat(context).format(Date(it))} ${DateFormat.getTimeFormat(context).format(Date(it))}"
    }
    val warnings = listOfNotNull(
        stringResource(R.string.tv_media_missing).takeIf { version.missing },
        stringResource(R.string.tv_media_not_analysed).takeIf { !version.analysed },
    )
    return Panel(
        key = "file",
        icon = Icons.Info,
        title = stringResource(R.string.tv_media_file),
        lead = version.path,
        facts = facts(
            stringResource(R.string.tv_media_container) to version.container?.let(::containerName),
            stringResource(R.string.tv_media_size) to version.sizeBytes.takeIf { it > 0 }?.let(::readableSize),
            stringResource(R.string.tv_media_added) to added,
            stringResource(R.string.tv_media_disk) to version.rootLabel,
            stringResource(R.string.tv_media_bitrate) to readableBitrate(version.overallBitrate),
            stringResource(R.string.tv_chapters) to version.chapters.takeIf { it > 0 }?.toString(),
        ),
        warnings = warnings,
    )
}

@Composable
private fun videoPanel(at: Int, track: VideoTrack) = Panel(
    key = "video/$at",
    icon = Icons.Film,
    title = stringResource(R.string.tv_video),
    lead = listOfNotNull(fileBadges(track.width, track.height, track.hdr, null).joinToString(" ").ifEmpty { null }, track.title)
        .joinToString(" · ").ifEmpty { null },
    facts = facts(
        stringResource(R.string.tv_track_codec) to track.codec.uppercase(),
        stringResource(R.string.tv_track_profile) to track.profile,
        stringResource(R.string.tv_track_level) to track.level?.toString(),
        stringResource(R.string.tv_track_resolution) to "${track.width}×${track.height}".takeIf { track.width > 0 && track.height > 0 },
        stringResource(R.string.tv_track_aspect) to track.aspectRatio,
        stringResource(R.string.tv_track_interlaced) to yesOrNo(track.isInterlaced),
        stringResource(R.string.tv_track_frame_rate) to track.frameRate?.let { String.format(Locale.ROOT, "%.3f", it) },
        stringResource(R.string.tv_track_bitrate) to readableBitrate(track.bitrate),
        stringResource(R.string.tv_track_hdr) to track.hdr?.let(::rangeName),
        stringResource(R.string.tv_track_primaries) to track.colorPrimaries,
        stringResource(R.string.tv_track_space) to track.colorSpace,
        stringResource(R.string.tv_track_transfer) to track.colorTransfer,
        stringResource(R.string.tv_track_depth) to track.bitDepth?.let { stringResource(R.string.tv_track_bits, it) },
        stringResource(R.string.tv_track_pixels) to track.pixelFormat,
        stringResource(R.string.tv_track_reference_frames) to track.referenceFrames?.toString(),
        stringResource(R.string.tv_track_default) to yesOrNo(track.isDefault),
    ),
)

@Composable
private fun audioPanel(at: Int, track: AudioTrack) = Panel(
    key = "audio/$at",
    icon = Icons.Audio,
    title = stringResource(R.string.tv_audio),
    lead = leadOf(track.language, track.title),
    facts = facts(
        stringResource(R.string.tv_track_codec) to track.codec.uppercase(),
        stringResource(R.string.tv_track_profile) to track.profile,
        stringResource(R.string.tv_track_layout) to track.channelLayout,
        stringResource(R.string.tv_track_channels) to stringResource(R.string.tv_track_ch, track.channels),
        stringResource(R.string.tv_track_bitrate) to readableBitrate(track.bitrate),
        stringResource(R.string.tv_track_sample_rate) to track.sampleRate?.let { "${NumberFormat.getInstance().format(it)} Hz" },
        stringResource(R.string.tv_track_depth) to track.bitDepth?.let { stringResource(R.string.tv_track_bits, it) },
        stringResource(R.string.tv_track_default) to yesOrNo(track.isDefault),
        stringResource(R.string.tv_track_forced) to yesOrNo(track.isForced),
    ),
)

@Composable
private fun subtitlePanel(at: Int, track: SubtitleTrack) = Panel(
    key = "subtitles/$at",
    icon = Icons.Subtitles,
    title = stringResource(R.string.tv_subtitles),
    lead = leadOf(track.language, track.title),
    facts = facts(
        stringResource(R.string.tv_track_codec) to track.codec.uppercase(),
        stringResource(R.string.tv_track_default) to yesOrNo(track.isDefault),
        stringResource(R.string.tv_track_forced) to yesOrNo(track.isForced),
        stringResource(R.string.tv_track_hearing_impaired) to yesOrNo(track.isHearingImpaired),
        stringResource(R.string.tv_track_external) to yesOrNo(track.isExternal),
        // Not a property of the file but of what playing it costs, and the
        // one thing here worth knowing before pressing play.
        stringResource(R.string.tv_track_burns_in) to yesOrNo(true).takeIf { track.burnsIn },
    ),
)

/** A track's language in the reader's words, and the title the file gives it. */
private fun leadOf(language: String?, title: String?): String? =
    listOfNotNull(languageName(language, Locale.getDefault()), title).joinToString(" · ").ifEmpty { null }

@Composable
private fun yesOrNo(yes: Boolean): String = stringResource(if (yes) R.string.tv_yes else R.string.tv_no)

/** The facts the analysis found something for. */
private fun facts(vararg all: Pair<String, String?>): List<Pair<String, String>> =
    all.mapNotNull { (label, value) -> value?.takeIf { it.isNotBlank() }?.let { label to it } }

/** A panel on the raised surface, lit as the remote comes to it. */
@Composable
private fun PanelOf(panel: Panel, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Column(
        modifier = modifier
            .width(PANEL_WIDE)
            .listLine { lit }
            .focusable(interactionSource = interaction)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph(panel.icon, palette.accentBright, 20.dp)
            Text(panel.title, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
        }
        panel.lead?.let { Text(it, fontSize = 13.sp, color = palette.textSoft, modifier = Modifier.padding(bottom = 6.dp)) }
        panel.facts.forEach { (label, value) ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(label, fontSize = 13.sp, color = palette.textSoft, modifier = Modifier.weight(1f))
                Text(value, fontSize = 13.sp, color = palette.text)
            }
        }
        panel.warnings.forEach { Text(it, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = palette.attention) }
    }
}
