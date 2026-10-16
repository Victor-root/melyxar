package app.melyxar.tv.inside

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import app.melyxar.R
import app.melyxar.tv.look.GlassField
import app.melyxar.tv.look.GlassIcon
import app.melyxar.tv.look.Icons

/** The order a list is read in, as the web's bar above a grid, on its
 *  piece of glass: the field opens the orders to choose from, the arrow
 *  turns the order round. */
@Composable
fun OrderButtons(order: String, descending: Boolean, memory: FocusMemory, onPick: () -> Unit, onTurn: () -> Unit) {
    GlassField(stringResource(R.string.tv_sort), order, onClick = onPick, modifier = Modifier.remembered(memory, "order"))
    GlassIcon(
        if (descending) Icons.ArrowDown else Icons.ArrowUp,
        label = stringResource(if (descending) R.string.tv_descending else R.string.tv_ascending),
        onClick = onTurn,
        modifier = Modifier.remembered(memory, "direction"),
    )
}
