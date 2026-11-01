package app.melyxar.music

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.os.Build
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.session.CommandButton
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaLibraryService.LibraryParams
import androidx.media3.session.MediaLibraryService.MediaLibrarySession
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionError
import androidx.media3.session.SessionResult
import app.melyxar.R
import app.melyxar.car.CarCatalogue
import app.melyxar.car.NobodySignedIn
import app.melyxar.server.PageSession
import app.melyxar.tv.isTelevision
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.SettableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.IOException

/** Android Auto, the only car that may read the music and its covers
 *  without being trusted by the system. */
private const val ANDROID_AUTO = "com.google.android.projection.gearhead"

private val SHUFFLE = SessionCommand("app.melyxar.SHUFFLE", Bundle.EMPTY)
private val REPEAT = SessionCommand("app.melyxar.REPEAT", Bundle.EMPTY)

/**
 * What keeps the music playing with the screen off or the application left:
 * a service in the foreground while a song plays, with the card of the
 * notifications and the lock screen. Media3 puts it there and takes it away
 * as the music plays and stops.
 *
 * It holds the session of the engine's card, the buttons shuffle and repeat
 * beside it, and, on a phone, the catalogue a car browses (CarCatalogue).
 * On a phone, the player is made here when a car asks for music with the
 * application closed; on a television, by the screen that leads it, which
 * is always there before it.
 */
@OptIn(UnstableApi::class)
class MusicService : MediaLibraryService() {
    private var session: MediaLibrarySession? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    override fun onCreate() {
        super.onCreate()
        val listening = if (isTelevision()) null else PhoneMusic.of(this)
        val engine = MusicEngine.ifMade() ?: listening?.engineNow() ?: return
        val catalogue = listening?.let { CarCatalogue(this, it) { PageSession.api(this) } }
        // Melyxar's own mark in the status bar, rather than a player's.
        setMediaNotificationProvider(
            DefaultMediaNotificationProvider.Builder(this)
                .setChannelId(silentChannel())
                .setChannelName(R.string.music_card_channel)
                .build()
                .apply { setSmallIcon(R.drawable.ic_launcher_monochrome) },
        )
        val made = MediaLibrarySession.Builder(this, engine.face, Library(engine, catalogue))
            // The covers are the server's, fetched signed in as the music is,
            // or the phone's own addresses of them for a car.
            .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(DefaultDataSource.Factory(this, engine.fetching)).build())
            .setMediaButtonPreferences(buttons(engine.face))
            // A press on the card brings the application back as it was left,
            // as its icon does.
            .apply {
                packageManager.getLaunchIntentForPackage(packageName)?.let { opening ->
                    setSessionActivity(PendingIntent.getActivity(this@MusicService, 0, opening, PendingIntent.FLAG_IMMUTABLE))
                }
            }
            .build()
        session = made
        // The buttons show the ways the queue plays now.
        engine.face.addListener(object : Player.Listener {
            override fun onShuffleModeEnabledChanged(shuffleModeEnabled: Boolean) {
                made.setMediaButtonPreferences(buttons(engine.face))
            }

            override fun onRepeatModeChanged(repeatMode: Int) {
                made.setMediaButtonPreferences(buttons(engine.face))
            }
        })
    }

    /** Shuffle and repeat, each drawn as it stands, beside the card's own
     *  buttons. */
    private fun buttons(face: SessionFace): List<CommandButton> = listOf(
        CommandButton.Builder(if (face.shuffleModeEnabled) CommandButton.ICON_SHUFFLE_ON else CommandButton.ICON_SHUFFLE_OFF)
            .setDisplayName(getString(R.string.tv_music_shuffle))
            .setSessionCommand(SHUFFLE)
            .build(),
        CommandButton.Builder(
            when (face.repeatMode) {
                Player.REPEAT_MODE_ALL -> CommandButton.ICON_REPEAT_ALL
                Player.REPEAT_MODE_ONE -> CommandButton.ICON_REPEAT_ONE
                else -> CommandButton.ICON_REPEAT_OFF
            },
        )
            .setDisplayName(
                getString(
                    when (face.repeatMode) {
                        Player.REPEAT_MODE_ALL -> R.string.tv_music_repeat_all
                        Player.REPEAT_MODE_ONE -> R.string.tv_music_repeat_one
                        else -> R.string.tv_music_repeat_off
                    },
                ),
            )
            .setSessionCommand(REPEAT)
            .build(),
    )

    /**
     * What the session answers: the buttons for everyone, and the catalogue
     * only to Android Auto, the application itself and what the system
     * trusts; any other application that comes to the service has the
     * buttons alone.
     */
    private inner class Library(private val engine: MusicEngine, private val catalogue: CarCatalogue?) : MediaLibrarySession.Callback {
        /** The last search, read once for the count the car asks first and
         *  the songs it asks after. */
        private var searched: Pair<String, List<MediaItem>>? = null

        private suspend fun found(reading: CarCatalogue, query: String): List<MediaItem> =
            searched?.takeIf { it.first == query }?.second ?: reading.search(query).also { searched = query to it }

        override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
            val browses = catalogue != null &&
                (controller.packageName == packageName || controller.packageName == ANDROID_AUTO || controller.isTrusted)
            if (browses && controller.packageName != packageName) {
                letReadTheCovers(this@MusicService, controller.packageName)
            }
            val commands = if (browses) {
                MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS
            } else {
                MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS
            }
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session, controller)
                .setAvailableSessionCommands(commands.buildUpon().add(SHUFFLE).add(REPEAT).build())
                .build()
        }

        override fun onCustomCommand(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            customCommand: SessionCommand,
            args: Bundle,
        ): ListenableFuture<SessionResult> {
            val face = engine.face
            when (customCommand.customAction) {
                SHUFFLE.customAction -> engine.asked(Pressed.Shuffled(!face.shuffleModeEnabled))
                REPEAT.customAction -> engine.asked(
                    Pressed.Repeated(
                        when (face.repeatMode) {
                            Player.REPEAT_MODE_OFF -> Repeat.All
                            Player.REPEAT_MODE_ALL -> Repeat.One
                            else -> Repeat.Off
                        },
                    ),
                )
                else -> return Futures.immediateFuture(SessionResult(SessionError.ERROR_NOT_SUPPORTED))
            }
            return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
        }

        override fun onGetLibraryRoot(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<MediaItem>> {
            // The system's own question of what played last is left to the
            // card, which already shows it.
            val reading = catalogue?.takeUnless { params?.isRecent == true }
                ?: return Futures.immediateFuture(LibraryResult.ofError(SessionError.ERROR_NOT_SUPPORTED))
            return Futures.immediateFuture(LibraryResult.ofItem(reading.root(), reading.drawn()))
        }

        override fun onGetChildren(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            parentId: String,
            page: Int,
            pageSize: Int,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = read { reading ->
            LibraryResult.ofItemList(reading.children(parentId, page, pageSize), params)
        }

        override fun onGetItem(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            mediaId: String,
        ): ListenableFuture<LibraryResult<MediaItem>> = read { reading ->
            reading.item(mediaId)?.let { LibraryResult.ofItem(it, null) } ?: LibraryResult.ofError(SessionError.ERROR_BAD_VALUE)
        }

        override fun onSearch(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<Void>> = read { reading ->
            session.notifySearchResultChanged(browser, query, found(reading, query).size, params)
            LibraryResult.ofVoid(params)
        }

        override fun onGetSearchResult(
            session: MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            page: Int,
            pageSize: Int,
            params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = read { reading ->
            LibraryResult.ofItemList(found(reading, query).drop(page * pageSize).take(pageSize), params)
        }

        /** A song, a list or a voice asked for: played here, the song that
         *  plays first handed back as what was set. */
        override fun onSetMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: List<MediaItem>,
            startIndex: Int,
            startPositionMs: Long,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
            val reading = catalogue ?: return Futures.immediateFailedFuture(UnsupportedOperationException())
            val asked = mediaItems.getOrNull(startIndex) ?: mediaItems.firstOrNull()
                ?: return Futures.immediateFailedFuture(IllegalArgumentException("nothing asked"))
            return answered {
                val first = reading.play(asked.mediaId, asked.requestMetadata.searchQuery) ?: throw IOException("nothing to play")
                MediaSession.MediaItemsWithStartPosition(listOf(first), 0, 0)
            }
        }

        /** Play pressed with nothing queued, the queue kept taken up first. */
        override fun onPlaybackResumption(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            isForPlayback: Boolean,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
            val reading = catalogue ?: return Futures.immediateFailedFuture(UnsupportedOperationException())
            return answered {
                val first = reading.play("", null) ?: throw IOException("nothing to play")
                MediaSession.MediaItemsWithStartPosition(listOf(first), 0, 0)
            }
        }

        /** The catalogue read away from the call, its failures said to the
         *  car in words it shows: sign in first, or the server is away. */
        private fun <T : Any> read(reading: suspend (CarCatalogue) -> LibraryResult<T>): ListenableFuture<LibraryResult<T>> {
            val held = catalogue ?: return Futures.immediateFuture(LibraryResult.ofError<T>(SessionError.ERROR_NOT_SUPPORTED))
            val answer = SettableFuture.create<LibraryResult<T>>()
            scope.launch {
                val result: LibraryResult<T> = try {
                    reading(held)
                } catch (_: NobodySignedIn) {
                    LibraryResult.ofError(SessionError(SessionError.ERROR_SESSION_AUTHENTICATION_EXPIRED, getString(R.string.car_sign_in)))
                } catch (_: IOException) {
                    LibraryResult.ofError(SessionError(SessionError.ERROR_IO, getString(R.string.car_unreachable)))
                }
                answer.set(result)
            }
            return answer
        }

        private fun <T> answered(work: suspend () -> T): ListenableFuture<T> {
            val answer = SettableFuture.create<T>()
            scope.launch {
                try {
                    answer.set(work())
                } catch (failure: NobodySignedIn) {
                    answer.setException(failure)
                } catch (failure: IOException) {
                    answer.setException(failure)
                }
            }
            return answer
        }
    }

    /**
     * The category the card is posted in, made silent here rather than left
     * to Media3. Its own came out loud on some phones (measured on an Oppo:
     * high importance with the notification sound, rung each time the card
     * first appeared), and a category keeps what it was made with for good,
     * so that one is deleted and this one is made silent from the start.
     */
    private fun silentChannel(): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = getSystemService(NotificationManager::class.java)
            manager.deleteNotificationChannel(MEDIA3_CHANNEL)
            if (manager.getNotificationChannel(CHANNEL) == null) {
                manager.createNotificationChannel(
                    NotificationChannel(CHANNEL, getString(R.string.music_card_channel), NotificationManager.IMPORTANCE_LOW).apply {
                        setSound(null, null)
                        enableVibration(false)
                        setShowBadge(false)
                    },
                )
            }
        }
        return CHANNEL
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaLibrarySession? = session

    override fun onDestroy() {
        scope.cancel()
        session?.release()
        session = null
        super.onDestroy()
    }

    private companion object {
        const val CHANNEL = "music_card"
        const val MEDIA3_CHANNEL = "default_channel_id"
    }
}
