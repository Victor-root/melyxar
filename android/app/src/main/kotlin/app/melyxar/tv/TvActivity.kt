package app.melyxar.tv

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import app.melyxar.server.KnownServers
import app.melyxar.server.Sessions

/**
 * The application on a television: every screen of it drawn here, natively,
 * for the remote, from the server's choice to the film.
 */
class TvActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val servers = KnownServers(this)
        val sessions = Sessions(this)
        setContent { TvApp(servers, sessions, onLeave = ::finish) }
    }
}
