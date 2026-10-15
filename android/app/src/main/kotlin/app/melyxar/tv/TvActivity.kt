package app.melyxar.tv

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.activity.compose.setContent
import app.melyxar.server.KnownServers
import app.melyxar.server.Sessions

/**
 * The application on a television: every screen of it drawn here, natively,
 * for the remote, from the server's choice to the film.
 */
class TvActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // The whole window to draw in, so the screens hear of the keyboard
        // and lift what is typed in above it, on every version of Android.
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val servers = KnownServers(this)
        val sessions = Sessions(this)
        setContent { TvApp(servers, sessions, onLeave = ::finish) }
    }
}
