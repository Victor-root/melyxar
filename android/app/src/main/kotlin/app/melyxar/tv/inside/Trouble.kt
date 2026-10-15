package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette

/** What a screen shows when the server did not answer it: that, and a way
 *  to ask again. */
@Composable
fun Trouble(onRetry: () -> Unit) {
    val palette = LocalPalette.current
    val retry = remember { FocusRequester() }
    Column(
        modifier = Modifier.fillMaxSize().padding(Gutter),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically),
    ) {
        Text(stringResource(R.string.tv_trouble), fontSize = 18.sp, color = palette.textSoft, textAlign = TextAlign.Center)
        AccentButton(stringResource(R.string.tv_retry), onRetry, Modifier.focusRequester(retry), icon = null)
    }
    LaunchedEffect(Unit) { retry.requestFocus() }
}
