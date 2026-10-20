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
 * server. Held as little ahead as plays smoothly, so the picture stays under
 * a second behind the camera.
 */
@OptIn(UnstableApi::class)
fun livePlayer(context: Context, address: String): ExoPlayer =
    ExoPlayer.Builder(context)
        .setAudioAttributes(sound, true)
        .setLoadControl(
            DefaultLoadControl.Builder()
                .setBufferDurationsMs(HELD_BEFORE_SHOWN_MS, HELD_AT_MOST_MS, HELD_BEFORE_SHOWN_MS, HELD_BEFORE_SHOWN_MS)
                .build(),
        )
        .build()
        .apply {
            // A camera's sound is often in a form the device's decoder takes
            // without ever giving it back, which holds the picture on its
            // first frame; the picture is what a camera is watched for.
            trackSelectionParameters = trackSelectionParameters.buildUpon()
                .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, true)
                .build()
            addAnalyticsListener(LiveTrace(this, address))
            setMediaSource(RtspMediaSource.Factory().createMediaSource(MediaItem.fromUri(address)))
            prepare()
            playWhenReady = true
        }
