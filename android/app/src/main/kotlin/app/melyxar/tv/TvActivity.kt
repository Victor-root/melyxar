package app.melyxar.tv

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.activity.compose.setContent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.lifecycleScope
import app.melyxar.R
import app.melyxar.server.KnownServers
import app.melyxar.server.Sessions
import app.melyxar.server.network
import app.melyxar.tv.look.TvScale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * The application on a television: every screen of it drawn here, natively,
 * for the remote, from the server's choice to the film.
 */
class TvActivity : ComponentActivity() {
    /** The work to open, asked from the system's Continue watching row. */
    private var opening by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        // The whole window to draw in, so the screens hear of the keyboard
        // and lift what is typed in above it, on every version of Android.
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        // Built while the window comes up rather than in its first frame:
        // making the client readies everything a secure connection needs.
        lifecycleScope.launch(Dispatchers.IO) { network }
        // Read away from the first frame too, into the store its text reads
        // them from: each font file is parsed once, slowly on a small box.
        lifecycleScope.launch(Dispatchers.IO) { FONTS.forEach { ResourcesCompat.getFont(this@TvActivity, it) } }
        val servers = KnownServers(this)
        val sessions = Sessions(this)
        opening = intent.getStringExtra(OPENING)
        setContent { TvScale { TvApp(servers, sessions, opening, onOpened = { opening = null }, onLeave = ::finish) } }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.getStringExtra(OPENING)?.let { opening = it }
    }

    companion object {
        /** The fonts every screen of the television writes in. */
        private val FONTS = listOf(R.font.inter_regular, R.font.inter_medium, R.font.inter_semibold, R.font.inter_bold, R.font.space_grotesk_bold)

        /** The extra naming the work to open. */
        const val OPENING = "app.melyxar.tv.OPENING"
    }
}
