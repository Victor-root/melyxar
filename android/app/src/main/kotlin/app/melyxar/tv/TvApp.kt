package app.melyxar.tv

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.melyxar.server.Account
import app.melyxar.server.Api
import app.melyxar.server.Branding
import app.melyxar.server.KnownServer
import app.melyxar.server.KnownServers
import app.melyxar.server.ServerOrigin
import app.melyxar.server.Sessions
import app.melyxar.server.network
import app.melyxar.tv.door.Door
import app.melyxar.tv.door.ServerChoice
import app.melyxar.tv.inside.Inside

/** Where the television is: choosing its server, at its door, or inside
 *  as one account. */
private sealed interface Stage {
    data object ChoosingServer : Stage

    data class AtTheDoor(val server: KnownServer) : Stage

    data class In(val api: Api, val account: Account, val branding: Branding, val server: KnownServer) : Stage
}

@Composable
fun TvApp(servers: KnownServers, sessions: Sessions, opening: String?, onOpened: () -> Unit, onLeave: () -> Unit) {
    var stage by remember {
        mutableStateOf(servers.current()?.let { Stage.AtTheDoor(it) } ?: Stage.ChoosingServer)
    }
    when (val now = stage) {
        Stage.ChoosingServer -> ServerChoice(
            servers = servers,
            onChosen = { server ->
                servers.open(server)
                stage = Stage.AtTheDoor(server)
            },
            onBack = { servers.current()?.let { stage = Stage.AtTheDoor(it) } ?: onLeave() },
        )
        is Stage.AtTheDoor -> Door(
            api = remember(now.server) { Api(ServerOrigin(now.server.origin), network, token = null) },
            server = now.server,
            sessions = sessions,
            onIn = { api, account, branding -> stage = Stage.In(api, account, branding, now.server) },
            onChangeServer = { stage = Stage.ChoosingServer },
            onBack = onLeave,
        )
        is Stage.In -> Inside(
            api = now.api,
            account = now.account,
            branding = now.branding,
            opening = opening,
            onOpened = onOpened,
            onChangeProfile = { stage = Stage.AtTheDoor(now.server) },
            onSignedOut = {
                sessions.forget(now.api.origin, now.account.name)
                stage = Stage.AtTheDoor(now.server)
            },
            onLeave = onLeave,
        )
    }
}
