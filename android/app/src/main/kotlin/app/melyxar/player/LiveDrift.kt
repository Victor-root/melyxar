package app.melyxar.player

import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener

/**
 * How far a camera watched live has fallen behind the clock since its
 * picture first played: each moment it waited for the camera is time the
 * picture never makes up, and these add up over the hours. Read again from
 * nought each time the camera is opened again or brought back to live.
 */
@OptIn(UnstableApi::class)
internal class LiveDrift(private val player: ExoPlayer) : AnalyticsListener {
    private var playingSince = 0L
    private var positionAtPlaying = 0L

    fun fellBehind(): Long =
        if (playingSince > 0) (SystemClock.elapsedRealtime() - playingSince) - (player.currentPosition - positionAtPlaying) else 0

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        if (state == Player.STATE_READY && playingSince == 0L) {
            playingSince = SystemClock.elapsedRealtime()
            positionAtPlaying = player.currentPosition
        }
    }

    override fun onTimelineChanged(eventTime: AnalyticsListener.EventTime, reason: Int) {
        if (reason == Player.TIMELINE_CHANGE_REASON_PLAYLIST_CHANGED) playingSince = 0
    }

    override fun onPositionDiscontinuity(
        eventTime: AnalyticsListener.EventTime,
        oldPosition: Player.PositionInfo,
        newPosition: Player.PositionInfo,
        reason: Int,
    ) {
        if (reason == Player.DISCONTINUITY_REASON_SEEK) playingSince = 0
    }
}
