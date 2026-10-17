package app.melyxar.server

import kotlinx.serialization.SerializationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.jsonObject

/*
 * The title requests, asked as the web's page of requests asks them, in the
 * [language] the screen reads in, which the provider answers in.
 */

suspend fun Api.requestAccess(): RequestAccess = get("/api/v1/requests/access", RequestAccess.serializer())

/** Titles answering [words]: a name, or an identifier or link of the provider. */
suspend fun Api.searchTitles(words: String, language: String): List<Found> =
    get(
        asking("/api/v1/requests/search") {
            addQueryParameter("query", words)
            addQueryParameter("language", language)
        },
        ListSerializer(Found.serializer()),
    )

suspend fun Api.titleSuggestions(language: String): List<TitleShelf> =
    get(asking("/api/v1/requests/suggestions") { addQueryParameter("language", language) }, ListSerializer(TitleShelf.serializer()))

/** One page of the most popular titles of a [catalogue], in a [genre] when
 *  one is named. */
suspend fun Api.popularTitles(catalogue: String, genre: String?, page: Int, language: String): List<Found> =
    get(
        asking("/api/v1/requests/popular") {
            addQueryParameter("catalogue", catalogue)
            addQueryParameter("page", page.toString())
            addQueryParameter("language", language)
            genre?.let { addQueryParameter("genre", it) }
        },
        ListSerializer(Found.serializer()),
    )

suspend fun Api.seasonsToAsk(tmdbId: String, language: String): List<SeasonChoice> =
    get(asking("/api/v1/requests/series/$tmdbId/seasons") { addQueryParameter("language", language) }, ListSerializer(SeasonChoice.serializer()))

suspend fun Api.titlePage(catalogue: String, tmdbId: String, language: String): TitlePage =
    get(asking("/api/v1/requests/title/$catalogue/$tmdbId") { addQueryParameter("language", language) }, TitlePage.serializer())

/** This account's requests, newest first. */
suspend fun Api.myRequests(): List<TitleRequest> = get("/api/v1/requests", ListSerializer(TitleRequest.serializer()))

/** Asks for a title, and gives back the request as the server keeps it. */
suspend fun Api.askFor(asking: Asking): TitleRequest {
    val sent = serverJson.encodeToJsonElement(Asking.serializer(), asking).jsonObject
    val made = send("POST", "/api/v1/requests", sent) ?: throw Refusal(200, "unreadable: no request")
    return try {
        serverJson.decodeFromJsonElement(TitleRequest.serializer(), made)
    } catch (failure: SerializationException) {
        throw Refusal(200, "unreadable: ${failure.message}")
    }
}

/** Takes a request back, or clears one decided from the list. */
suspend fun Api.withdrawRequest(id: String) {
    send("DELETE", "/api/v1/requests/$id", null)
}
