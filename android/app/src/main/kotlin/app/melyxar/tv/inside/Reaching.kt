package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow

/**
 * Where the remote has come to in a list read a page at a time: the place
 * given to what is returned, as the remote comes to each piece, is handed to
 * [reachedTo], which reads on when it is near the end.
 *
 * The place is followed away from the composition, so a move of the remote
 * composes nothing, and a page being read is not cancelled by the next move.
 * It is handed again whenever [list] or what was [read] of it changes.
 */
@Composable
fun rememberReaching(list: Any, read: Any?, reachedTo: suspend (Int) -> Unit): (Int) -> Unit {
    val reached = remember(list) { mutableIntStateOf(0) }
    val reaching by rememberUpdatedState(reachedTo)
    LaunchedEffect(reached, read) {
        snapshotFlow { reached.intValue }.collect { reaching(it) }
    }
    return remember(reached) { { at -> reached.intValue = at } }
}
