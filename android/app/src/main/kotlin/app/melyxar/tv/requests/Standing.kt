package app.melyxar.tv.requests

import app.melyxar.server.Found
import app.melyxar.server.TitleRequest

/*
 * Where a title stands for this account, by the web's own rules: read from
 * what the server answered of it and from this account's requests as held
 * now, so a request made or taken back shows at once.
 */

sealed interface Standing {
    /** Here, whole, in a library this account sees. */
    data class Here(val workId: String) : Standing

    /** This account asked for it and waits. */
    data class Mine(val request: TitleRequest, val others: Int) : Standing

    /** Free to ask for, by others already or not; a series held in part
     *  names the seasons it holds. */
    data class Free(val others: Int, val heldSeasons: List<Int>) : Standing
}

/** This account's request for a title, while it waits. */
fun openRequestFor(mine: List<TitleRequest>?, catalogue: String, tmdbId: String): TitleRequest? =
    mine?.firstOrNull { it.catalogue == catalogue && it.tmdbId == tmdbId && (it.state == "pending" || it.state == "accepted") }

fun standingOf(found: Found, mine: List<TitleRequest>?): Standing {
    val others = found.askedBy - if (found.mine != null) 1 else 0
    openRequestFor(mine, found.catalogue, found.tmdbId)?.let { return Standing.Mine(it, others) }
    val held = found.held
    if (held != null && found.catalogue == "films") {
        return Standing.Here(held.workId)
    }
    return Standing.Free(others, held?.seasons.orEmpty())
}
