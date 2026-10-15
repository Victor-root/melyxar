package app.melyxar.web

import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import app.melyxar.music.DeckTold
import app.melyxar.music.Described
import app.melyxar.music.MusicEngine
import kotlinx.serialization.json.Json

/**
 * The page's player of music and the application's engine of sound, word
 * for word what the page's `app-music.ts` asks and hears.
 *
 * Called by the page off the main thread, each request is handed to it. Any
 * frame of the page can call this, so what it names is checked first: only
 * the server's addresses are played, and only the two decks there are.
 */
class MusicBridge(
    private val activity: ComponentActivity,
    private val page: WebView,
    private val engine: MusicEngine,
    private val isTheServers: (String) -> Boolean,
    /** The forms of sound the device plays, as the server reads them. */
    private val forms: () -> String,
    /** Told each time a song is asked to sound, on the main thread. */
    private val onPlay: () -> Unit,
) : MusicEngine.Listener {
    private fun onDeck(deck: Int, request: () -> Unit) {
        if (deck in DECKS) {
            activity.runOnUiThread(request)
        }
    }

    @JavascriptInterface
    fun load(deck: Int, address: String, andPlay: Boolean) {
        if (isTheServers(address)) {
            onDeck(deck) {
                engine.load(deck, address, andPlay)
                if (andPlay) {
                    onPlay()
                }
            }
        }
    }

    @JavascriptInterface
    fun clear(deck: Int) = onDeck(deck) { engine.clear(deck) }

    @JavascriptInterface
    fun play(deck: Int) = onDeck(deck) {
        engine.play(deck)
        onPlay()
    }

    @JavascriptInterface
    fun pause(deck: Int) = onDeck(deck) { engine.pause(deck) }

    @JavascriptInterface
    fun seek(deck: Int, seconds: Double) = onDeck(deck) { engine.seek(deck, seconds) }

    @JavascriptInterface
    fun setSound(deck: Int, volume: Double, muted: Boolean) =
        onDeck(deck) { engine.setSound(deck, volume.toFloat(), muted) }

    @JavascriptInterface
    fun level(deck: Int, level: Double, seconds: Double) =
        onDeck(deck) { engine.level(deck, level.toFloat(), seconds) }

    @JavascriptInterface
    fun handOverAt(from: Int, to: Int, atSeconds: Double, fadeSeconds: Double, level: Double) {
        if (from in DECKS && to in DECKS && from != to) {
            activity.runOnUiThread { engine.handOverAt(from, to, atSeconds, fadeSeconds, level.toFloat()) }
        }
    }

    @JavascriptInterface
    fun cancelHandOver() = activity.runOnUiThread { engine.cancelHandOver() }

    @JavascriptInterface
    fun describe(described: String?) {
        val song = described?.let { READ.decodeFromString<Described>(it) }
        activity.runOnUiThread { engine.describe(song) }
    }

    @JavascriptInterface
    fun forms(): String = forms.invoke()

    override fun told(told: DeckTold) {
        page.evaluateJavascript("window.melyxarMusicTold && window.melyxarMusicTold(${Json.encodeToString(told)})", null)
    }

    override fun asked(asked: String, seconds: Double) {
        page.evaluateJavascript(
            "window.melyxarMusicAsked && window.melyxarMusicAsked(${Json.encodeToString(asked)}, $seconds)",
            null,
        )
    }

    private companion object {
        val DECKS = 0..1
        val READ = Json { ignoreUnknownKeys = true }
    }
}
