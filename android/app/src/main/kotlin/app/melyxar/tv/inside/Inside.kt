package app.melyxar.tv.inside

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.Library
import app.melyxar.server.Preferences
import app.melyxar.server.Sessions
import app.melyxar.theme.USUAL_ACCENT
import app.melyxar.theme.paletteOf
import app.melyxar.theme.parseHex
import app.melyxar.theme.wantsLight
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PageBackdrop
import app.melyxar.tv.look.TvLook
import app.melyxar.tv.look.rememberPictures
import app.melyxar.tv.music.AlbumScreen
import app.melyxar.tv.music.ArtistScreen
import app.melyxar.tv.music.GenreScreen
import app.melyxar.tv.music.Hearts
import app.melyxar.tv.music.Listening
import app.melyxar.tv.music.LocalHearts
import app.melyxar.tv.music.LocalListening
import app.melyxar.tv.music.MusicLibraryScreen
import app.melyxar.tv.music.NowPlayingScreen
import app.melyxar.tv.music.PlaylistScreen
import app.melyxar.tv.notes.LocalNotes
import app.melyxar.tv.notes.Notes
import app.melyxar.tv.notes.NotesCorner
import app.melyxar.tv.notes.NotesScreen
import app.melyxar.tv.play.PlayerScreen

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
    branding: Branding,
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
    var comebacks by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { comebacks += 1 }

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
    ) {
        val held = rememberLoaded("preferences", comebacks) { preferences() }
        val chosen = held.value.ready
        val preferences = remember(api, held) { AccountPreferences(api, scope, onSignedOut, held) }
        val libraries = rememberLoaded("libraries", comebacks) { libraries() }.value.ready
        val marks = LocalMarks.current
        // What the libraries hold moved: every screen, the libraries above
        // them included, reads again what it shows.
        val notes = LocalNotes.current
        FollowTheLiveLine(onLibrariesMoved = marks::rowsHaveMoved, onWord = notes::heard)
        val hearts = LocalHearts.current
        LaunchedEffect(api, comebacks, marks.rowsMoved) {
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
        LaunchedEffect(trail.toList()) { pinned.keepOnly(trail) }
        val here = trail.last()
        notes.filmOnScreen = here is Place.Playing
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

        val chrome = Chrome(
            branding = branding,
            account = account,
            libraries = libraries.orEmpty(),
            go = { place ->
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
                    -> trail.add(place)
                    // One search and one song playing at a time: going to
                    // either again comes back to the one already open, the
                    // search then looking where it was asked to.
                    is Place.Search, Place.NowPlaying, Place.Settings, Place.Notifications -> {
                        val open = trail.indexOfFirst { it.key == place.key }
                        if (open >= 0) {
                            backTo(open + 1)
                            trail[open] = place
                        } else {
                            trail.add(place)
                        }
                    }
                }
            },
            back = { if (trail.size > 1) backTo(trail.size - 1) },
            canGoBack = trail.size > 1,
            changeProfile = onChangeProfile,
            buttons = barButtons(chosen?.headerButtons.orEmpty(), chosen?.buttonsInTheBar ?: Preferences().buttonsInTheBar),
            here = here,
        )
        // Music started anew is followed to its page, the queue beside it.
        LaunchedEffect(listening.begun) {
            if (listening.begun > 0) chrome.go(Place.NowPlaying)
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
        TvLook(palette) {
            CompositionLocalProvider(LocalChrome provides chrome, LocalAccountPreferences provides preferences, LocalPinnedBar provides pinned) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(LocalPalette.current.ground)
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
                                !music -> return@onKeyEvent false
                                event.key == Key.MediaPlayPause -> if (down) listening.toggle()
                                event.key == Key.MediaPlay -> if (down && !listening.playing) listening.toggle()
                                event.key == Key.MediaPause -> if (down && listening.playing) listening.toggle()
                                event.key == Key.MediaNext -> if (down) listening.next()
                                event.key == Key.MediaPrevious -> if (down) listening.previous()
                                // By the account's own steps, each press and as it is held.
                                event.key == Key.MediaFastForward -> if (event.type == KeyEventType.KeyDown) listening.skip(on = true)
                                event.key == Key.MediaRewind -> if (event.type == KeyEventType.KeyDown) listening.skip(on = false)
                                else -> return@onKeyEvent false
                            }
                            true
                        },
                ) {
                    // What the account chose to draw behind its pages, once
                    // its choice is known.
                    chosen?.let { PageBackdrop(it.backdrop, it.backdropLight, Modifier.fillMaxSize()) }
                    kept.SaveableStateProvider(here.key) {
                        when (here) {
                            Place.Home -> HomeScreen(comebacks)
                            is Place.Shelf -> if (here.library.kind == "music") {
                                MusicLibraryScreen(here.library, comebacks)
                            } else {
                                LibraryScreen(here.library, comebacks)
                            }
                            is Place.Opened -> WorkScreen(here.work, comebacks)
                            is Place.Person -> PersonScreen(here.id, comebacks)
                            is Place.Search -> SearchScreen(here.library, comebacks)
                            is Place.Listed -> ShortlistScreen(here.list, comebacks)
                            is Place.Album -> AlbumScreen(here.id, comebacks)
                            is Place.Artist -> ArtistScreen(here.id, comebacks)
                            is Place.Playlist -> PlaylistScreen(here.id, comebacks)
                            is Place.Genre -> GenreScreen(here.library, here.name, comebacks)
                            Place.NowPlaying -> NowPlayingScreen()
                            Place.Settings -> SettingsScreen(sessions)
                            Place.Notifications -> NotesScreen()
                            // The film is always shown dark, as the web's player is.
                            is Place.Playing -> key(here.play) {
                                TvLook(paletteOf(chosenAccent, light = false)) {
                                    PlayerScreen(
                                        here.play,
                                        onDone = { backTo(trail.size - 1) },
                                        onNext = { next -> trail[trail.lastIndex] = Place.Playing(next) },
                                    )
                                }
                            }
                        }
                    }
                    PinnedTopBar(Modifier.align(Alignment.TopCenter))
                    // Over every screen, the film's included, which only
                    // what is urgent is said over.
                    NotesCorner(Modifier.align(Alignment.TopEnd))
                }
            }
        }
    }
}
