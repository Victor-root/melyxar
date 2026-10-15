package app.melyxar.tv.inside

import android.content.ActivityNotFoundException
import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Browse
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.RoundMark
import app.melyxar.tv.look.TvField
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce

/** How long the words have to stay as they are before they are asked
 *  about, as the web's header waits. */
private const val SEARCH_WAIT_MS = 200L

/**
 * The search, as the web's: the title of a film, a series or a video, typed
 * with the television's keyboard or said into the remote, and the works it
 * names in a grid under it, as soon as the words stop changing.
 */
@OptIn(FlowPreview::class)
@Composable
fun SearchScreen(comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val words = rememberTextFieldState()
    var asked by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(words) {
        snapshotFlow { words.text.toString().trim() }
            .debounce(SEARCH_WAIT_MS)
            .collect { asked = it }
    }
    val memory = rememberFocusMemory()
    val field = remember { FocusRequester() }
    val works = rememberWorks(Browse(search = asked), comebacks, asking = asked.isNotEmpty())
    WorksGrid(
        works = works,
        lying = false,
        empty = stringResource(R.string.tv_search_none),
        memory = memory,
        grid = rememberLazyGridState(),
        onOpen = { chrome.go(Place.Opened(it.id)) },
    ) {
        Column {
            // The grid keeps the page's margin already.
            TopBar(edges = 0.dp)
            Text(
                stringResource(R.string.tv_search),
                fontSize = 28.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                modifier = Modifier.padding(top = 26.dp),
            )
            Row(
                modifier = Modifier.padding(top = 14.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                TvField(
                    state = words,
                    placeholder = stringResource(R.string.tv_search_placeholder),
                    onDone = {},
                    modifier = Modifier.weight(1f).remembered(memory, "field").focusRequester(field),
                )
                Listen(onHeard = { words.setTextAndPlaceCursorAtEnd(it) })
            }
            if (asked.isEmpty()) {
                Text(stringResource(R.string.tv_search_lead), fontSize = 15.sp, color = palette.textSoft)
            }
        }
    }
    RestoreFocus(memory, field, ready = true)
}

/**
 * The microphone beside the field, for a television that has something to
 * listen with: what was said is written into the field. Not drawn at all on
 * one that has not.
 */
@Composable
private fun Listen(onHeard: (String) -> Unit) {
    val context = LocalContext.current
    val asking = remember {
        Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
    }
    val canListen = remember { context.packageManager.queryIntentActivities(asking, 0).isNotEmpty() }
    val listen = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        result.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.let(onHeard)
    }
    if (!canListen) {
        return
    }
    val prompt = stringResource(R.string.tv_search_voice)
    RoundMark(
        Icons.Microphone,
        label = prompt,
        on = false,
        onClick = {
            try {
                listen.launch(Intent(asking).putExtra(RecognizerIntent.EXTRA_PROMPT, prompt))
            } catch (_: ActivityNotFoundException) {
                // Gone since the screen was drawn: nothing to listen with.
            }
        },
    )
}
