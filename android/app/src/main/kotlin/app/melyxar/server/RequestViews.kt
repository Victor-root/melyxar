package app.melyxar.server

import kotlinx.serialization.Serializable

/*
 * What the server says of title requests, as it writes it: kept apart from
 * the rest, as the requests are everywhere else.
 */

/** Whether requests are on, and whether this account may make them. */
@Serializable
data class RequestAccess(val enabled: Boolean = false, val mayAsk: Boolean = false)

/** A title found here already: the work to open, and for a series the
 *  seasons it holds. */
@Serializable
data class TitleHeld(val workId: String, val seasons: List<Int> = emptyList())

/** A title of the provider, and where it stands here. */
@Serializable
data class Found(
    /** films or series. */
    val catalogue: String,
    val tmdbId: String,
    val title: String,
    val year: Int? = null,
    val overview: String? = null,
    /** Its whole address, at the provider. */
    val poster: String? = null,
    val held: TitleHeld? = null,
    /** How many accounts asked for it and wait. */
    val askedBy: Int = 0,
    /** This account's own request for it, while it waits. */
    val mine: String? = null,
)

/** A row of titles to suggest: from a genre the account watches, or the
 *  most popular of all. */
@Serializable
data class TitleShelf(
    val genre: String? = null,
    /** That genre at the provider, which seeing more asks the rest of. */
    val genreId: String? = null,
    val catalogue: String,
    val items: List<Found> = emptyList(),
)

/** A season of a series, as it may be asked for. */
@Serializable
data class SeasonChoice(val number: Int, val episodes: Int = 0, val held: Boolean = false)

/** Somebody of the cast, as the provider names them. */
@Serializable
data class TitlePerson(val name: String, val character: String? = null, val photo: String? = null)

/** A title as the provider describes it, whole, with where it stands here. */
@Serializable
data class TitlePage(
    val found: Found,
    val tagline: String? = null,
    val releaseDate: String? = null,
    val runtimeMinutes: Int? = null,
    val rating: Double? = null,
    val ageRating: String? = null,
    val genres: List<String> = emptyList(),
    val bigPoster: String? = null,
    val backdrop: String? = null,
    val cast: List<TitlePerson> = emptyList(),
    val seasons: List<SeasonChoice> = emptyList(),
)

/** One of this account's requests. */
@Serializable
data class TitleRequest(
    val id: String,
    val catalogue: String,
    val tmdbId: String,
    val title: String,
    val year: Int? = null,
    val poster: String? = null,
    /** None for a film or a whole series. */
    val seasons: List<Int> = emptyList(),
    val note: String = "",
    /** pending, accepted, refused or added. */
    val state: String,
    /** The administrator's word on it. */
    val answer: String = "",
    val workId: String? = null,
    val createdAt: String = "",
)

/** What is asked for. */
@Serializable
data class Asking(
    val catalogue: String,
    val tmdbId: String,
    val seasons: List<Int>,
    val note: String,
    val language: String,
)
