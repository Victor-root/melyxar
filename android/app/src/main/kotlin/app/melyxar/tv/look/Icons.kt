package app.melyxar.tv.look

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/*
 * The web interface's icons (`web/src/icons.tsx`, `web/src/player/icons.tsx`),
 * path for path, on the same 24 point square: drawn in one colour, the
 * outlined ones with round ends.
 */

/** A circle as a path, the way the web's `<circle>` draws it. */
private fun circle(x: Float, y: Float, r: Float) =
    "M${x - r} ${y}a$r $r 0 1 0 ${2 * r} 0a$r $r 0 1 0 ${-2 * r} 0"

/** A rectangle with round corners as a path, as `<rect rx>` draws it. */
private fun rect(x: Float, y: Float, w: Float, h: Float, r: Float) =
    "M${x + r} ${y}h${w - 2 * r}a$r $r 0 0 1 $r ${r}v${h - 2 * r}a$r $r 0 0 1 ${-r} ${r}" +
        "h${-(w - 2 * r)}a$r $r 0 0 1 ${-r} ${-r}v${-(h - 2 * r)}a$r $r 0 0 1 $r ${-r}z"

private fun outlined(name: String, stroke: Float = 1.8f, vararg paths: String) = ImageVector.Builder(
    name = name,
    defaultWidth = 24.dp,
    defaultHeight = 24.dp,
    viewportWidth = 24f,
    viewportHeight = 24f,
).apply {
    paths.forEach { path ->
        addPath(
            pathData = addPathNodes(path),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = stroke,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        )
    }
}.build()

private fun solid(name: String, stroke: Float = 0f, vararg paths: String) = ImageVector.Builder(
    name = name,
    defaultWidth = 24.dp,
    defaultHeight = 24.dp,
    viewportWidth = 24f,
    viewportHeight = 24f,
).apply {
    paths.forEach { path ->
        addPath(
            pathData = addPathNodes(path),
            fill = SolidColor(Color.Black),
            stroke = if (stroke > 0f) SolidColor(Color.Black) else null,
            strokeLineWidth = stroke,
            strokeLineJoin = StrokeJoin.Round,
        )
    }
}.build()

object Icons {
    /** Filled and stroked alike, which rounds its corners. */
    val Play = solid("play", 2.6f, "M7.9 6.1 17.3 12 7.9 17.9Z")
    val Pause = solid("pause", 0f, rect(6.5f, 5f, 4f, 14f, 1.2f), rect(13.5f, 5f, 4f, 14f, 1.2f))
    val Heart = outlined(
        "heart",
        paths = arrayOf("M19.5 12.572l-7.5 7.428l-7.5 -7.428a5 5 0 1 1 7.5 -6.566a5 5 0 1 1 7.5 6.572"),
    )
    val HeartFull = solid(
        "heart-full",
        1.8f,
        "M19.5 12.572l-7.5 7.428l-7.5 -7.428a5 5 0 1 1 7.5 -6.566a5 5 0 1 1 7.5 6.572",
    )
    val Television = outlined("television", paths = arrayOf(rect(3f, 4.5f, 18f, 12f, 2f), "M8.5 20.5h7", "M12 16.5v4"))
    val Bell = outlined(
        "bell",
        paths = arrayOf(
            "M6.2 16.6V11a5.8 5.8 0 0 1 11.6 0v5.6l1.6 1.6H4.6Z",
            "M10 20.4a2.2 2.2 0 0 0 4 0",
        ),
    )
    val Stop = solid("stop", 0f, "M7 7h10v10H7Z")
    val Tick = outlined("tick", paths = arrayOf("M5 12.6l4.6 4.6L19 7.4"))
    val Clock = outlined("clock", paths = arrayOf(circle(12f, 12f, 8.6f), "M12 7.4V12l3 1.8"))
    val Info = outlined("info", paths = arrayOf(circle(12f, 12f, 8.8f), "M12 11v5.4", "M12 7.8h.01"))
    val Search = outlined("search", paths = arrayOf(circle(10.6f, 10.6f, 6.8f), "M15.6 15.6 20.6 20.6"))
    val ArrowUp = outlined("arrow-up", paths = arrayOf("M12 19V5", "m6 11 6-6 6 6"))
    val ArrowDown = outlined("arrow-down", paths = arrayOf("M12 5v14", "m6 13 6 6 6-6"))
    val Microphone = outlined(
        "microphone",
        paths = arrayOf(
            "M12 3.4a2.9 2.9 0 0 1 2.9 2.9v5.2a2.9 2.9 0 0 1-5.8 0V6.3A2.9 2.9 0 0 1 12 3.4Z",
            "M6.4 11.4a5.6 5.6 0 0 0 11.2 0",
            "M12 17v3.6",
        ),
    )
    val ChevronRight = outlined("chevron-right", paths = arrayOf("M8.8 5.6 15.2 12 8.8 18.4"))
    val ChevronLeft = outlined("chevron-left", paths = arrayOf("M15.2 5.6 8.8 12 15.2 18.4"))
    val ChevronDown = outlined("chevron-down", paths = arrayOf("M5.6 8.8 12 15.2 18.4 8.8"))
    val Back = outlined("back", paths = arrayOf("M19.4 12H5.2", "m11 5.6-5.8 6.4 5.8 6.4"))
    val Lock = outlined(
        "lock",
        paths = arrayOf(rect(4.8f, 10.4f, 14.4f, 9.4f, 2.2f), "M8.4 10.4V7.8a3.6 3.6 0 0 1 7.2 0v2.6", "M12 14.2v2"),
    )
    val Restart = outlined("restart", paths = arrayOf("M12 5a7 7 0 1 1-6.8 8.6", "M5 3.4V7.4h4"))
    val StepOn = outlined("step-on", paths = arrayOf("M12 5a7 7 0 1 0 6.8 8.6", "M19 3.4V7.4h-4"))
    val People = outlined(
        "people",
        paths = arrayOf(
            circle(9.2f, 8.4f, 3.2f),
            "M3.4 19.2a5.8 5.8 0 0 1 11.6 0",
            "M15.4 5.4a3.2 3.2 0 0 1 0 6M17.6 14a5.8 5.8 0 0 1 3 5.2",
        ),
    )
    val Subtitles = outlined(
        "subtitles",
        paths = arrayOf(rect(2.8f, 5f, 18.4f, 14f, 2.4f), "M6.6 14.6h4.2M13.4 14.6h4M6.6 11h2.6M11.8 11h5.6"),
    )
    val Gear = outlined(
        "gear",
        paths = arrayOf(
            circle(12f, 12f, 3.1f),
            "M18.66 14.63a1.5 1.5 0 0 0 .3 1.66l.06.05a1.8 1.8 0 1 1-2.55 2.55l-.05-.06a1.5 1.5 0 0 0-1.66-.3 " +
                "1.5 1.5 0 0 0-.9 1.37v.16a1.8 1.8 0 1 1-3.6 0v-.09a1.5 1.5 0 0 0-.99-1.37 1.5 1.5 0 0 0-1.65.3l-.06.06" +
                "a1.8 1.8 0 1 1-2.55-2.55l.06-.05a1.5 1.5 0 0 0 .3-1.66 1.5 1.5 0 0 0-1.38-.9h-.16a1.8 1.8 0 1 1 0-3.6h.09" +
                "a1.5 1.5 0 0 0 1.37-.99 1.5 1.5 0 0 0-.3-1.65l-.06-.06A1.8 1.8 0 1 1 7.56 5.11l.05.06a1.5 1.5 0 0 0 1.66.3h.07" +
                "a1.5 1.5 0 0 0 .9-1.38v-.16a1.8 1.8 0 1 1 3.6 0v.09a1.5 1.5 0 0 0 .9 1.37 1.5 1.5 0 0 0 1.66-.3l.05-.06" +
                "a1.8 1.8 0 1 1 2.55 2.55l-.06.05a1.5 1.5 0 0 0-.3 1.66v.07a1.5 1.5 0 0 0 1.38.9h.16a1.8 1.8 0 1 1 0 3.6h-.09" +
                "a1.5 1.5 0 0 0-1.37.9Z",
        ),
    )
    val Chapters = outlined("chapters", paths = arrayOf(rect(2.8f, 6.4f, 8f, 11.2f, 1.6f), rect(13.2f, 6.4f, 8f, 11.2f, 1.6f)))
    val Audio = outlined("audio", paths = arrayOf("M4 9.6v4.8M8 6.6v10.8M12 4v16M16 7.4v9.2M20 10.4v3.2"))
    val NextEpisode = solid(
        "next-episode",
        0f,
        "M 11 12 L 4.61 6.74 A 0.7 0.7 0 0 0 4 7.1 L 4 16.9 A 0.7 0.7 0 0 0 4.61 17.26 Z",
        "M 18 12 L 11.61 6.74 A 0.7 0.7 0 0 0 11 7.1 L 11 16.9 A 0.7 0.7 0 0 0 11.61 17.26 Z",
        rect(19f, 5.9f, 1.1f, 12.2f, 0.55f),
    )
    val Film = outlined(
        "film",
        paths = arrayOf(
            rect(2.8f, 4.4f, 18.4f, 15.2f, 2.2f),
            "M7.4 4.4v15.2M16.6 4.4v15.2M2.8 12h18.4M2.8 8.2h4.6M2.8 15.8h4.6M16.6 8.2h4.6M16.6 15.8h4.6",
        ),
    )
    val Series = outlined(
        "series",
        paths = arrayOf(rect(2.8f, 6.6f, 18.4f, 12f, 2.2f), "M8.4 3.2 12 6.6l3.6-3.4", "M8.6 21.4h6.8"),
    )
    val Anime = outlined(
        "anime",
        paths = arrayOf(
            "M13.192 9h6.616a2 2 0 0 1 1.992 2.183l-.567 6.182a4 4 0 0 1 -3.983 3.635h-1.5a4 4 0 0 1 -3.983 -3.635l-.567 -6.182a2 2 0 0 1 1.992 -2.183",
            "M15 13h.01",
            "M18 13h.01",
            "M15 16.5c1 .667 2 .667 3 0",
            "M8.632 15.982a4.037 4.037 0 0 1 -.382 .018h-1.5a4 4 0 0 1 -3.983 -3.635l-.567 -6.182a2 2 0 0 1 1.992 -2.183h6.616a2 2 0 0 1 2 2",
            "M6 8h.01",
            "M9 8h.01",
            "M6 12c.884 .251 1.648 .131 2.291 -.36",
        ),
    )
    val Shows = outlined(
        "shows",
        paths = arrayOf(
            circle(12f, 9.4f, 2.2f),
            "M7.8 5.2a6 6 0 0 0 0 8.4M16.2 5.2a6 6 0 0 1 0 8.4",
            "M4.8 2.6a10 10 0 0 0 0 13.6M19.2 2.6a10 10 0 0 1 0 13.6",
            "M12 11.6 9.6 21.4M12 11.6l2.4 9.8",
        ),
    )
    val HomeMedia = outlined(
        "home-media",
        paths = arrayOf(
            rect(3f, 3f, 18f, 18f, 3f),
            "M15 8h.01",
            "M3 16l5-5c.93-.9 2.07-.9 3 0l5 5",
            "M14 14l1-1c.93-.9 2.07-.9 3 0l3 3",
        ),
    )
    val NextSong = solid("next-song", 2.2f, "M6.2 6.6 14.4 12l-8.2 5.4Z", "M17.8 6.4v11.2")
    val PreviousSong = solid("previous-song", 2.2f, "M17.8 6.6 9.6 12l8.2 5.4Z", "M6.2 6.4v11.2")
    val Shuffle = outlined(
        "shuffle",
        paths = arrayOf(
            "M3.6 7.2h3.2c2.2 0 3.4 1.1 4.6 3.4l1.2 2.8c1.2 2.3 2.4 3.4 4.6 3.4h3.2",
            "M3.6 16.8h3.2c1.5 0 2.5-.5 3.4-1.5M13.8 8.7c.9-1 1.9-1.5 3.4-1.5h3.2",
            "m18 4.9 2.4 2.3L18 9.5M18 14.5l2.4 2.3-2.4 2.3",
        ),
    )
    val Repeat = outlined(
        "repeat",
        paths = arrayOf(
            "M4.4 11.2V9.6a2.8 2.8 0 0 1 2.8-2.8h12.2M17 4.4l2.4 2.4L17 9.2",
            "M19.6 12.8v1.6a2.8 2.8 0 0 1-2.8 2.8H4.6M7 19.6l-2.4-2.4L7 14.8",
        ),
    )
    val RepeatOne = outlined(
        "repeat-one",
        paths = arrayOf(
            "M4.4 11.2V9.6a2.8 2.8 0 0 1 2.8-2.8h12.2M17 4.4l2.4 2.4L17 9.2",
            "M19.6 12.8v1.6a2.8 2.8 0 0 1-2.8 2.8H4.6M7 19.6l-2.4-2.4L7 14.8",
            "M11.2 10.6 12.4 9.8v4.6",
        ),
    )
    val Music = outlined(
        "music",
        paths = arrayOf("M9.4 18.2V5.6l10-2v12.2", circle(6.8f, 18.2f, 2.6f), circle(16.8f, 15.8f, 2.6f)),
    )

    val Eye = outlined(
        "eye",
        paths = arrayOf("M2.6 12S6.4 5.8 12 5.8 21.4 12 21.4 12 17.6 18.2 12 18.2 2.6 12 2.6 12Z", circle(12f, 12f, 3f)),
    )
    val Binoculars = outlined(
        "binoculars",
        paths = arrayOf(
            "M4 16a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
            "M14 16a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
            "M16.346 9.17l-.729 -1.261c-.16 -.248 -1.056 -.203 -1.117 .091l-.177 1.38",
            "M19.761 14.813l-2.84 -5.133c-.189 -.31 -.592 -.68 -1.421 -.68c-.828 0 -1.5 .448 -1.5 1v6",
            "M7.654 9.17l.729 -1.261c.16 -.249 1.056 -.203 1.117 .091l.177 1.38",
            "M4.239 14.813l2.84 -5.133c.189 -.31 .592 -.68 1.421 -.68c.828 0 1.5 .448 1.5 1v6",
            "M10 12h4v2h-4l0 -2",
        ),
    )
    val Camera = outlined(
        "camera",
        paths = arrayOf(
            "M15 10l4.553 -2.276a1 1 0 0 1 1.447 .894v6.764a1 1 0 0 1 -1.447 .894l-4.553 -2.276v-4",
            "M3 8a2 2 0 0 1 2 -2h8a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-8a2 2 0 0 1 -2 -2l0 -8",
            "M7 12l4 0",
            "M9 10l0 4",
        ),
    )

    /** The icon of a kind of library. */
    fun ofKind(kind: String): ImageVector = when (kind) {
        "series" -> Series
        "anime" -> Anime
        "shows" -> Shows
        "home_media" -> HomeMedia
        "music" -> Music
        else -> Film
    }
}

/** An icon drawn in one colour. */
@Composable
fun Glyph(icon: ImageVector, color: Color, size: Dp, modifier: Modifier = Modifier) {
    Image(
        painter = rememberVectorPainter(icon),
        contentDescription = null,
        colorFilter = ColorFilter.tint(color),
        modifier = modifier.size(size),
    )
}
