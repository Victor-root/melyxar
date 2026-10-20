package app.melyxar.player

import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener

/** How far behind the camera the picture is let be, in milliseconds. */
private const val BEHIND_AT_MOST_MS = 300L

/** The fastest the picture is played to catch up, and how much of a second
 *  behind adds to the speed. */
private const val FASTEST = 1.5f
private const val SPEED_PER_SECOND_BEHIND = 0.5f
private const val SPEED_STEP = 0.05f

/** How long a shown picture may wait on the sound before the sound is let go. */
private const val SOUND_WAITED_MS = 1_500L

private const val LOOKED_EVERY_MS = 250L

/**
 * Keeps a camera at now. What has come from the camera and not been shown
 * yet is how far behind the picture is: past [BEHIND_AT_MOST_MS] it is played
 * faster until caught up, so a picture never drifts behind the longer it is
 * watched. A camera can also announce a sound it never sends, or one the
 * device cannot decode, which would hold the picture on its first frame: when
 * a shown picture still waits on the sound, the sound is let go.
 */
@OptIn(UnstableApi::class)
internal class LiveEdge(private val player: ExoPlayer, private val trace: LiveTrace) : AnalyticsListener {
    private val clock = Handler(Looper.getMainLooper())
    /** When a picture was shown while the player still waited, until it plays. */
    private var shownWaitingSince = 0L
    private val look = object : Runnable {
        override fun run() {
            keepAtNow()
            clock.postDelayed(this, LOOKED_EVERY_MS)
        }
    }

    init {
        clock.postDelayed(look, LOOKED_EVERY_MS)
    }

    private fun keepAtNow() {
        if (player.playbackState == Player.STATE_READY) {
            val behind = player.totalBufferedDuration - BEHIND_AT_MOST_MS
            val wanted = if (behind > 0) (1f + behind / 1_000f * SPEED_PER_SECOND_BEHIND).coerceAtMost(FASTEST) else 1f
            // In steps, so the speed is not changed on every look.
            val speed = Math.round(wanted / SPEED_STEP) * SPEED_STEP
            if (speed != player.playbackParameters.speed) {
                player.setPlaybackSpeed(speed)
                trace.say("speed $speed with ${player.totalBufferedDuration}ms behind")
            }
            return
        }
        val waited = if (shownWaitingSince == 0L) 0L else System.currentTimeMillis() - shownWaitingSince
        val soundOn = !player.trackSelectionParameters.disabledTrackTypes.contains(C.TRACK_TYPE_AUDIO)
        if (soundOn && waited > SOUND_WAITED_MS) {
            trace.say("sound held the picture ${waited}ms, let go")
            player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, true)
                .build()
        }
    }

    override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) {
        if (player.playbackState != Player.STATE_READY) shownWaitingSince = System.currentTimeMillis()
    }

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        if (state == Player.STATE_READY) shownWaitingSince = 0L
    }

    override fun onPlayerReleased(eventTime: AnalyticsListener.EventTime) {
        clock.removeCallbacks(look)
    }
}
