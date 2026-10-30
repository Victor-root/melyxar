package app.melyxar.server

import kotlinx.serialization.Serializable

/**
 * Where maintenance stands, as the server says it (crates/app/src/maintenance.rs):
 * off, announced and counting down, or on, closing the server to all but its
 * administrators.
 */
@Serializable
data class MaintenanceSeen(
    /** off, announced or on. */
    val state: String = "off",
    /** How long before it takes hold, while announced, counted by the server
     *  so a television whose clock is wrong still counts down right. */
    val startsInSeconds: Long? = null,
    /** When the administrator expects it to end. */
    val until: String? = null,
    val message: String? = null,
)

/** Where maintenance stands, answered to anybody. */
suspend fun Api.maintenance(): MaintenanceSeen = get("/api/v1/public/maintenance", MaintenanceSeen.serializer())
