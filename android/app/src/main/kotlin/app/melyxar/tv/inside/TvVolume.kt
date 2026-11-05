package app.melyxar.tv.inside

import android.content.Context
import android.database.ContentObserver
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.awaitCancellation
import kotlin.math.roundToInt

/**
 * The television's own volume, as its remote control changes it: set from
 * another device of the account, the system shows its bar of volume as it
 * would under a press of the remote's keys, and turned with those keys, the
 * remotes of the other devices follow.
 */
class TvVolume(context: Context) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val resolver = context.contentResolver

    /** From nothing to everything, as the system last said. */
    var level by mutableFloatStateOf(read())
        private set
    var muted by mutableStateOf(audio.isStreamMute(AudioManager.STREAM_MUSIC))
        private set

    /** How many steps the system's volume goes by, as its keys move it. */
    val steps: Int get() = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)

    /** Whether the system lets anything change it: a television handing its
     *  sound to an amplifier keeps its own volume fixed. */
    val fixed: Boolean get() = audio.isVolumeFixed

    private fun read(): Float {
        val most = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        return if (most > 0) audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat() / most else 0f
    }

    fun set(to: Float) {
        val most = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        val step = (to.coerceIn(0f, 1f) * most).roundToInt()
        audio.setStreamVolume(AudioManager.STREAM_MUSIC, step, AudioManager.FLAG_SHOW_UI)
    }

    fun mute(silent: Boolean) {
        val how = if (silent) AudioManager.ADJUST_MUTE else AudioManager.ADJUST_UNMUTE
        audio.adjustStreamVolume(AudioManager.STREAM_MUSIC, how, AudioManager.FLAG_SHOW_UI)
    }

    /** Follows the system's volume for as long as it is followed: every
     *  change of it is written in the system's settings. */
    suspend fun follow() {
        val watching = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                level = read()
                muted = audio.isStreamMute(AudioManager.STREAM_MUSIC)
            }
        }
        resolver.registerContentObserver(Settings.System.CONTENT_URI, true, watching)
        try {
            awaitCancellation()
        } finally {
            resolver.unregisterContentObserver(watching)
        }
    }
}
