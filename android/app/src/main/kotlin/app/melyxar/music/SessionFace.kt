package app.melyxar.music

import android.os.Looper
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

/**
 * What the phone sees of the music: one song, the one the page describes,
 * playing or paused as the deck the engine last started is, and buttons that
 * hand every press back to the page, as a press of its own buttons.
 *
 * The notification, the lock screen, a headset and a watch all go through
 * it, by the session the service holds.
 */
@OptIn(UnstableApi::class)
class SessionFace(private val engine: MusicEngine) : SimpleBasePlayer(Looper.getMainLooper()) {
    private var described: Described? = null

    /** Told apart from the song before, for the card to change with it. */
    private var songs = 0

    fun describe(song: Described?) {
        if (song != described) {
            described = song
            songs += 1
            invalidateState()
        }
    }

    /** The engine changed what it is doing. */
    fun changed() = invalidateState()

    override fun getState(): State {
        val song = described
            ?: return State.Builder()
                .setAvailableCommands(Player.Commands.EMPTY)
                .setPlaybackState(Player.STATE_IDLE)
                .build()
        val commands = Player.Commands.Builder()
            .addAll(
                Player.COMMAND_PLAY_PAUSE,
                Player.COMMAND_STOP,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_METADATA,
                Player.COMMAND_GET_TIMELINE,
            )
            .addIf(Player.COMMAND_SEEK_TO_NEXT, song.canStepOn)
            .addIf(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM, song.canStepOn)
            .build()
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artist)
            .setAlbumTitle(song.album)
            .setArtworkUri(song.artwork?.toUri())
            .build()
        val item = MediaItemData.Builder(songs)
            .setMediaItem(MediaItem.Builder().setMediaId(songs.toString()).setMediaMetadata(metadata).build())
            .setMediaMetadata(metadata)
            .setDurationUs(song.seconds?.let { (it * 1_000_000).toLong() } ?: C.TIME_UNSET)
            .setIsSeekable(true)
            .build()
        return State.Builder()
            .setAvailableCommands(commands)
            .setPlaylist(listOf(item))
            .setPlayWhenReady(engine.liveIsPlaying(), Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setPlaybackState(if (engine.liveIsWaiting()) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { engine.livePositionMs() }
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> =
        answered { engine.asked(if (playWhenReady) "play" else "pause") }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> =
        answered {
            when (seekCommand) {
                Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> engine.asked("next")
                Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> engine.asked("previous")
                else -> engine.asked("seek", positionMs / 1000.0)
            }
        }

    override fun handleStop(): ListenableFuture<*> = answered { engine.asked("stop") }

    /** Handed to the page, whose answer comes back as the engine changes. */
    private fun answered(press: () -> Unit): ListenableFuture<*> {
        press()
        return Futures.immediateVoidFuture()
    }
}
