package app.melyxar.tv.inside

import app.melyxar.server.MaintenanceSeen
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class MaintenanceTest {
    @Test
    fun anAnnouncementCountsDownFromTheSecondsTheServerGave() {
        val standing = standingOf(MaintenanceSeen(state = "announced", startsInSeconds = 90), heardAtMs = 1_000_000)
        assertEquals(1_090_000L, standing.startsAtMs)
        assertEquals("announced", standing.stateAt(1_089_999))
        assertEquals("on", standing.stateAt(1_090_000))
    }

    @Test
    fun whatIsNotAnnouncedKeepsNoCountdown() {
        val on = standingOf(MaintenanceSeen(state = "on", startsInSeconds = 30), heardAtMs = 5)
        assertNull(on.startsAtMs)
        assertEquals("on", on.stateAt(10))
        assertEquals("off", Standing.OFF.stateAt(10))
    }
}
