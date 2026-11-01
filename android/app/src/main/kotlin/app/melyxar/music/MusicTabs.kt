package app.melyxar.music

import androidx.annotation.StringRes
import app.melyxar.R

/** The tabs of a library of music, in the web's order (`music/tabs.ts`). */
val MUSIC_TABS = listOf("for_you", "albums", "album_artists", "artists", "songs", "playlists", "favourites", "genres")

/** The tabs an account shows, in their order. Never none, as the web's: a
 *  choice that hides every one hides none. */
fun shownTabs(hidden: List<String>): List<String> = MUSIC_TABS.filter { it !in hidden }.ifEmpty { MUSIC_TABS }

/** What a tab is called. */
@StringRes
fun musicTabName(tab: String): Int = when (tab) {
    "for_you" -> R.string.tv_music_for_you
    "album_artists" -> R.string.tv_music_album_artists
    "artists" -> R.string.tv_music_artists
    "songs" -> R.string.tv_music_songs
    "playlists" -> R.string.tv_music_playlists
    "favourites" -> R.string.tv_music_favourites
    "genres" -> R.string.tv_music_genres
    else -> R.string.tv_music_albums
}
