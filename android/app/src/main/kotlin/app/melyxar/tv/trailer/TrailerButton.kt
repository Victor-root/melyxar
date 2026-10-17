package app.melyxar.tv.trailer

import android.content.ActivityNotFoundException
import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.core.net.toUri
import app.melyxar.R
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised

/**
 * The trailer of the work called [title], offered beside how to play it: a
 * file of the server played in Melyxar, or a link opened in the application
 * of the television made for it. A link nothing here can open says so.
 */
@Composable
fun TrailerButton(trailer: Trailer, title: String, modifier: Modifier = Modifier) {
    val chrome = LocalChrome.current
    val context = LocalContext.current
    var nowhere by remember { mutableStateOf(false) }
    PlainButton(
        stringResource(R.string.tv_trailer),
        onClick = {
            when (trailer) {
                is Trailer.Here -> chrome.go(Place.Trailer(title, trailer.url))
                is Trailer.Away -> try {
                    context.startActivity(Intent(Intent.ACTION_VIEW, trailer.link.toUri()))
                } catch (_: ActivityNotFoundException) {
                    nowhere = true
                }
            }
        },
        modifier = modifier,
        icon = Icons.Trailer,
    )
    if (nowhere) {
        Nowhere(onClose = { nowhere = false })
    }
}

/** Said when no application of the television opens a trailer's link; the
 *  remote on the one way out, which the back key is too. */
@Composable
private fun Nowhere(onClose: () -> Unit) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(18.dp)
    val close = remember { FocusRequester() }
    Dialog(onDismissRequest = onClose) {
        Column(
            modifier = Modifier
                .width(520.dp)
                .boxShadow(shape, palette.shadowRaised)
                .clip(shape)
                .background(palette.surfaceRaised)
                .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
                .padding(28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Text(stringResource(R.string.tv_trailer_no_app), fontSize = 17.sp, color = palette.text)
            AccentButton(stringResource(R.string.tv_back), onClick = onClose, modifier = Modifier.focusRequester(close), icon = null)
        }
        LaunchedEffect(Unit) { close.requestFocus() }
    }
}
