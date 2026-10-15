package app.melyxar.server

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

/** What the servers of the home network announce themselves as. */
private const val SERVICE_TYPE = "_melyxar._tcp."

/** A server found on the home network, under the name it gives itself. */
data class FoundServer(val origin: ServerOrigin, val name: String)

/**
 * The Melyxar servers the home network answers with, told one by one as
 * they are found, for as long as they are looked for. Each is checked as a
 * typed address is: only one that answers as a Melyxar server is told.
 */
fun serversAtHome(context: Context): Flow<FoundServer> = callbackFlow {
    val nsd = context.getSystemService(NsdManager::class.java)
    val heard = Channel<NsdServiceInfo>(Channel.UNLIMITED)
    val listening = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(service: NsdServiceInfo) {
            heard.trySend(service)
        }

        override fun onServiceLost(service: NsdServiceInfo) = Unit

        override fun onDiscoveryStarted(serviceType: String) = Unit

        override fun onDiscoveryStopped(serviceType: String) = Unit

        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
            close()
        }

        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
    }
    nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listening)
    // One after the other: older systems resolve only one at a time. A
    // server heard on several of its addresses is told once.
    launch {
        val told = HashSet<String>()
        for (service in heard) {
            if (service.serviceName in told) {
                continue
            }
            val origin = resolve(nsd, service)?.let(::originOf) ?: continue
            val branding = askBranding(origin) ?: continue
            told += service.serviceName
            send(FoundServer(origin, branding.serverName))
        }
    }
    awaitClose {
        heard.close()
        try {
            nsd.stopServiceDiscovery(listening)
        } catch (_: IllegalArgumentException) {
            // The search never started.
        }
    }
}

// What every version of Android resolves with: its successor needs Android 14.
@Suppress("DEPRECATION")
private suspend fun resolve(nsd: NsdManager, service: NsdServiceInfo): NsdServiceInfo? = suspendCancellableCoroutine { going ->
    nsd.resolveService(
        service,
        object : NsdManager.ResolveListener {
            override fun onServiceResolved(resolved: NsdServiceInfo) = going.resume(resolved)

            override fun onResolveFailed(failed: NsdServiceInfo, errorCode: Int) = going.resume(null)
        },
    )
}

@Suppress("DEPRECATION")
private fun originOf(resolved: NsdServiceInfo): ServerOrigin? = resolved.host?.hostAddress?.let { originAt(it, resolved.port) }

/**
 * The address a server found at home is reached at, in plain http since it
 * is at home; nothing for an address that is not, nor for a link-local IPv6
 * one, which needs the name of an interface to be reached.
 */
fun originAt(address: String, port: Int): ServerOrigin? {
    val host = if (':' in address) {
        if (address.lowercase().startsWith("fe80")) return null
        "[${address.substringBefore('%')}]"
    } else {
        address
    }
    return ServerOrigin("http://$host:$port").takeIf { isAtHome(host) }
}
