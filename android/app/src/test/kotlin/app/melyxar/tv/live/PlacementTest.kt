package app.melyxar.tv.live

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class PlacementTest {
    @Test
    fun leavesBandsAboveAndBelowAWidePictureOnATallerScreen() {
        // 16:9 on 4:3: the picture is 1024 by 576, leaving 96 above and below.
        assertEquals(Room(0f, 96f), roomAround(1024, 768, 2560, 1440, 1f))
    }

    @Test
    fun leavesBandsAtTheSidesOfATallPicture() {
        assertEquals(Room(240f, 0f), roomAround(1920, 1080, 1080, 1080, 1f))
    }

    @Test
    fun countsPixelsWiderThanTheyAreHigh() {
        // 720 by 576 stretched to 16:9 fills a 16:9 screen.
        val room = roomAround(1024, 576, 720, 576, 1024f / 720)
        assertEquals(0f, room.across, 0.5f)
        assertEquals(0f, room.down, 0.5f)
    }

    @Test
    fun hasNoRoomBeforeThePictureIsKnown() {
        assertEquals(Room(0f, 0f), roomAround(1920, 1080, 0, 0, 1f))
    }

    @Test
    fun movedAllTheWayUpTheBandAboveIsGone() {
        val up = (1..PLACEMENT_STEPS + 5).fold(Placement()) { at, _ -> at.moved(0, -1) }
        assertEquals(Placement(0, -PLACEMENT_STEPS), up)
        assertEquals(0 to -96, up.offset(Room(0f, 96f)))
    }

    @Test
    fun movesHalfwayInHalfThePresses() {
        assertEquals(50 to 0, Placement(PLACEMENT_STEPS / 2, 0).offset(Room(100f, 0f)))
    }

    @Test
    fun isCentredUntilMoved() {
        assertEquals(true, Placement().centred)
        assertEquals(false, Placement().moved(1, 0).centred)
        assertEquals(true, Placement().moved(1, 0).moved(-1, 0).centred)
    }
}
