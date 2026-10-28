package app.melyxar.player

import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import kotlinx.serialization.json.put

/** How far behind the camera the picture is let be, in milliseconds. */
private const val BEHIND_AT_MOST_MS = 300L

/** The fastest the picture is played to catch up, and how much of a second
 *  behind adds to the speed. */
private const val FASTEST = 1.5f
private const val SPEED_PER_SECOND_BEHIND = 0.5f
private const val SPEED_STEP = 0.05f

/** How much may come from the camera while the player still waits, in
 *  milliseconds, before the sound is taken to hold it and let go. */
private const val SOUND_WAITED_MS = 1_500L

private const val LOOKED_EVERY_MS = 250L

/**
 * Keeps a camera at now. What has come from the camera and not been shown
 * yet is how far behind the picture is: past [BEHIND_AT_MOST_MS] it is played
 * faster until caught up, so a picture never drifts behind the longer it is
 * watched. A camera can also announce a sound it never sends, or one the
 * player cannot read, which would hold the picture: when the camera keeps
 * sending while the player still waits, it is opened again without its sound.
 */
@OptIn(UnstableApi::class)
internal class LiveEdge(
    private val player: ExoPlayer,
    private val journal: CameraJournal,
    /** Opens the camera again from the start. */
    private val reopen: () -> Unit,
) : AnalyticsListener {
    private val clock = Handler(Looper.getMainLooper())
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
                journal.tell(DELAY, "catching_up") {
                    put("speed", speed)
                    put("behind_ms", player.totalBufferedDuration)
                }
            }
            return
        }
        val soundOn = !player.trackSelectionParameters.disabledTrackTypes.contains(C.TRACK_TYPE_AUDIO)
        if (soundOn && player.playbackState == Player.STATE_BUFFERING && player.totalBufferedDuration > SOUND_WAITED_MS) {
            journal.tell(NET, "reopened_without_sound") { put("received_ms", player.totalBufferedDuration) }
            // Opened again rather than the sound let go while it plays: a
            // camera's streams are chosen once, as it is opened, and choosing
            // again under way loses the picture.
            player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, true)
                .build()
            reopen()
        }
    }

    override fun onPlayerReleased(eventTime: AnalyticsListener.EventTime) {
        clock.removeCallbacks(look)
    }
}
