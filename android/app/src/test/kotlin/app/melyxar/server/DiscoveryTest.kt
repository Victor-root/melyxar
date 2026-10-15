package app.melyxar.server

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class DiscoveryTest {
    @Test
    fun reachesAServerFoundAtHomeInPlainHttp() {
        assertEquals(ServerOrigin("http://192.168.1.20:2100"), originAt("192.168.1.20", 2100))
        assertEquals(ServerOrigin("http://[fd00::20]:2100"), originAt("fd00::20", 2100))
    }

    @Test
    fun leavesOutWhatCannotBeReachedAsItIs() {
        assertNull(originAt("fe80::1%wlan0", 2100))
        assertNull(originAt("8.8.8.8", 2100))
    }
}
