package app.melyxar.theme

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class FacesTest {
    @Test
    fun wearsThePictureTheServerGave() {
        assertEquals(FacePicture.Spiral, facePictureOf("Victor", "spiral"))
        assertEquals(FacePicture.Verdant, facePictureOf("Bob", "verdant"))
    }

    @Test
    fun picksByTheNameWhenTheServerGaveNone() {
        // What these names wore on every screen before the server gave one.
        assertEquals(
            listOf(
                FacePicture.Comet,
                FacePicture.Verdant,
                FacePicture.Spiral,
                FacePicture.Binary,
                FacePicture.Comet,
                FacePicture.Rings,
                FacePicture.Comet,
            ),
            listOf("Victor", "Élodie", "Zoë", "Marie-Hélène", "Bob", "", "🎬 Cinéma").map { facePictureOf(it, null) },
        )
        assertEquals(FacePicture.Comet, facePictureOf("Victor", "a picture this application does not have"))
    }
}
