package app.melyxar.tv.door

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.DoorName
import app.melyxar.server.KeptSession
import app.melyxar.server.KnownServer
import app.melyxar.server.Picture
import app.melyxar.server.Refusal
import app.melyxar.server.Sessions
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.theme.paletteOf
import app.melyxar.theme.wantsLight
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.BoxShadow
import app.melyxar.tv.look.Ease
import app.melyxar.tv.look.FacePainting
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.MelyxarMark
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.TvLook
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rememberPictures
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowRaised
import java.io.IOException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** What the door knows of its server, once it has answered. */
private sealed interface Answer {
    data object Waiting : Answer

    data object Unreachable : Answer

    data class Open(val branding: Branding, val profiles: List<DoorName>) : Answer
}

/**
 * "Who is watching?": the accounts of the server, each a tile, as the web's
 * door draws them. A profile already signed in on this television opens at
 * once, or once its code is typed when it gave itself one; any other asks
 * for its password, once.
 */
@Composable
fun Door(
    api: Api,
    server: KnownServer,
    sessions: Sessions,
    onIn: (Api, Account, Branding) -> Unit,
    onChangeServer: () -> Unit,
    onBack: () -> Unit,
) {
    var answer by remember { mutableStateOf<Answer>(Answer.Waiting) }
    var tries by remember { mutableIntStateOf(0) }
    /** The profile asking for its password, or "" for a name typed by hand. */
    var knocking by remember { mutableStateOf<String?>(null) }
    /** The profile signed in here asking for its code before it opens. */
    var locked by remember { mutableStateOf<Locked?>(null) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(tries) {
        answer = try {
            val branding = api.branding()
            val offered = api.names().names
            val kept = sessions.of(api.origin).map { DoorName(it.name) }
            Answer.Open(branding, offered + kept.filter { one -> offered.none { it.name == one.name } })
        } catch (_: IOException) {
            Answer.Unreachable
        }
    }

    fun signedIn(session: KeptSession, account: Account, branding: Branding) {
        sessions.keep(api.origin, session)
        onIn(api.signedInAs(session.token), account, branding)
    }

    /** A profile already signed in here opens straight away, or once its
     *  code is typed; one whose session has ended asks for its password
     *  again. */
    fun open(name: String, branding: Branding) {
        val kept = sessions.of(api.origin).firstOrNull { it.name == name }
        if (kept == null) {
            knocking = name
            return
        }
        scope.launch {
            try {
                val theirs = api.signedInAs(kept.token)
                val account = theirs.me()
                if (theirs.hasTvCode()) {
                    locked = Locked(kept, account)
                } else {
                    signedIn(kept, account, branding)
                }
            } catch (refused: Refusal) {
                if (refused.signedOut) {
                    sessions.forget(api.origin, name)
                }
                knocking = name
            } catch (_: IOException) {
                answer = Answer.Unreachable
            }
        }
    }

    val open = answer as? Answer.Open
    val deviceIsDark = isSystemInDarkTheme()
    val palette = remember(open?.branding?.defaultTheme, deviceIsDark) {
        paletteOf(light = wantsLight("server", open?.branding?.defaultTheme ?: "dark", deviceIsDark))
    }
    TvLook(palette) {
        CompositionLocalProvider(LocalServer provides api, LocalPictures provides rememberPictures(api)) {
            StageLight {
                when (val now = answer) {
                    Answer.Waiting -> Unit
                    Answer.Unreachable -> Unreachable(server.name, onRetry = { tries += 1 }, onChangeServer)
                    is Answer.Open -> Profiles(
                        branding = now.branding,
                        profiles = now.profiles,
                        server = server,
                        onPick = { open(it, now.branding) },
                        onByHand = { knocking = "" },
                        onChangeServer = onChangeServer,
                        quiet = knocking != null || locked != null,
                    )
                }
                val asking = knocking
                if (asking != null && open != null) {
                    Knock(
                        api = api,
                        known = asking.ifEmpty { null },
                        avatar = open.profiles.firstOrNull { it.name == asking }?.avatar,
                        client = sessions.client,
                        onIn = { session, account -> signedIn(session, account, open.branding) },
                        onClose = { knocking = null },
                    )
                }
                val asked = locked
                if (asked != null && open != null) {
                    CodeLock(
                        api = api.signedInAs(asked.session.token),
                        name = asked.session.name,
                        avatar = open.profiles.firstOrNull { it.name == asked.session.name }?.avatar,
                        onOpen = { signedIn(asked.session, asked.account, open.branding) },
                        onClose = { locked = null },
                    )
                }
            }
        }
    }
    BackHandler(enabled = knocking == null && locked == null, onBack = onBack)
}

/** A profile signed in here, waiting on its code. */
private class Locked(val session: KeptSession, val account: Account)

@Composable
private fun Profiles(
    branding: Branding,
    profiles: List<DoorName>,
    server: KnownServer,
    onPick: (String) -> Unit,
    onByHand: () -> Unit,
    onChangeServer: () -> Unit,
    quiet: Boolean,
) {
    val palette = LocalPalette.current
    val first = remember { FocusRequester() }
    Column(
        modifier = Modifier.fillMaxSize().padding(horizontal = Gutter, vertical = 28.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            modifier = Modifier.weight(1f).fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            ServerMark(branding, 92.dp)
            Text(
                text = buildAnnotatedString {
                    append(stringResource(R.string.tv_welcome_before))
                    withStyle(
                        SpanStyle(
                            color = palette.accentBright,
                            shadow = Shadow(palette.accentGlow, Offset.Zero, blurRadius = 36f),
                        ),
                    ) { append(branding.serverName) }
                },
                style = TextStyle(fontFamily = MelyxarDisplay, fontSize = 36.sp, lineHeight = 40.sp, color = palette.text),
                modifier = Modifier.padding(top = 18.dp),
            )
            Text(
                stringResource(R.string.tv_pick),
                fontSize = 17.sp,
                color = palette.textSoft,
                modifier = Modifier.padding(top = 8.dp, bottom = 34.dp),
            )
            Row(
                horizontalArrangement = Arrangement.spacedBy(36.dp),
                modifier = Modifier.focusRestorer(first).drawBehind {
                    // The pool of light the tiles stand in.
                    val across = maxOf(size.width, TILE.toPx() * 4)
                    val center = Offset(size.width / 2, TILE.toPx() * 1.0f)
                    scaleFloor(center, across / 2, TILE.toPx() * 0.45f, palette.accent, palette.accentDim)
                },
            ) {
                profiles.forEachIndexed { at, profile ->
                    ProfileTile(
                        profile = profile,
                        arrivesAfterMs = 80 + minOf(at, 8) * 55,
                        onPick = { onPick(profile.name) },
                        modifier = if (at == 0) Modifier.focusRequester(first) else Modifier,
                    )
                }
                if (profiles.isEmpty()) {
                    PlainButton(stringResource(R.string.tv_by_hand), onByHand, Modifier.focusRequester(first))
                }
            }
        }
        // Coming down to the foot lands on the button left, or on its first.
        val foot = remember { FocusRequester() }
        Row(modifier = Modifier.fillMaxWidth().focusRestorer(foot), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(palette.ok))
            Text(
                "${server.name} · ${server.origin.substringAfter("://")}",
                fontSize = 14.sp,
                color = palette.textFaint,
                modifier = Modifier.padding(start = 10.dp).weight(1f),
            )
            if (profiles.isNotEmpty()) {
                PlainButton(stringResource(R.string.tv_by_hand), onByHand, Modifier.padding(end = 12.dp).focusRequester(foot))
            }
            PlainButton(
                stringResource(R.string.tv_change_server),
                onChangeServer,
                if (profiles.isEmpty()) Modifier.focusRequester(foot) else Modifier,
            )
        }
    }
    LaunchedEffect(quiet) {
        if (!quiet) {
            first.requestFocus()
        }
    }
}

/** The side of a profile's tile. */
private val TILE = 124.dp

private fun DrawScope.scaleFloor(
    center: Offset,
    radiusX: Float,
    radiusY: Float,
    accent: Color,
    dim: Color,
) {
    val squash = radiusY / radiusX
    drawContext.transform.scale(1f, squash, center)
    drawCircle(
        Brush.radialGradient(
            0f to accent.copy(alpha = 0.22f),
            0.55f to dim.copy(alpha = 0.08f),
            1f to Color.Transparent,
            center = center,
            radius = radiusX,
        ),
        radius = radiusX,
        center = center,
    )
    drawContext.transform.scale(1f, 1 / squash, center)
}

@Composable
private fun ProfileTile(profile: DoorName, arrivesAfterMs: Int, onPick: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val arrival = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        delay(arrivesAfterMs.toLong())
        arrival.animateTo(1f, tween(520, easing = Ease))
    }
    val shape = RoundedCornerShape(TILE * 0.1f)
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = modifier
            .graphicsLayer {
                alpha = arrival.value
                translationY = (1 - arrival.value) * 14.dp.toPx() - lit * 6.dp.toPx()
            }
            .clickable(interaction, indication = null, onClick = onPick),
    ) {
        Box(
            Modifier
                .size(TILE)
                .boxShadow(shape, palette.shadowRaised)
                .boxShadow(
                    shape,
                    BoxShadow(palette.accent.copy(alpha = 0.7f), blur = 0.dp, spread = 1.dp),
                    BoxShadow(palette.accent.copy(alpha = 0.6f), blur = 30.dp, spread = (-2).dp),
                    BoxShadow(palette.accentDim.copy(alpha = 0.7f), blur = 60.dp, spread = (-12).dp, y = 16.dp),
                ) { lit }
                .clip(shape)
                .rim(shape, palette.text.copy(alpha = 0.1f), 1.dp)
                .rim(shape, palette.accentBright, 2.dp) { lit },
        ) {
            Face(profile.name, profile.avatar, TILE)
        }
        Text(
            profile.name,
            fontSize = 16.sp,
            fontWeight = FontWeight.Medium,
            color = if (lit > 0.5f) palette.text else palette.textSoft,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 14.dp).width(TILE + 20.dp),
            textAlign = TextAlign.Center,
        )
    }
}

/** An account's own picture, or the painting the web gives it. */
@Composable
fun Face(name: String, avatar: String?, across: Dp, modifier: Modifier = Modifier) {
    if (avatar != null) {
        ServerPicture(listOf(Picture(avatar)), across, modifier.size(across))
    } else {
        FacePainting(name, modifier.size(across))
    }
}

/** The server's own logo when it has one, Melyxar's in the accent when not. */
@Composable
fun ServerMark(branding: Branding, size: Dp) {
    val palette = LocalPalette.current
    val logo = branding.logo
    val glow = Modifier.boxShadow(RoundedCornerShape(size / 2), BoxShadow(palette.accentGlow, blur = 24.dp, y = 8.dp))
    if (logo != null) {
        ServerPicture(listOf(Picture(logo)), size, glow.size(size), contentScale = ContentScale.Fit)
    } else {
        MelyxarMark(size, glow)
    }
}

@Composable
private fun Unreachable(name: String, onRetry: () -> Unit, onChangeServer: () -> Unit) {
    val palette = LocalPalette.current
    val first = remember { FocusRequester() }
    Column(
        modifier = Modifier.fillMaxSize().padding(Gutter),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        MelyxarMark(72.dp)
        Text(
            stringResource(R.string.unreachable_title),
            fontSize = 24.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            modifier = Modifier.padding(top = 18.dp),
        )
        Text(
            stringResource(R.string.unreachable_lead, name),
            fontSize = 16.sp,
            color = palette.textSoft,
            modifier = Modifier.padding(top = 8.dp, bottom = 26.dp).width(520.dp),
            textAlign = TextAlign.Center,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.focusRestorer(first)) {
            AccentButton(
                stringResource(R.string.unreachable_retry),
                onRetry,
                Modifier.focusRequester(first),
                icon = null,
            )
            PlainButton(stringResource(R.string.unreachable_change), onChangeServer)
        }
        Box(Modifier.height(40.dp))
    }
    LaunchedEffect(Unit) { first.requestFocus() }
}
