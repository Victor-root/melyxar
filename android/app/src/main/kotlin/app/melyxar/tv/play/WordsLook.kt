package app.melyxar.tv.play

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.edit
import app.melyxar.R

/**
 * How the subtitles look, the web player's own choices (see the web's
 * `appearance.ts` and `subtitles.css`), each a short list rather than a free
 * value: a handful of choices that all stay readable.
 */
enum class WordsSize(val key: String, /** As a share of the screen's height. */ val ofHeight: Float) {
    Small("small", 0.032f),
    Normal("normal", 0.04f),
    Large("large", 0.054f),
    Huge("huge", 0.07f),
}

enum class WordsColour(val key: String, val argb: Long) {
    White("white", 0xFFFFFFFF),
    Yellow("yellow", 0xFFF5E050),
    Cyan("cyan", 0xFF7FE8F0),
}

/** What keeps the words legible against the picture behind them. */
enum class WordsEdge(val key: String) {
    Outline("outline"),
    Shadow("shadow"),
    None("none"),
}

enum class WordsBackground(val key: String, val argb: Long?) {
    None("none", null),
    Dim("dim", 0x8C000000),
    Solid("solid", 0xFF000000),
}

/** How far up the screen the words sit, as a share of its height counted
 *  from the bottom; the last is the one the viewer sets by hand. */
enum class WordsHeight(val key: String, val share: Float?) {
    Bottom("bottom", 0.08f),
    Raised("raised", 0.5f),
    High("high", 0.85f),
    Custom("custom", null),
}

/** The custom heights offered, in hundredths of the screen. */
val CUSTOM_HEIGHTS = (0..100 step 5).toList()

data class WordsLook(
    val size: WordsSize = WordsSize.Normal,
    val colour: WordsColour = WordsColour.White,
    val edge: WordsEdge = WordsEdge.Outline,
    val background: WordsBackground = WordsBackground.None,
    val height: WordsHeight = WordsHeight.Bottom,
    /** Where the words sit when [height] is custom, in hundredths. Kept
     *  while another height is chosen, so going back to it finds it again. */
    val heightCustom: Int = 50,
) {
    /** Where the words sit, as a share of the screen's height. */
    val share: Float get() = height.share ?: (heightCustom.coerceIn(0, 100) / 100f)
}

/**
 * Where the top of the words stands, in pixels from the top of a [screen]
 * that tall, for words [block] tall sitting at [share] of it.
 *
 * As the web places them: their foot at [share] of the screen from its
 * bottom, then let down by that same share of their own height, which keeps
 * the whole block on the screen at either end. While the controls cover the
 * foot of the film, [lift] says how far up their foot has to be, and the
 * higher of the two wins.
 */
fun wordsTop(screen: Float, block: Float, share: Float, lift: Float?): Float {
    val chosen = share * screen
    val foot = if (lift == null) chosen else maxOf(chosen, lift)
    return screen - foot + share * block - block
}

/** What each choice is called, as the web's player calls it. */
val WordsSize.named: Int get() = when (this) {
    WordsSize.Small -> R.string.tv_subtitle_size_small
    WordsSize.Normal -> R.string.tv_subtitle_size_normal
    WordsSize.Large -> R.string.tv_subtitle_size_large
    WordsSize.Huge -> R.string.tv_subtitle_size_huge
}

val WordsColour.named: Int get() = when (this) {
    WordsColour.White -> R.string.tv_subtitle_colour_white
    WordsColour.Yellow -> R.string.tv_subtitle_colour_yellow
    WordsColour.Cyan -> R.string.tv_subtitle_colour_cyan
}

val WordsEdge.named: Int get() = when (this) {
    WordsEdge.Outline -> R.string.tv_subtitle_edge_outline
    WordsEdge.Shadow -> R.string.tv_subtitle_edge_shadow
    WordsEdge.None -> R.string.tv_subtitle_edge_none
}

val WordsBackground.named: Int get() = when (this) {
    WordsBackground.None -> R.string.tv_subtitle_background_none
    WordsBackground.Dim -> R.string.tv_subtitle_background_dim
    WordsBackground.Solid -> R.string.tv_subtitle_background_solid
}

val WordsHeight.named: Int get() = when (this) {
    WordsHeight.Bottom -> R.string.tv_subtitle_height_bottom
    WordsHeight.Raised -> R.string.tv_subtitle_height_raised
    WordsHeight.High -> R.string.tv_subtitle_height_high
    WordsHeight.Custom -> R.string.tv_subtitle_height_custom
}

/** The look chosen on this television, kept on it alone, as a browser keeps
 *  its own: it is this screen and this distance that call for it. */
object KeptWordsLook {
    private const val KEPT = "subtitles"

    private var kept: SharedPreferences? = null

    /** Read where the words are drawn, so a choice shows at once. */
    var look by mutableStateOf(WordsLook())
        private set

    fun keep(context: Context) {
        kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE).also { look = read(it) }
    }

    fun choose(chosen: WordsLook) {
        look = chosen
        kept?.edit {
            putString("size", chosen.size.key)
            putString("colour", chosen.colour.key)
            putString("edge", chosen.edge.key)
            putString("background", chosen.background.key)
            putString("height", chosen.height.key)
            putInt("height_custom", chosen.heightCustom)
        }
    }

    /** What was kept, anything that is not a choice read as the default. */
    private fun read(from: SharedPreferences): WordsLook {
        val usual = WordsLook()
        return WordsLook(
            size = WordsSize.entries.firstOrNull { it.key == from.getString("size", null) } ?: usual.size,
            colour = WordsColour.entries.firstOrNull { it.key == from.getString("colour", null) } ?: usual.colour,
            edge = WordsEdge.entries.firstOrNull { it.key == from.getString("edge", null) } ?: usual.edge,
            background = WordsBackground.entries.firstOrNull { it.key == from.getString("background", null) } ?: usual.background,
            height = WordsHeight.entries.firstOrNull { it.key == from.getString("height", null) } ?: usual.height,
            heightCustom = from.getInt("height_custom", usual.heightCustom).coerceIn(0, 100),
        )
    }
}
