package app.melyxar.tv.music

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.melyxar.server.Refusal
import app.melyxar.server.SongSpectrum
import app.melyxar.server.spectrum
import app.melyxar.tv.look.LocalServer
import java.io.IOException

/** The least time between two steps of the levels: they only dress the
 *  screen, so thirty a second is plenty. */
private const val FRAME_EVERY_NS = 33_000_000L

/**
 * The levels of the song playing, read once from the server and followed as
 * it plays, for whatever moves with it: the wave behind the player and the
 * bars of the mark of the song playing, wherever they are. Worked out only
 * while something on screen shows them.
 */
@Stable
class SongLevels {
    /** What the server kept of the song playing, nothing when it kept
     *  nothing, the account does not want the wave, or none plays. */
    var spectrum by mutableStateOf<SongSpectrum?>(null)
        internal set

    /** The levels of the wave as drawn, and those they glide to. */
    var levels = FloatArray(0)
        private set
    private var targets = FloatArray(0)

    /** The heights of the bars of the mark, from nought to one. */
    val bars = FloatArray(BARS)
    private val read = FloatArray(BARS)
    private val meter = BarMeter()

    /** Moved on at each step, read by what draws them so each step draws
     *  them again and does nothing more. */
    var drawn by mutableIntStateOf(0)
        private set

    /** How many things on screen show them. */
    var watchers by mutableIntStateOf(0)
        private set

    @Composable
    internal fun Watched() {
        DisposableEffect(this) {
            watchers += 1
            onDispose { watchers -= 1 }
        }
    }

    /** One step at [moment] of the song [heard] while it plays, or towards
     *  rest, and whether the wave has settled there. Without levels to
     *  follow, the bars keep beats of their own while the song plays. */
    internal fun step(heard: SongSpectrum?, playing: Boolean, moment: Double, nowMs: Long, sinceMs: Long): Boolean {
        if (heard != null && levels.size != heard.bands) {
            levels = levels.copyOf(heard.bands)
            targets = FloatArray(heard.bands)
        }
        if (heard != null && playing) {
            levelsAt(heard, moment, targets)
            barsOf(targets, read)
            meter.step(read, sinceMs)
            meter.shown.copyInto(bars)
        } else {
            targets.fill(0f)
            if (playing) {
                for (bar in 0 until BARS) bars[bar] = ownBeat(bar, nowMs)
            }
        }
        val settled = approach(levels, targets)
        drawn += 1
        return settled
    }
}

val LocalSongLevels = staticCompositionLocalOf { SongLevels() }

/** Where the song was when the player last said, and when that was. */
private class Said {
    var position = 0.0
    var at = 0L
}

/** Keeps [levels] following the song playing while anything shows them;
 *  set once, over every screen. */
@Composable
fun FollowSongLevels(levels: SongLevels) {
    val listening = LocalListening.current
    val api = LocalServer.current
    val song = listening.queue.current?.id
    val wanted = listening.preferences.spectrum
    LaunchedEffect(song, wanted) {
        levels.spectrum = null
        if (song != null && wanted) {
            levels.spectrum = try {
                api.spectrum(song)
            } catch (_: IOException) {
                null
            } catch (_: Refusal) {
                null
            }
        }
    }
    val said = remember { Said() }
    LaunchedEffect(listening) {
        snapshotFlow { listening.position }.collect {
            said.position = it
            said.at = System.nanoTime()
        }
    }
    // The levels this composition saw: the state may already hold the next
    // song's, of another size.
    val heard = levels.spectrum
    val playing = listening.playing
    LaunchedEffect(heard, playing, levels.watchers > 0) {
        if (levels.watchers == 0) {
            return@LaunchedEffect
        }
        var last = 0L
        var going = true
        while (going) {
            going = withFrameNanos { now ->
                if (now - last < FRAME_EVERY_NS) {
                    return@withFrameNanos true
                }
                val since = if (last == 0L) 0L else (now - last) / 1_000_000
                last = now
                val settled = levels.step(heard, playing, momentOf(said.position, (now - said.at) / 1_000_000), now / 1_000_000, since)
                playing || !settled
            }
        }
    }
}

/** The width of a bar of the mark, the room between two, and the height
 *  it is drawn at when nothing says otherwise. */
private val BAR = 3.dp
private val BAR_GAP = 2.dp

/**
 * The mark of the song playing, as the web's: four small bars, as a little
 * equaliser would stand, rising and falling with the song's levels, or on
 * beats of their own when it has none.
 */
@Composable
fun PlayingBars(color: Color, tall: Dp, modifier: Modifier = Modifier) {
    val levels = LocalSongLevels.current
    levels.Watched()
    Spacer(
        modifier
            .size(BAR * BARS + BAR_GAP * (BARS - 1), tall)
            .drawBehind {
                // Read here, so each step draws them again and does nothing
                // more; nothing to draw before the first.
                if (levels.drawn == 0) {
                    return@drawBehind
                }
                val wide = BAR.toPx()
                val apart = wide + BAR_GAP.toPx()
                for (bar in 0 until BARS) {
                    val high = size.height * levels.bars[bar].coerceIn(0.15f, 1f)
                    drawRoundRect(color, Offset(bar * apart, size.height - high), Size(wide, high), CornerRadius(wide / 2))
                }
            },
    )
}
