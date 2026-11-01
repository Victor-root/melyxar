package app.melyxar.music

import android.annotation.SuppressLint
import android.content.Context
import app.melyxar.server.PageSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * The phone's player of music, one for the application, made by whichever
 * needs it first: the page, or a car asking the service while the
 * application is closed. It asks the server the application opened last as
 * the page's account (see PageSession), keeps its queue and its loudness on
 * the phone, and takes the queue up again as it is made when the account
 * asks for it. Made and used on the main thread.
 */
@SuppressLint("StaticFieldLeak")
object PhoneMusic {
    private var made: Listening? = null
    private var kept: KeptOnThePhone? = null

    fun of(context: Context): Listening = made ?: make(context.applicationContext)

    private fun make(application: Context): Listening {
        val keeping = KeptOnThePhone(application).also { kept = it }
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        val listening = Listening(application, { PageSession.api(application) }, scope, keeping) { address ->
            coverAddress(application, address)
        }
        made = listening
        scope.launch {
            listening.readPreferences()
            if (listening.preferences.resumeQueue) {
                keeping.queue()?.let(listening::takeUp)
            }
        }
        return listening
    }

    /** The page opened, signed in as [account]: a queue another account left
     *  is not this one's to hear, and goes. */
    fun heldBy(account: String) {
        val keeping = kept ?: return
        val before = keeping.heldBy()
        if (before != null && before != account) {
            made?.stop()
        }
        keeping.keepHeldBy(account)
    }
}
