package app.melyxar.tv.music

import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.music.Listening

/** The player of music of the account on the television, for its screens. */
val LocalListening = staticCompositionLocalOf<Listening> { error("no music") }
