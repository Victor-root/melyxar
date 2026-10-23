package app.melyxar.theme

import androidx.annotation.DrawableRes
import app.melyxar.R

/*
 * Which picture an account without one of its own wears: the web's sum
 * (`faces.ts`), so an account wears the same face on the television as on
 * the page.
 */

/** The pictures, in the web's order, which the name's hash picks from. */
enum class FacePicture(@DrawableRes val drawing: Int) {
    Comet(R.drawable.face_comet),
    Rings(R.drawable.face_rings),
    Orbits(R.drawable.face_orbits),
    Crystal(R.drawable.face_crystal),
    Eclipse(R.drawable.face_eclipse),
}

/** FNV-1a over the name's code points, as `for...of` walks them on the web. */
private fun hashOf(name: String): UInt {
    var hash = 0x811c9dc5u
    name.codePoints().forEach { point ->
        hash = (hash xor point.toUInt()) * 0x01000193u
    }
    return hash
}

fun facePictureOf(name: String): FacePicture {
    val pictures = FacePicture.entries
    return pictures[(hashOf(name) % pictures.size.toUInt()).toInt()]
}
