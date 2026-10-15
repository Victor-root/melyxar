package app.melyxar.server

/**
 * What a grid of works asks the server for, as the web's `works()` does: a
 * library or a kind, an order, and what narrows it. Every grid of the
 * interface is one of these: a library, a search, the favourites, the list
 * to watch later.
 */
data class Browse(
    val library: String? = null,
    val kind: String? = null,
    /** title, added_at, release_year, community_rating or runtime. */
    val order: String = "title",
    val descending: Boolean = false,
    val genre: String? = null,
    /** A decade by its first year, 1990 for the nineties. */
    val decade: Int? = null,
    /** A letter, or "#" for what starts with anything else. */
    val initial: String? = null,
    val search: String? = null,
    val favourites: Boolean = false,
    val watchLater: Boolean = false,
)
