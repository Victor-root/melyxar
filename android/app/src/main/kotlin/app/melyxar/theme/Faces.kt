package app.melyxar.theme

import androidx.annotation.DrawableRes
import app.melyxar.R

/*
 * Which picture an account without one of its own wears: the one the server
 * gave it, named as the server names it. A server older than that gives none,
 * and the name decides, as it did on every screen then.
 */

/** The pictures, in the order the name's hash picks from. */
enum class FacePicture(@DrawableRes val drawing: Int) {
    Comet(R.drawable.face_comet),
    Rings(R.drawable.face_rings),
    Orbits(R.drawable.face_orbits),
    Crystal(R.drawable.face_crystal),
    Eclipse(R.drawable.face_eclipse),
    Aurora(R.drawable.face_aurora),
    Crescent(R.drawable.face_crescent),
    Spiral(R.drawable.face_spiral),
    Binary(R.drawable.face_binary),
    Verdant(R.drawable.face_verdant),
}

/** FNV-1a over the name's code points, as `for...of` walks them on the web. */
private fun hashOf(name: String): UInt {
    var hash = 0x811c9dc5u
    name.codePoints().forEach { point ->
        hash = (hash xor point.toUInt()) * 0x01000193u
    }
    return hash
}

fun facePictureOf(name: String, given: String?): FacePicture {
    val pictures = FacePicture.entries
    return pictures.firstOrNull { it.name.lowercase() == given }
        ?: pictures[(hashOf(name) % pictures.size.toUInt()).toInt()]
}
