package app.melyxar.music

import android.app.PendingIntent
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import app.melyxar.BuildConfig
import app.melyxar.R

/**
 * What keeps the music playing with the screen off or the application left:
 * a service in the foreground while a song plays, with the card of the
 * notifications and the lock screen. Media3 puts it there and takes it away
 * as the music plays and stops.
 *
 * It holds the session of the engine's card, and nothing else: the engine
 * is made by the screen that leads it, which is always there before it.
 */
@OptIn(UnstableApi::class)
class MusicService : MediaSessionService() {
    private var session: MediaSession? = null
    private val probe = if (BuildConfig.DEBUG) LockScreenProbe(this) { session } else null

    override fun onCreate() {
        super.onCreate()
        probe?.start()
        val engine = MusicEngine.ifMade() ?: return
        // Melyxar's own mark in the status bar, rather than a player's.
        setMediaNotificationProvider(DefaultMediaNotificationProvider(this).apply { setSmallIcon(R.drawable.ic_launcher_monochrome) })
        session = MediaSession.Builder(this, engine.face)
            // The covers are the server's, fetched signed in as the page is.
            .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(engine.fetching).build())
            // A press on the card brings the application back as it was left,
            // as its icon does.
            .apply {
                packageManager.getLaunchIntentForPackage(packageName)?.let { opening ->
                    setSessionActivity(PendingIntent.getActivity(this@MusicService, 0, opening, PendingIntent.FLAG_IMMUTABLE))
                }
            }
            .build()
    }

    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        super.onUpdateNotification(session, startInForegroundRequired)
        probe?.report("card updated, foreground=$startInForegroundRequired")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        probe?.stop()
        session?.release()
        session = null
        super.onDestroy()
    }
}
