package app.melyxar.tv.inside

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.melyxar.R
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.Camera
import app.melyxar.server.Library
import app.melyxar.server.Preferences
import app.melyxar.server.Sessions
import app.melyxar.theme.USUAL_ACCENT
import app.melyxar.theme.paletteOf
import app.melyxar.theme.parseHex
import app.melyxar.theme.wantsLight
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.Lighter
import app.melyxar.tv.look.PageBackdrop
import app.melyxar.tv.look.TvLook
import app.melyxar.tv.look.WindowGround
import app.melyxar.tv.look.rememberPictures
import app.melyxar.tv.music.AlbumScreen
import app.melyxar.tv.music.ArtistScreen
import app.melyxar.tv.music.FollowSongLevels
import app.melyxar.tv.music.GenreScreen
import app.melyxar.tv.music.Hearts
import app.melyxar.tv.music.Listening
import app.melyxar.tv.music.LocalHearts
import app.melyxar.tv.music.LocalListening
import app.melyxar.tv.music.LocalSongLevels
import app.melyxar.tv.music.MusicLibraryScreen
import app.melyxar.tv.music.NowPlayingScreen
import app.melyxar.tv.music.PlaylistScreen
import app.melyxar.tv.music.SongLevels
import app.melyxar.tv.notes.LocalNotes
import app.melyxar.tv.notes.Notes
import app.melyxar.tv.notes.NotesCorner
import app.melyxar.tv.notes.NotesScreen
import app.melyxar.tv.play.KeepingSheets
import app.melyxar.tv.play.PlayerScreen
import app.melyxar.tv.probe.Part
import app.melyxar.tv.probe.Probe
import app.melyxar.tv.probe.probed
import app.melyxar.tv.requests.LocalRequests
import app.melyxar.tv.requests.MoreTitlesScreen
import app.melyxar.tv.requests.RequestedTitleScreen
import app.melyxar.tv.requests.Requests
import app.melyxar.tv.requests.RequestsScreen
import app.melyxar.tv.live.CamerasScreen
import app.melyxar.tv.live.LiveScreen
import app.melyxar.tv.trailer.TrailerScreen
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** A screen of the inside, as the trail of screens holds it. */
sealed interface Place {
    /** Its name in the trail, under which what it scrolled to and where the
     *  remote was are kept while another screen is over it. */
    val key: String

    data object Home : Place {
        override val key = "home"
    }

    data class Shelf(val library: Library) : Place {
        override val key = "library:${library.id}"
    }

    data class Opened(val work: String) : Place {
        override val key = "work:$work"
    }

    data class Person(val id: String) : Place {
        override val key = "person:$id"
    }

    /** The search, in one [library] when it was asked from that library's
     *  own pages, everywhere otherwise. */
    data class Search(val library: Library? = null) : Place {
        override val key = "search"
    }

    data class Listed(val list: Shortlist) : Place {
        override val key = "list:$list"
    }

    data class Playing(val play: Play) : Place {
        override val key = "playing"
    }

    data class Album(val id: String) : Place {
        override val key = "album:$id"
    }

    data class Artist(val id: String) : Place {
        override val key = "artist:$id"
    }

    data class Playlist(val id: String) : Place {
        override val key = "playlist:$id"
    }

    data class Genre(val library: Library, val name: String) : Place {
        override val key = "genre:${library.id}:$name"
    }

    /** Asking for titles the server has not, and this account's requests. */
    data object TitleRequests : Place {
        override val key = "requests"
    }

    /** The page of a title of the provider, which may not be here. */
    data class RequestedTitle(val catalogue: String, val tmdbId: String) : Place {
        override val key = "request-title:$catalogue:$tmdbId"
    }

    /** Many more titles of a shelf of suggestions: the most popular of a
     *  catalogue, in a [genre] when one is named. */
    data class MoreTitles(val catalogue: String, val genreId: String?, val genre: String?) : Place {
        override val key = "request-more:$catalogue:$genreId"
    }

    /** A trailer sitting next to the work called [title], fetched from [url]. */
    /** A camera watched live. */
    data class Live(val camera: Camera) : Place {
        override val key = "live:${camera.id}"
    }

    data class Trailer(val title: String, val url: String) : Place {
        override val key = "trailer"
    }

    /** The account's notifications. */
    data object Notifications : Place {
        override val key = "notifications"
    }

    /** What the account chooses for itself, and this television. */
    data object Settings : Place {
        override val key = "settings"
    }

    /** The song playing and what comes after it. */
    data object NowPlaying : Place {
        override val key = "now-playing"
    }
}

/** A work to play, from this file when it says, from this second when it
 *  says, or where the server has it stopped. */
data class Play(val work: String, val source: String?, val from: Double?)

/** The library whose pages these are, which a search asked from them looks
 *  in, as the web's does. */
private val Place.library: Library?
    get() = when (this) {
        is Place.Shelf -> library
        is Place.Genre -> library
        else -> null
    }

/** A key of the remote that does one thing pressed and another held: held,
 *  letting it go does nothing more. */
private class Holding(private val scope: CoroutineScope) {
    private var held = false
    private var waiting: Job? = null

    /** Gone down: [onHeld] once it has been held long enough. */
    fun down(onHeld: () -> Unit) {
        held = false
        waiting?.cancel()
        waiting = scope.launch {
            delay(HELD_AFTER_MS)
            held = true
            onHeld()
        }
    }

    /** Let go: [onPressed], unless it was held. */
    fun up(onPressed: () -> Unit) {
        waiting?.cancel()
        if (!held) onPressed()
    }
}

/** How long a key is held before it counts as held: sooner than the
 *  remote's own first repeat, which comes after half a second. */
private const val HELD_AFTER_MS = 300L

/** What every screen's top bar shows, and where its pieces lead. */
class Chrome(
    val branding: Branding,
    val account: Account,
    val libraries: List<Library>,
    val go: (Place) -> Unit,
    /** Takes the screen shown off, back to the one under it. */
    val back: () -> Unit,
    /** Whether the back key leads to another screen, which it does
     *  anywhere but home. */
    val canGoBack: Boolean,
    val changeProfile: () -> Unit,
    /** What the account chose to have on the bar and in its menu. */
    val buttons: BarButtons,
    /** The screen shown. */
    val here: Place,
) {
    fun play(play: Play) = go(Place.Playing(play))

    /** The search, in the library whose pages these are, if they are. */
    fun search() = go(Place.Search(here.library))
}

val LocalChrome = staticCompositionLocalOf<Chrome> { error("no chrome") }

/**
 * The television once somebody is in: the account's colours on everything,
 * and the screens one over the other, the back key taking off the last one.
 */
@Composable
fun Inside(
    api: Api,
    account: Account,
    /** What the server wore when the account came in, until it is read again. */
    signedInWith: Branding,
    /** A work to open, asked from the system's Continue watching row. */
    opening: String?,
    onOpened: () -> Unit,
    /** What this television keeps of its own, which its settings change. */
    sessions: Sessions,
    onChangeProfile: () -> Unit,
    onSignedOut: () -> Unit,
    onLeave: () -> Unit,
) {
    // Read again each time the television comes back to the application:
    // a colour or a theme changed on another device shows without a restart.
    // Only after it was left: the resume the lifecycle replays to a screen
    // composed while the application is already on would ask everything
    // twice as the television comes in.
    var comebacks by remember { mutableIntStateOf(0) }
    var away by remember { mutableStateOf(false) }
    LifecycleEventEffect(Lifecycle.Event.ON_PAUSE) { away = true }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        if (away) {
            away = false
            comebacks += 1
        }
    }
    // And each time the live line says how Melyxar looks moved.
    var looksMoved by remember { mutableIntStateOf(0) }

    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val listening = remember(api) { Listening(context, api, scope) }
    // The music is this account's: it stops when somebody else comes in, or
    // when the television leaves the application.
    DisposableEffect(listening) { onDispose { listening.leave() } }
    CompositionLocalProvider(
        LocalServer provides api,
        LocalPictures provides rememberPictures(api),
        LocalSignedOut provides onSignedOut,
        LocalAnswers provides remember(api) { HashMap() },
        LocalMarks provides remember(api) { Marks(api, scope, onSignedOut) },
        LocalListening provides listening,
        LocalHearts provides remember(api) { Hearts(api, scope, onSignedOut) },
        LocalNotes provides remember(api) { Notes(api, scope, onSignedOut) },
        LocalRequests provides remember(api) { Requests(api, scope, onSignedOut) },
        LocalSongLevels provides remember(api) { SongLevels() },
    ) {
        FollowSongLevels(LocalSongLevels.current)
        // The preferences carry the languages the libraries hold; the
        // server's look has its own word on the live line.
        val held = rememberLoaded("preferences", comebacks, looksMoved) { preferences() }
        val branding = rememberLoaded("branding", comebacks, looksMoved, followsRows = false) { branding() }.value.ready ?: signedInWith
        val chosen = held.value.ready
        val preferences = remember(api, held) { AccountPreferences(api, scope, onSignedOut, held) }
        val libraries = rememberLoaded("libraries", comebacks) { libraries() }.value.ready
        val marks = LocalMarks.current
        // What the libraries hold moved: every screen, the libraries above
        // them included, reads again what it shows.
        val notes = LocalNotes.current
        val requests = LocalRequests.current
        val maintenance = remember(api) { Maintenance(api, scope) }
        FollowTheLiveLine(
            onLibrariesMoved = marks::rowsHaveMoved,
            onLooksMoved = { looksMoved += 1 },
            onWord = { word ->
                notes.heard(word)
                requests.heard(word)
                maintenance.heard(word)
            },
        )
        LaunchedEffect(api, comebacks) { maintenance.ask() }
        // A maintenance in force stands in for every screen of an account
        // that is not an administrator's.
        val turnedAway = maintenance.standing.state == "on" && !account.isAdministrator
        FollowMaintenance(maintenance, turnedAway)
        LaunchedEffect(api, comebacks) { requests.read() }
        val hearts = LocalHearts.current
        LaunchedEffect(api, comebacks, marks.rowsMoved, looksMoved) {
            hearts.read()
            listening.readPreferences()
        }
        val deviceIsDark = isSystemInDarkTheme()
        val chosenAccent = chosen?.accentColor?.let(::parseHex) ?: USUAL_ACCENT
        val palette = remember(chosen, branding.defaultTheme, deviceIsDark) {
            paletteOf(
                accent = chosenAccent,
                light = wantsLight(chosen?.themeMode ?: "server", branding.defaultTheme, deviceIsDark),
            )
        }
        val trail = remember { mutableStateListOf<Place>(Place.Home) }
        val pinned = remember { PinnedBar() }
        val skipped = remember { Holding(scope) }
        val more = remember { Holding(scope) }
        val cardMenus = remember { CardMenus() }
        val screen = remember { FocusRequester() }
        LaunchedEffect(trail.toList()) { pinned.keepOnly(trail) }
        val here = trail.last()
        val film = here is Place.Playing || here is Place.Trailer || here is Place.Live
        notes.filmOnScreen = film
        val kept = rememberSaveableStateHolder()

        /** Takes the screens above [depth] off the trail, forgetting them. */
        fun backTo(depth: Int) {
            while (trail.size > depth) {
                val gone = trail.removeAt(trail.lastIndex)
                if (trail.none { it.key == gone.key }) {
                    kept.removeState(gone.key)
                }
            }
        }

        val go: (Place) -> Unit = remember {
            { place ->
                when (place) {
                    // Home and the libraries lead from home, not from
                    // wherever the trail had got to.
                    Place.Home -> backTo(1)
                    is Place.Shelf -> {
                        backTo(1)
                        trail.add(place)
                    }
                    is Place.Opened, is Place.Person, is Place.Playing, is Place.Listed,
                    is Place.Album, is Place.Artist, is Place.Playlist, is Place.Genre,
                    is Place.RequestedTitle, is Place.MoreTitles, is Place.Trailer, is Place.Live,
                    -> trail.add(place)
                    // One search and one song playing at a time: going to
                    // either again comes back to the one already open, the
                    // search then looking where it was asked to.
                    is Place.Search, Place.NowPlaying, Place.Settings, Place.Notifications, Place.TitleRequests -> {
                        val open = trail.indexOfFirst { it.key == place.key }
                        if (open >= 0) {
                            backTo(open + 1)
                            trail[open] = place
                        } else {
                            trail.add(place)
                        }
                    }
                }
            }
        }
        val back: () -> Unit = remember { { if (trail.size > 1) backTo(trail.size - 1) } }
        // Turned away, the film and the music stop and the screens are put
        // down; once it ends, the television is back at home and every
        // screen reads again what it shows.
        var shutOut by remember { mutableStateOf(false) }
        LaunchedEffect(turnedAway) {
            if (turnedAway) {
                backTo(1)
                listening.stop()
                shutOut = true
            } else if (shutOut) {
                shutOut = false
                comebacks += 1
            }
        }
        val buttons = barButtons(
            chosen?.headerButtons.orEmpty(),
            chosen?.buttonsInTheBar ?: Preferences().buttonsInTheBar,
            // Requests only while they are on and this account may make them.
            if (requests.access?.mayAsk == true) ON_A_TELEVISION + "requests" else ON_A_TELEVISION,
        )
        val canGoBack = trail.size > 1
        // Every screen reads it: a new one only when what it says changed,
        // or all of them are composed again.
        val chrome = remember(branding, account, libraries, go, back, canGoBack, onChangeProfile, buttons, here) {
            Chrome(
                branding = branding,
                account = account,
                libraries = libraries.orEmpty(),
                go = go,
                back = back,
                canGoBack = canGoBack,
                changeProfile = onChangeProfile,
                buttons = buttons,
                here = here,
            )
        }
        // Its page over home, as if opened from Melyxar's own row.
        LaunchedEffect(opening) {
            opening ?: return@LaunchedEffect
            chrome.go(Place.Home)
            chrome.go(Place.Opened(opening))
            onOpened()
        }
        // Before the screens, so what a screen opens is closed by the key first.
        BackHandler {
            if (trail.size > 1) {
                backTo(trail.size - 1)
            } else {
                onLeave()
            }
        }
        WindowGround(palette.ground)
        TvLook(palette) {
            CompositionLocalProvider(LocalChrome provides chrome, LocalAccountPreferences provides preferences, LocalPinnedBar provides pinned, LocalCardMenus provides cardMenus) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .onPreviewKeyEvent { event ->
                            if (event.type == KeyEventType.KeyDown) focusSaid { "key ${event.key}" }
                            false
                        }
                        // The search key of a remote that has one, and its keys
                        // of music while a song is queued. A film's own
                        // player answers them first. A key of music is acted
                        // on as it goes down and taken whole: let through, its
                        // going down reaches the music's session too, which
                        // acted on it a second time.
                        .onKeyEvent { event ->
                            // Once, however long the key is held.
                            val down = event.type == KeyEventType.KeyDown && event.nativeKeyEvent.repeatCount == 0
                            val music = listening.queue.current != null
                            when {
                                event.key == Key.Search -> if (!down) chrome.search()
                                // The key of more options pressed puts the remote on
                                // the bar: on the song's cover while one plays, on its
                                // first button otherwise, the screen brought back to
                                // its top where its own bar is. Held, it opens the
                                // menu of the card the remote is on.
                                event.key == Key.Menu -> {
                                    fun toTheBar() {
                                        scope.launch {
                                            if (listening.queue.current == null) {
                                                pinned.toTop[here]?.invoke()
                                                withFrameNanos {}
                                            }
                                            try {
                                                pinned.entry.requestFocus()
                                            } catch (_: IllegalStateException) {
                                                // A screen without a bar, a film's or a photo's.
                                            }
                                        }
                                    }
                                    when {
                                        down -> more.down { if (!cardMenus.openFocused()) toTheBar() }
                                        event.type == KeyEventType.KeyUp -> more.up(::toTheBar)
                                    }
                                }
                                !music -> return@onKeyEvent false
                                event.key == Key.MediaPlayPause -> if (down) listening.toggle()
                                event.key == Key.MediaPlay -> if (down && !listening.playing) listening.toggle()
                                event.key == Key.MediaPause -> if (down && listening.playing) listening.toggle()
                                event.key == Key.MediaNext -> if (down) listening.next()
                                event.key == Key.MediaPrevious -> if (down) listening.previous()
                                // Pressed, a step by the account's own; held, the
                                // song after or before.
                                event.key == Key.MediaFastForward || event.key == Key.MediaRewind -> {
                                    val on = event.key == Key.MediaFastForward
                                    when {
                                        down -> skipped.down { if (on) listening.next() else listening.previous() }
                                        event.type == KeyEventType.KeyUp -> skipped.up { listening.skip(on) }
                                    }
                                }
                                else -> return@onKeyEvent false
                            }
                            true
                        },
                ) {
                    // What the account chose to draw behind its pages, once
                    // its choice is known; kept ready, but not drawn, under a
                    // film or while the television draws lighter.
                    chosen?.let {
                        PageBackdrop(it.backdrop, it.backdropLight, Modifier.fillMaxSize().drawWithContent { if (!film && !Lighter.on) drawContent() })
                    }
                    LaunchedEffect(here) {
                        focusSaid { "screen shown: ${here.key}" }
                        Probe.screenShown(here.key)
                    }
                    if (turnedAway) {
                        MaintenanceScreen(branding, maintenance.standing, stringResource(R.string.tv_change_profile), onChangeProfile)
                    } else {
                        // The screen as one piece the remote comes back into where it
                        // left it, which the bar pinned over it leads down to.
                        Box(Modifier.fillMaxSize().focusRequester(screen).focusRestorer()) {
                            kept.SaveableStateProvider(here.key) {
                                when (here) {
                                    Place.Home -> HomeScreen(comebacks)
                                    is Place.Shelf -> when (here.library.kind) {
                                        "music" -> MusicLibraryScreen(here.library, comebacks)
                                        "cameras" -> CamerasScreen(here.library, comebacks)
                                        else -> LibraryScreen(here.library, comebacks)
                                    }
                                    is Place.Opened -> KeepingSheets { WorkScreen(here.work, comebacks) }
                                    is Place.Person -> PersonScreen(here.id, comebacks)
                                    is Place.Search -> SearchScreen(here.library, comebacks)
                                    is Place.Listed -> ShortlistScreen(here.list, comebacks)
                                    is Place.Album -> AlbumScreen(here.id, comebacks)
                                    is Place.Artist -> ArtistScreen(here.id, comebacks)
                                    is Place.Playlist -> PlaylistScreen(here.id, comebacks)
                                    is Place.Genre -> GenreScreen(here.library, here.name, comebacks)
                                    Place.TitleRequests -> RequestsScreen()
                                    is Place.RequestedTitle -> RequestedTitleScreen(here.catalogue, here.tmdbId)
                                    is Place.MoreTitles -> MoreTitlesScreen(here.catalogue, here.genreId, here.genre)
                                    is Place.Trailer -> TrailerScreen(here.title, here.url)
                                    is Place.Live -> LiveScreen(here.camera)
                                    Place.NowPlaying -> NowPlayingScreen()
                                    Place.Settings -> SettingsScreen(sessions)
                                    Place.Notifications -> NotesScreen()
                                    // The film is always shown dark, as the web's player is.
                                    is Place.Playing -> key(here.play) {
                                        TvLook(paletteOf(chosenAccent, light = false)) {
                                            KeepingSheets {
                                                PlayerScreen(
                                                    here.play,
                                                    onDone = { backTo(trail.size - 1) },
                                                    onNext = { next -> trail[trail.lastIndex] = Place.Playing(next) },
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        PinnedTopBar(screen, Modifier.align(Alignment.TopCenter).probed(Part.Bar))
                        cardMenus.open?.let { card -> CardMenu(card, onClose = { cardMenus.open = null }) }
                    }
                    // Over every screen, the film's included, which only
                    // what is urgent is said over.
                    NotesCorner(Modifier.align(Alignment.TopEnd))
                    MaintenanceBand(maintenance.standing, account.isAdministrator, Modifier.align(Alignment.TopCenter))
                }
            }
        }
    }
}
