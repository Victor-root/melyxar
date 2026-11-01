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

/** One song as the card shows it. */
data class Described(
    val id: String,
    val title: String,
    val artist: String,
    val album: String,
    /** The address of its cover, whole, or nothing. */
    val artwork: String?,
    val seconds: Double?,
)

/** What the card shows: the queue in the order it plays, where in it the
 *  song playing is, whether there is one to go on to, and the ways it
 *  plays. */
data class Card(
    val songs: List<Described>,
    val at: Int,
    val canStepOn: Boolean,
    val shuffle: Boolean,
    val repeat: Repeat,
)

/** A press on the card, a headset, a watch or a car, for the player to
 *  answer as it answers its own buttons. */
sealed interface Pressed {
    data object Play : Pressed

    data object Pause : Pressed

    data object Next : Pressed

    data object Previous : Pressed

    data object Stop : Pressed

    data class Seek(val seconds: Double) : Pressed

    /** The song at this place of the order. */
    data class Jump(val place: Int) : Pressed

    data class Shuffled(val on: Boolean) : Pressed

    data class Repeated(val mode: Repeat) : Pressed
}

/**
 * What the device sees of the music: the queue the player describes, the
 * song playing in it, playing or paused as the deck the engine last started
 * is, and buttons that hand every press back to the player, as a press of
 * its own buttons.
 *
 * The notification, the lock screen, a headset, a watch and a car all go
 * through it, by the session the service holds. Even with nothing queued,
 * it takes a song asked for and a press on play, which is how a car starts
 * the music or takes up the queue kept.
 */
@OptIn(UnstableApi::class)
class SessionFace(private val engine: MusicEngine) : SimpleBasePlayer(Looper.getMainLooper()) {
    private var card: Card? = null

    /** The queue as the system sees it, made again only with the card. */
    private var items: List<MediaItemData> = emptyList()

    /** Told apart from the card before, for every song of it to change. */
    private var cards = 0

    fun describe(next: Card?) {
        if (next == card) {
            return
        }
        card = next
        cards += 1
        items = next?.songs?.mapIndexed { place, song -> itemOf(song, place) }.orEmpty()
        invalidateState()
    }

    /** The engine changed what it is doing. */
    fun changed() = invalidateState()

    private fun itemOf(song: Described, place: Int): MediaItemData {
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artist)
            .setAlbumTitle(song.album)
            .setArtworkUri(song.artwork?.toUri())
            .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .build()
        return MediaItemData.Builder("$cards/$place")
            .setMediaItem(MediaItem.Builder().setMediaId(song.id).setMediaMetadata(metadata).build())
            .setMediaMetadata(metadata)
            .setDurationUs(song.seconds?.let { (it * 1_000_000).toLong() } ?: C.TIME_UNSET)
            .setIsSeekable(true)
            .build()
    }

    override fun getState(): State {
        val shown = card
        val commands = Player.Commands.Builder().addAll(Player.COMMAND_PLAY_PAUSE, Player.COMMAND_PREPARE, Player.COMMAND_SET_MEDIA_ITEM)
        if (shown == null) {
            return State.Builder()
                .setAvailableCommands(commands.build())
                .setPlaybackState(Player.STATE_IDLE)
                .build()
        }
        commands
            .addAll(
                Player.COMMAND_STOP,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_MEDIA_ITEM,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_METADATA,
                Player.COMMAND_GET_TIMELINE,
                Player.COMMAND_SET_SHUFFLE_MODE,
                Player.COMMAND_SET_REPEAT_MODE,
            )
            .addIf(Player.COMMAND_SEEK_TO_NEXT, shown.canStepOn)
            .addIf(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM, shown.canStepOn)
        return State.Builder()
            .setAvailableCommands(commands.build())
            .setPlaylist(items)
            .setCurrentMediaItemIndex(shown.at.coerceIn(0, (items.size - 1).coerceAtLeast(0)))
            .setShuffleModeEnabled(shown.shuffle)
            .setRepeatMode(
                when (shown.repeat) {
                    Repeat.Off -> Player.REPEAT_MODE_OFF
                    Repeat.All -> Player.REPEAT_MODE_ALL
                    Repeat.One -> Player.REPEAT_MODE_ONE
                },
            )
            .setPlayWhenReady(engine.liveIsPlaying(), Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setPlaybackState(if (engine.liveIsWaiting()) Player.STATE_BUFFERING else Player.STATE_READY)
            .setContentPositionMs { engine.positionMs() }
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> =
        answered(if (playWhenReady) Pressed.Play else Pressed.Pause)

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> =
        answered(
            when (seekCommand) {
                Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> Pressed.Next
                Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> Pressed.Previous
                Player.COMMAND_SEEK_TO_MEDIA_ITEM -> if (mediaItemIndex != card?.at) Pressed.Jump(mediaItemIndex) else Pressed.Seek(0.0)
                else -> Pressed.Seek(positionMs.coerceAtLeast(0) / 1000.0)
            },
        )

    override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> =
        answered(Pressed.Shuffled(shuffleModeEnabled))

    override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> =
        answered(
            Pressed.Repeated(
                when (repeatMode) {
                    Player.REPEAT_MODE_ALL -> Repeat.All
                    Player.REPEAT_MODE_ONE -> Repeat.One
                    else -> Repeat.Off
                },
            ),
        )

    override fun handleStop(): ListenableFuture<*> = answered(Pressed.Stop)

    /** The songs a car or a voice asked for are already queued by the time
     *  they reach the face (see MusicLibrary): only the answer is left. */
    override fun handleSetMediaItems(mediaItems: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> =
        Futures.immediateVoidFuture()

    override fun handlePrepare(): ListenableFuture<*> = Futures.immediateVoidFuture()

    /** Handed to the player, whose answer comes back as the engine changes. */
    private fun answered(pressed: Pressed): ListenableFuture<*> {
        engine.asked(pressed)
        return Futures.immediateVoidFuture()
    }
}
