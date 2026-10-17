package app.melyxar.tv.trailer

import app.melyxar.server.WorkTrailer
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class TrailersTest {
    private val link = WorkTrailer(remoteUrl = "https://www.youtube.com/watch?v=abc")
    private val file = WorkTrailer(url = "/api/v1/trailers/7")

    @Test
    fun aFileNextToTheFilmIsPreferredToAnyLink() {
        assertEquals(Trailer.Here("/api/v1/trailers/7"), trailerOf(listOf(link, file)))
    }

    @Test
    fun withoutAFileTheFirstLinkIsOffered() {
        assertEquals(Trailer.Away("https://www.youtube.com/watch?v=abc"), trailerOf(listOf(WorkTrailer(), link)))
        assertEquals(Trailer.Away("https://vimeo.com/1"), trailerOfLinks(listOf("https://vimeo.com/1", "https://vimeo.com/2")))
    }

    @Test
    fun nothingIsOfferedWithoutEither() {
        assertNull(trailerOf(emptyList()))
        assertNull(trailerOf(listOf(WorkTrailer())))
        assertNull(trailerOfLinks(emptyList()))
    }
}
