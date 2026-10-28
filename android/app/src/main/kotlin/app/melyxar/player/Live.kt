package app.melyxar.player

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.rtsp.RtspMediaSource

/** How much a camera is held ahead before it shows, and at most, in
 *  milliseconds: a moment, so what is shown is what is happening. */
private const val HELD_BEFORE_SHOWN_MS = 300
private const val HELD_AT_MOST_MS = 1_500

private val sound = AudioAttributes.Builder()
    .setUsage(C.USAGE_MEDIA)
    .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
    .build()

/**
 * A camera watched live: the application opens its RTSP address itself, with
 * the account the server wrote into it, and nothing passes through the
 * server. Held as little ahead as plays smoothly, and kept there by
 * [LiveEdge], so the picture stays under a second behind the camera. All
 * of it is told to [journal]; the exchange with the camera is heard for it,
 * which is why the RTSP client says what it does.
 */
@OptIn(UnstableApi::class)
fun livePlayer(context: Context, address: String, journal: CameraJournal): ExoPlayer =
    ExoPlayer.Builder(context)
        .setAudioAttributes(sound, true)
        .setLoadControl(
            DefaultLoadControl.Builder()
                .setBufferDurationsMs(HELD_BEFORE_SHOWN_MS, HELD_AT_MOST_MS, HELD_BEFORE_SHOWN_MS, HELD_BEFORE_SHOWN_MS)
                .build(),
        )
        .build()
        .apply {
            LiveTrace.hearTheExchange()
            val open = {
                setMediaSource(
                    RtspMediaSource.Factory()
                        .setDebugLoggingEnabled(true)
                        .createMediaSource(MediaItem.fromUri(address)),
                )
                prepare()
            }
            journal.openingAt(address)
            addAnalyticsListener(LiveTrace(this, journal))
            addAnalyticsListener(LiveEdge(this, journal, reopen = open))
            open()
            playWhenReady = true
        }
