package app.melyxar.tv.inside

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.Library
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

    data class Playing(val play: Play) : Place {
        override val key = "playing"
    }
}

/** A work to play, from this file when it says, from this second when it
 *  says, or where the server has it stopped. */
data class Play(val work: String, val source: String?, val from: Double?)

/** What every screen's top bar shows, and where its pieces lead. */
class Chrome(
    val branding: Branding,
    val account: Account,
    val libraries: List<Library>,
    val go: (Place) -> Unit,
    /** The way back one screen, or nothing on the home screen. */
    val back: (() -> Unit)?,
    val changeProfile: () -> Unit,
) {
    fun play(play: Play) = go(Place.Playing(play))
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
    onChangeProfile: () -> Unit,
    onSignedOut: () -> Unit,
    onLeave: () -> Unit,
) {
    // Read again each time the television comes back to the application:
    // a colour or a theme changed on another device shows without a restart.
    var comebacks by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { comebacks += 1 }

    CompositionLocalProvider(
        LocalServer provides api,
        LocalPictures provides rememberPictures(api),
        LocalSignedOut provides onSignedOut,
        LocalAnswers provides remember(api) { HashMap() },
    ) {
        val chosen = rememberLoaded("preferences", comebacks) { preferences() }.value.ready
        val libraries = rememberLoaded("libraries", comebacks) { libraries() }.value.ready
        val deviceIsDark = isSystemInDarkTheme()
        val chosenAccent = chosen?.accentColor?.let(::parseHex) ?: USUAL_ACCENT
        val palette = remember(chosen, branding.defaultTheme, deviceIsDark) {
            paletteOf(
                accent = chosenAccent,
                light = wantsLight(chosen?.themeMode ?: "server", branding.defaultTheme, deviceIsDark),
            )
        }
        val trail = remember { mutableStateListOf<Place>(Place.Home) }
        val here = trail.last()
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
                    is Place.Opened, is Place.Playing -> trail.add(place)
                }
            },
            back = if (trail.size > 1) {
                { backTo(trail.size - 1) }
            } else {
                null
            },
            changeProfile = onChangeProfile,
        )
        // Before the screens, so what a screen opens is closed by the key first.
        BackHandler {
            if (trail.size > 1) {
                backTo(trail.size - 1)
            } else {
                onLeave()
            }
        }
        TvLook(palette) {
            CompositionLocalProvider(LocalChrome provides chrome) {
                Box(Modifier.fillMaxSize().background(LocalPalette.current.ground)) {
                    // What the account chose to draw behind its pages, once
                    // its choice is known.
                    chosen?.let { PageBackdrop(it.backdrop, it.backdropLight, Modifier.fillMaxSize()) }
                    kept.SaveableStateProvider(here.key) {
                        when (here) {
                            Place.Home -> HomeScreen(comebacks)
                            is Place.Shelf -> LibraryScreen(here.library, comebacks)
                            is Place.Opened -> WorkScreen(here.work, comebacks)
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
                }
            }
        }
    }
}
