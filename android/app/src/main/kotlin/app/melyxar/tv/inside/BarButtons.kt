package app.melyxar.tv.inside

/** Every button of the bar, in the order everybody starts with, as the
 *  web's. */
private val EVERY_BUTTON = listOf(
    "search", "favourites", "watch_later", "collections", "playlists", "requests",
    "notifications", "scan", "administration", "cast", "settings",
)

/** The buttons a television has a page for. */
val ON_A_TELEVISION = setOf("search", "favourites", "watch_later")

/** The buttons on the bar and those in the account's menu, each in the
 *  account's order. */
data class BarButtons(val bar: List<String>, val menu: List<String>)

/** The account's buttons as it ordered them on the web, a button never
 *  placed after the others, those of no use on a television left out. */
fun barButtons(order: List<String>, inTheBar: List<String>, shown: Set<String> = ON_A_TELEVISION): BarButtons {
    val offered = (order + EVERY_BUTTON).filter { it in EVERY_BUTTON && it in shown }.distinct()
    return BarButtons(offered.filter { it in inTheBar }, offered.filterNot { it in inTheBar })
}

/** How long until the clock's minute turns, from [nowMillis]. */
fun untilTheNextMinute(nowMillis: Long): Long = 60_000 - nowMillis % 60_000
