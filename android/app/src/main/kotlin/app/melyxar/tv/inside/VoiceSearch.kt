package app.melyxar.tv.inside

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.core.content.ContextCompat
import app.melyxar.R
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.RoundMark

/**
 * Searching by voice without leaving the screen: the television's own
 * recogniser is listened to directly, with no window of its own over
 * Melyxar's, and what it hears is written into the field as it hears it.
 */
@Stable
class Voice internal constructor(
    private val context: Context,
    private val askToHear: () -> Unit,
) {
    var listening by mutableStateOf(false)
        private set

    internal var onHeard: (String) -> Unit = {}
    private var recogniser: SpeechRecognizer? = null

    /** Starts listening, asking first for the microphone when it was never
     *  granted; a second press while listening stops. */
    fun toggle() {
        when {
            listening -> stop()
            ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED -> start()
            else -> askToHear()
        }
    }

    internal fun start() {
        val recogniser = recogniser ?: SpeechRecognizer.createSpeechRecognizer(context).also {
            it.setRecognitionListener(heard)
            recogniser = it
        }
        listening = true
        recogniser.startListening(
            Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true),
        )
    }

    fun stop() {
        recogniser?.stopListening()
        listening = false
    }

    internal fun release() {
        recogniser?.destroy()
        recogniser = null
        listening = false
    }

    private val heard = object : RecognitionListener {
        override fun onPartialResults(partialResults: Bundle) = said(partialResults)

        override fun onResults(results: Bundle) {
            said(results)
            listening = false
        }

        override fun onError(error: Int) {
            listening = false
        }

        override fun onEndOfSpeech() = Unit
        override fun onReadyForSpeech(params: Bundle?) = Unit
        override fun onBeginningOfSpeech() = Unit
        override fun onRmsChanged(rmsdB: Float) = Unit
        override fun onBufferReceived(buffer: ByteArray?) = Unit
        override fun onEvent(eventType: Int, params: Bundle?) = Unit
    }

    private fun said(results: Bundle) {
        results.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            ?.firstOrNull()
            ?.takeIf { it.isNotBlank() }
            ?.let(onHeard)
    }
}

/** The voice of a screen, writing what it hears with [onHeard]; nothing on
 *  a television with nothing to listen with. */
@Composable
fun rememberVoice(onHeard: (String) -> Unit): Voice? {
    val context = LocalContext.current
    if (!remember { SpeechRecognizer.isRecognitionAvailable(context) }) {
        return null
    }
    val writes by rememberUpdatedState(onHeard)
    var voice: Voice? = null
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) voice?.start()
    }
    voice = remember { Voice(context) { ask.launch(Manifest.permission.RECORD_AUDIO) } }
    voice.onHeard = { writes(it) }
    DisposableEffect(voice) {
        onDispose { voice.release() }
    }
    return voice
}

/** The microphone beside a field, lit while it listens. */
@Composable
fun Listen(voice: Voice?) {
    voice ?: return
    RoundMark(
        Icons.Microphone,
        label = stringResource(if (voice.listening) R.string.tv_search_listening else R.string.tv_search_voice),
        on = voice.listening,
        onClick = voice::toggle,
    )
}

/** The remote's own key for searching starts listening, wherever the remote
 *  is on the screen. */
fun Modifier.listensOnItsKey(voice: Voice?): Modifier = if (voice == null) this else onPreviewKeyEvent { event ->
    val ours = event.key == Key.Search || event.key == Key.VoiceAssist
    if (ours && event.type == KeyEventType.KeyUp) voice.toggle()
    ours
}
