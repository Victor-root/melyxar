package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import app.melyxar.R
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.RoundMark

/** The order a list is read in, as the web's bar above a grid: the button
 *  opens the orders to choose from, the arrow turns the order round. */
@Composable
fun OrderButtons(order: String, descending: Boolean, memory: FocusMemory, onPick: () -> Unit, onTurn: () -> Unit) {
    PlainButton(
        stringResource(R.string.tv_sorted_by, order),
        onClick = onPick,
        modifier = Modifier.remembered(memory, "order"),
    )
    RoundMark(
        if (descending) Icons.ArrowDown else Icons.ArrowUp,
        label = stringResource(if (descending) R.string.tv_descending else R.string.tv_ascending),
        on = descending,
        onClick = onTurn,
        modifier = Modifier.remembered(memory, "direction"),
    )
}
