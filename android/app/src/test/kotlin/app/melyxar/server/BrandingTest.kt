package app.melyxar.server

import kotlinx.serialization.SerializationException
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

class BrandingTest {
    @Test
    fun `a server's answer is read, whatever else it carries`() {
        val read = brandingOf(
            """{"server_name":"Cinéma du salon","logo":"/api/v1/public/logo/logo-abc.webp",
               "logo_icon":null,"login_background_style":"drift","default_theme":"dark",
               "setup_complete":false}""",
        )
        assertEquals("Cinéma du salon", read.serverName)
        assertEquals("/api/v1/public/logo/logo-abc.webp", read.logo)
        assertFalse(read.setupComplete)
    }

    @Test
    fun `an answer that is not a server's is refused`() {
        assertThrows<SerializationException> { brandingOf("""{"hello":"world"}""") }
    }
}
