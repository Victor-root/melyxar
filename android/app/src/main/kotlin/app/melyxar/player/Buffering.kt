package app.melyxar.player

import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.LoadControl

/**
 * How much of a film a player holds ahead: no more than a quarter of what
 * the application may hold in all. Left to Media3, the picture alone may
 * take 131 MB of it, kept in the application's own memory, which on a
 * television is 192 MB shared with everything else: a film of a high rate
 * filled it and the application was stopped for want of memory.
 */
@OptIn(UnstableApi::class)
fun boundedLoading(): LoadControl =
    DefaultLoadControl.Builder()
        .setTargetBufferBytes((Runtime.getRuntime().maxMemory() / 4).toInt())
        .build()
