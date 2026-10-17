package app.melyxar.tv.inside

import androidx.compose.ui.unit.Dp

/*
 * How the sections of the home page are laid out, by the web's own rule
 * (screens/home.ts): the two rows of what is under way share a line when
 * they follow each other and both hold on it.
 */

/** One section as it is laid out, or the two rows of what is under way. */
sealed interface Laid {
    data class One(val section: String) : Laid

    data class Two(val first: String, val second: String) : Laid
}

private fun underWay(section: String?) = section == "carry_on" || section == "up_next"

/** The sections in the order the viewer chose, the two rows of what is under
 *  way paired when nothing stands between them. */
fun laidOut(sections: List<String>): List<Laid> {
    val laid = mutableListOf<Laid>()
    var place = 0
    while (place < sections.size) {
        val section = sections[place]
        val next = sections.getOrNull(place + 1)
        if (underWay(section) && underWay(next)) {
            laid += Laid.Two(section, next!!)
            place += 2
        } else {
            laid += Laid.One(section)
            place += 1
        }
    }
    return laid
}

/** How wide a row of [cards] cards [across] wide stands with its margin of
 *  [gutter] on both sides. */
fun rowWidth(cards: Int, across: Dp, gap: Dp, gutter: Dp): Dp = gutter * 2 + across * cards + gap * (cards - 1).coerceAtLeast(0)

/** Whether two rows of so many cards both hold on a line [screen] wide. */
fun shareALine(first: Int, second: Int, across: Dp, gap: Dp, gutter: Dp, screen: Dp): Boolean =
    first > 0 && second > 0 && rowWidth(first, across, gap, gutter) + rowWidth(second, across, gap, gutter) <= screen
