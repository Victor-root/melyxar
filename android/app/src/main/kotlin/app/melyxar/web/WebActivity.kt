package app.melyxar.web

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.Manifest
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.RenderProcessGoneDetail
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import app.melyxar.BuildConfig
import app.melyxar.R
import app.melyxar.connect.ConnectActivity
import app.melyxar.music.MusicEngine
import app.melyxar.music.MusicService
import app.melyxar.player.FilmPlayer
import app.melyxar.player.measuredMusicForms
import app.melyxar.player.serverFetching
import app.melyxar.server.KnownServer
import app.melyxar.theme.Melyxar
import app.melyxar.theme.MelyxarTheme
import app.melyxar.theme.drawUnderTheBars

/**
 * The interface of a server, as its own web pages draw it.
 *
 * Everything shown here is the server's: a server brought up to date brings
 * this screen with it. What is native is only what a page cannot do well on
 * a phone, the screen saying the server does not answer, and the film itself,
 * played beneath the page by the device's own player (see FilmPlayer).
 */
class WebActivity : ComponentActivity() {
    private lateinit var page: WebView
    private lateinit var server: KnownServer
    private lateinit var notice: ComposeView
    private lateinit var film: FilmPlayer
    private lateinit var pageWindow: PageWindow
    private lateinit var barShades: BarShades
    private lateinit var music: MusicEngine
    private var musicService: ListenableFuture<MediaController>? = null
    private val chrome = InterfaceChrome()

    /** The page waiting for files somebody picks, while the picker is open. */
    private var picking: ValueCallback<Array<Uri>>? = null
    private val picker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        picking?.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(result.resultCode, result.data))
        picking = null
    }

    /** Asked once a sitting, and only of a phone that asks for it: a phone
     *  answers by itself after the second refusal, and never asks again. */
    private var notificationsAsked = false
    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    /** The card of the music shows on the lock screen of some phones only for
     *  an application allowed to notify: asked at the first song played. */
    private fun askToShowTheMusic() {
        if (notificationsAsked || Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return
        }
        notificationsAsked = true
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    /** What the page put into full screen, a video, while it is there. */
    private var fullScreen: View? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        server = KnownServer(
            origin = requireNotNull(intent.getStringExtra(ORIGIN)),
            name = intent.getStringExtra(NAME).orEmpty(),
        )
        drawUnderTheBars()

        val holder = FrameLayout(this)
        page = interfacePage()
        pageWindow = PageWindow(this, holder, page)
        barShades = BarShades(this, page)
        film = FilmPlayer(this, page, Melyxar.ground.toArgb(), ::isTheServers, barShades::filmShown)
        page.addJavascriptInterface(film, "MelyxarPlayer")
        page.addJavascriptInterface(pageWindow, "MelyxarWindow")
        page.addJavascriptInterface(LauncherIcons(applicationContext), "MelyxarIcon")
        page.addJavascriptInterface(LiveActivity.Bridge(this), "MelyxarCamera")
        music = MusicEngine.of(this) { serverFetching(page.settings.userAgentString) }
        val musicBridge = MusicBridge(this, page, music, ::isTheServers, ::measuredMusicForms, ::askToShowTheMusic)
        music.listener = musicBridge
        page.addJavascriptInterface(musicBridge, "MelyxarMusic")
        // The service that keeps the music going, woken now so it is there
        // the moment a song starts.
        musicService = MediaController.Builder(this, SessionToken(this, ComponentName(this, MusicService::class.java)))
            .buildAsync()
        holder.addView(film.picture, FrameLayout.LayoutParams(MATCH, MATCH))
        holder.addView(page, FrameLayout.LayoutParams(MATCH, MATCH))
        notice = ComposeView(this).apply {
            visibility = View.GONE
            setContent {
                MelyxarTheme {
                    Unreachable(
                        name = server.name.ifEmpty { server.origin },
                        onRetry = {
                            showUnreachable(false)
                            page.reload()
                        },
                        onChange = {
                            startActivity(ConnectActivity.choosing(this@WebActivity))
                            finish()
                        },
                    )
                }
            }
        }
        holder.addView(notice, FrameLayout.LayoutParams(MATCH, MATCH))
        setContentView(holder)

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                // Every step the page takes, a window or a panel it opens
                // included, is a step in its history; with none left, the
                // application goes behind the others as every one does from
                // Android 12, and comes back where it was rather than loaded
                // again from the start.
                when {
                    fullScreen != null -> chrome.onHideCustomView()
                    page.canGoBack() -> page.goBack()
                    else -> moveTaskToBack(true)
                }
            }
        })

        if (savedInstanceState == null || page.restoreState(savedInstanceState) == null) {
            page.loadUrl(server.origin)
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun interfacePage() = WebView(this).apply {
        setBackgroundColor(Melyxar.ground.toArgb())
        WebView.setWebContentsDebuggingEnabled(BuildConfig.TRACES)
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        // How the server and the page tell the application from a browser.
        settings.userAgentString = "${settings.userAgentString} MelyxarApp/${BuildConfig.VERSION_NAME}"
        // The page leads the music, so it stays as alive as the application,
        // which the music keeps in the foreground, when it is out of sight.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            setRendererPriorityPolicy(WebView.RENDERER_PRIORITY_IMPORTANT, false)
        }
        CookieManager.getInstance().setAcceptCookie(true)
        webViewClient = InterfaceClient()
        webChromeClient = chrome
    }

    private fun showUnreachable(shown: Boolean) {
        notice.visibility = if (shown) View.VISIBLE else View.GONE
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        page.saveState(outState)
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        film.leaving()
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        film.windowed(isInPictureInPictureMode)
    }

    override fun onStop() {
        super.onStop()
        film.stopped()
    }

    override fun onPause() {
        super.onPause()
        // The session cookie is written to the disk now, not whenever the
        // system gets round to it: a phone that kills the application in the
        // background must not sign anybody out.
        CookieManager.getInstance().flush()
    }

    override fun onDestroy() {
        film.release()
        musicService?.let(MediaController::releaseFuture)
        music.listener = null
        // Left for good, swept away from the recent ones: the page that led
        // the music is gone, and the music with it.
        if (!isChangingConfigurations) {
            music.stopAll()
        }
        page.destroy()
        super.onDestroy()
    }

    /** Whether an address is one of the server's own pages. */
    private fun isTheServers(address: String) = address == server.origin ||
        address.startsWith("${server.origin}/") ||
        address.startsWith("${server.origin}?")

    // The check counts the call to the parent's constructor that a Kotlin
    // class makes as a bare WebViewClient made without that method: this one
    // has it, below.
    @SuppressLint("MissingOnRenderProcessGone")
    private inner class InterfaceClient : WebViewClient() {
        /** The server's own pages stay here; anything else opens where it
         *  belongs, in the browser or the application that handles it. */
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val target = request.url
            if (isTheServers(target.toString())) {
                return false
            }
            try {
                startActivity(Intent(Intent.ACTION_VIEW, target))
            } catch (_: ActivityNotFoundException) {
                // Nothing on the device opens it: the press does nothing,
                // which is what a link to nowhere does in a browser too.
            }
            return true
        }

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            film.pageLeft()
        }

        /** The engine drawing the page was stopped, by the system short of
         *  memory or by a fault of its own: the screen starts again with a
         *  new one rather than taking the application down with it. */
        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            // The music it led goes with it: the page drawn again starts
            // from where the queue was kept.
            music.stopAll()
            recreate()
            return true
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            if (request.isForMainFrame) {
                showUnreachable(true)
            }
        }
    }

    private inner class InterfaceChrome : WebChromeClient() {
        override fun onShowFileChooser(
            view: WebView,
            callback: ValueCallback<Array<Uri>>,
            params: FileChooserParams,
        ): Boolean {
            picking?.onReceiveValue(null)
            picking = callback
            return try {
                picker.launch(params.createIntent())
                true
            } catch (_: ActivityNotFoundException) {
                picking = null
                false
            }
        }

        override fun onShowCustomView(view: View, callback: CustomViewCallback) {
            fullScreen?.let { onHideCustomView() }
            fullScreen = view
            (window.decorView as ViewGroup).addView(view, FrameLayout.LayoutParams(MATCH, MATCH))
            WindowCompat.getInsetsController(window, window.decorView).apply {
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
        }

        override fun onHideCustomView() {
            val shown = fullScreen ?: return
            fullScreen = null
            (window.decorView as ViewGroup).removeView(shown)
            WindowCompat.getInsetsController(window, window.decorView).show(WindowInsetsCompat.Type.systemBars())
        }
    }

    companion object {
        private const val ORIGIN = "origin"
        private const val NAME = "name"
        private const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT
        fun opening(context: Context, server: KnownServer) = Intent(context, WebActivity::class.java)
            .putExtra(ORIGIN, server.origin)
            .putExtra(NAME, server.name)
    }
}

/** In place of the interface, when its server does not answer. */
@Composable
private fun Unreachable(name: String, onRetry: () -> Unit, onChange: () -> Unit) {
    Surface(modifier = Modifier.fillMaxSize(), color = Melyxar.ground) {
        Column(
            modifier = Modifier.fillMaxSize().safeDrawingPadding().padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
        ) {
            Text(
                text = stringResource(R.string.unreachable_title),
                color = Melyxar.text,
                fontSize = 20.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = stringResource(R.string.unreachable_lead, name),
                color = Melyxar.textSoft,
                fontSize = 14.sp,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(max = 420.dp),
            )
            Button(onClick = onRetry) { Text(stringResource(R.string.unreachable_retry)) }
            TextButton(onClick = onChange) {
                Text(stringResource(R.string.unreachable_change), color = Melyxar.textSoft)
            }
        }
    }
}
