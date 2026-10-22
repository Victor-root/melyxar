package app.melyxar.tv.inside

import androidx.compose.ui.unit.DpSize
import app.melyxar.tv.look.PosterWidth

/** How wide a smaller poster is, as the web draws them. */
private val CompactWidth = PosterWidth * 0.72f

/** How much of a card's frame the picture of a line of a list keeps. */
private const val LINE_SHARE = 0.6f

/**
 * How the grid of a library is laid out, as the account chose it for that
 * library: the web's posters, smaller posters, stills lying down, or a list.
 */
enum class LibraryLayout(val word: String) {
    Posters("posters"),
    SmallPosters("compact_posters"),
    Thumbnails("thumbnails"),
    Lines("list");

    /** Whether its pictures lie, for a library whose cards lie by nature
     *  when [lying]. */
    fun lies(lying: Boolean): Boolean = when (this) {
        Posters, Lines -> lying
        SmallPosters -> false
        Thumbnails -> true
    }

    /** The frame of a picture laid out this way. */
    fun frame(lying: Boolean): DpSize = when (this) {
        SmallPosters -> DpSize(CompactWidth, CompactWidth * 3 / 2)
        Lines -> frameOf(lies(lying)) * LINE_SHARE
        Posters, Thumbnails -> frameOf(lies(lying))
    }

    companion object {
        /** The layout a word names, the posters for any other. */
        fun of(word: String?): LibraryLayout = entries.firstOrNull { it.word == word } ?: Posters
    }
}
