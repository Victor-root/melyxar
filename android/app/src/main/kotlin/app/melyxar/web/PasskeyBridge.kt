package app.melyxar.web

import android.net.Uri
import android.os.Build
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.annotation.RequiresApi
import androidx.credentials.CreatePublicKeyCredentialRequest
import androidx.credentials.CreatePublicKeyCredentialResponse
import androidx.credentials.CredentialManager
import androidx.credentials.GetCredentialRequest
import androidx.credentials.GetPublicKeyCredentialOption
import androidx.credentials.PublicKeyCredential
import androidx.credentials.exceptions.CreateCredentialCancellationException
import androidx.credentials.exceptions.CreateCredentialException
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.domerrors.InvalidStateError
import androidx.credentials.exceptions.publickeycredential.CreatePublicKeyCredentialDomException
import androidx.lifecycle.lifecycleScope
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import kotlinx.coroutines.launch
import org.json.JSONException
import org.json.JSONObject

/**
 * Passkeys for the page, asked of Android by this application rather than by
 * the WebView.
 *
 * The site a key is made or used for is always the server this window shows,
 * whatever the page asks for: a page of another server added to the
 * application never reaches the keys of this one. The page reaches this
 * through a channel the WebView opens to the server's own origin alone, in
 * its main frame. Only on Android 14 and later, where the system keeps
 * passkeys itself, without Google's services.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class PasskeyBridge private constructor(private val activity: ComponentActivity, private val page: WebView) {
    private val manager = CredentialManager.create(activity)
    private var listening = false

    /** Opens the channel to one server's pages, and to theirs alone. Pages
     *  in plain http have no passkeys, so nothing is opened for them. */
    fun listenTo(origin: String) {
        if (listening) {
            WebViewCompat.removeWebMessageListener(page, NAME)
            listening = false
        }
        val host = Uri.parse(origin).host
        if (!origin.startsWith("https://") || host.isNullOrEmpty()) {
            return
        }
        WebViewCompat.addWebMessageListener(page, NAME, setOf(origin)) { _, message, _, isMainFrame, reply ->
            val asked = message.data
            if (isMainFrame && asked != null) {
                answer(asked, host, reply)
            }
        }
        listening = true
    }

    private fun answer(asked: String, host: String, reply: JavaScriptReplyProxy) {
        val question = try {
            JSONObject(asked)
        } catch (_: JSONException) {
            return
        }
        val id = question.optString("id")
        val options = question.optJSONObject("options") ?: return refuse(reply, id, FAILED)
        when (question.optString("kind")) {
            "make" -> make(options, host, reply, id)
            "use" -> use(options, host, reply, id)
            else -> refuse(reply, id, FAILED)
        }
    }

    private fun make(options: JSONObject, host: String, reply: JavaScriptReplyProxy, id: String) {
        if (options.optJSONObject("rp")?.optString("id") != host) {
            return refuse(reply, id, FAILED)
        }
        activity.lifecycleScope.launch {
            try {
                val made = manager.createCredential(activity, CreatePublicKeyCredentialRequest(options.toString()))
                if (made is CreatePublicKeyCredentialResponse) {
                    answered(reply, id, made.registrationResponseJson)
                } else {
                    refuse(reply, id, FAILED)
                }
            } catch (_: CreateCredentialCancellationException) {
                refuse(reply, id, CANCELLED)
            } catch (error: CreatePublicKeyCredentialDomException) {
                refuse(reply, id, if (error.domError is InvalidStateError) ALREADY_HERE else FAILED)
            } catch (_: CreateCredentialException) {
                refuse(reply, id, FAILED)
            }
        }
    }

    private fun use(options: JSONObject, host: String, reply: JavaScriptReplyProxy, id: String) {
        options.put("rpId", host)
        activity.lifecycleScope.launch {
            try {
                val got = manager.getCredential(
                    activity,
                    GetCredentialRequest(listOf(GetPublicKeyCredentialOption(options.toString()))),
                )
                val credential = got.credential
                if (credential is PublicKeyCredential) {
                    answered(reply, id, credential.authenticationResponseJson)
                } else {
                    refuse(reply, id, FAILED)
                }
            } catch (_: GetCredentialCancellationException) {
                refuse(reply, id, CANCELLED)
            } catch (_: GetCredentialException) {
                refuse(reply, id, FAILED)
            }
        }
    }

    private fun answered(reply: JavaScriptReplyProxy, id: String, credential: String) {
        reply.postMessage(JSONObject().put("id", id).put("credential", JSONObject(credential)).toString())
    }

    private fun refuse(reply: JavaScriptReplyProxy, id: String, why: String) {
        reply.postMessage(JSONObject().put("id", id).put("refused", why).toString())
    }

    companion object {
        /** What the page finds the channel under. */
        private const val NAME = "MelyxarPasskeys"
        private const val CANCELLED = "cancelled"
        private const val ALREADY_HERE = "already_here"
        private const val FAILED = "failed"

        /** The bridge, where this Android and this WebView can carry it. */
        fun of(activity: ComponentActivity, page: WebView): PasskeyBridge? =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE &&
                WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)
            ) {
                PasskeyBridge(activity, page)
            } else {
                null
            }
    }
}
