package app.melyxar.tv.trailer

import app.melyxar.server.WorkTrailer

/*
 * Which trailer a work is offered by, by the web's rule: one sitting next to
 * the film first, since it needs nobody else's server; failing that, the
 * first link.
 */

sealed interface Trailer {
    /** A file of the server, played in Melyxar. */
    data class Here(val url: String) : Trailer

    /** A link, opened in the application of the television made for it. */
    data class Away(val link: String) : Trailer
}

fun trailerOf(trailers: List<WorkTrailer>): Trailer? =
    trailers.firstNotNullOfOrNull { it.url }?.let(Trailer::Here)
        ?: trailers.firstNotNullOfOrNull { it.remoteUrl }?.let(Trailer::Away)

/** The trailer of a title of the provider, which only ever has links. */
fun trailerOfLinks(links: List<String>): Trailer? = links.firstOrNull()?.let(Trailer::Away)
