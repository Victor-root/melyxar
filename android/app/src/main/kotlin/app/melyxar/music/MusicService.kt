package app.melyxar.music

import android.app.PendingIntent
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

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

    override fun onCreate() {
        super.onCreate()
        val engine = MusicEngine.ifMade() ?: return
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

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.release()
        session = null
        super.onDestroy()
    }
}
