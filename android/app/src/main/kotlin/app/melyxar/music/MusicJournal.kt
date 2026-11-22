package app.melyxar.music

import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import app.melyxar.BuildConfig
import app.melyxar.server.Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.io.IOException

/** The tag of the logcat, in the debug build, as the server's journal has
 *  it (`music-app`). */
private const val TAG = "MelyxarMusicApp"

/** How many times the sound card running dry are told one by one in each
 *  stretch of [HEALTH_EVERY_MS]; the rest are only counted. A sound that
 *  breaks up does so dozens of times a second. */
private const val RAN_DRY_TOLD_AT_MOST = 6

/** How often how the sound holds up is told, while a song is held. */
internal const val HEALTH_EVERY_MS = 10_000L

/** How much of a failure's words is sent, the server cutting it again. */
private const val WORDS_SENT = 300

/**
 * What the player of music hears of its own sound, told to the server's
 * journal under its own tag, `music-app`, and in the debug build to the
 * logcat: the sound card running dry, the waits for sound, each piece of a
 * song that came in and how fast, the form read and what the sound card is
 * handed, the sound taken by somebody else and what was done about it, the
 * ways out of the sound and whether a car holds it, and every
 * [HEALTH_EVERY_MS] how it held up.
 *
 * A sound that breaks up in a car is chased through these alone: nothing
 * else on the server hears the device play. The facts are the server's own
 * list (`music_watch/app.rs`), which refuses any other.
 */
@OptIn(UnstableApi::class)
class MusicJournal(
    context: Context,
    private val asking: () -> Api?,
    private val scope: CoroutineScope,
    /** The song playing, as the player holds it. */
    private val song: () -> String?,
) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val power = context.getSystemService(PowerManager::class.java)
    private val modes = context.getSystemService(UiModeManager::class.java)

    /** The sound card running dry in this stretch: told, and all of them. */
    private var ranDryTold = 0
    private var ranDry = 0
    private var stretchStarted = SystemClock.elapsedRealtime()

    init {
        // Said once as it is registered, with every way out there is, then
        // at each one plugged in or taken away.
        audio.registerAudioDeviceCallback(object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = outputsChanged()

            override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = outputsChanged()
        }, Handler(Looper.getMainLooper()))
    }

    private fun tell(what: String, fields: JsonObjectBuilder.() -> Unit) {
        val fact = buildJsonObject {
            song()?.let { put("song", it) }
            put("saw", "app")
            put("what", what)
            fields()
        }
        if (BuildConfig.TRACES) {
            Log.i(TAG, fact.toString())
        }
        val api = asking() ?: return
        scope.launch {
            try {
                api.tellTheMusicJournal(fact)
            } catch (_: IOException) {
                // Only the journal goes without it.
            }
        }
    }

    /** What one deck's player sees, told for that deck. */
    fun watching(deck: Int, isLive: () -> Boolean): AnalyticsListener = DeckWatch(deck, isLive)

    private inner class DeckWatch(private val deck: Int, private val isLive: () -> Boolean) : AnalyticsListener {
        private var asked = false
        private var waitingSince: Long? = null

        private fun second(at: AnalyticsListener.EventTime) = at.currentPlaybackPositionMs / 1000.0

        override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
            ranDry++
            if (ranDryTold >= RAN_DRY_TOLD_AT_MOST) {
                return
            }
            ranDryTold++
            tell("ran_dry") {
                put("deck", deck)
                put("live", isLive())
                put("second", second(eventTime))
                put("holds_ms", bufferSizeMs)
                put("since_fed_ms", elapsedSinceLastFeedMs)
                put("ahead_seconds", eventTime.totalBufferedDurationMs / 1000.0)
            }
        }

        override fun onPlayWhenReadyChanged(eventTime: AnalyticsListener.EventTime, playWhenReady: Boolean, reason: Int) {
            asked = playWhenReady
            if (!playWhenReady) {
                waitingSince = null
            }
        }

        override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
            when (state) {
                Player.STATE_BUFFERING -> if (asked && waitingSince == null) waitingSince = SystemClock.elapsedRealtime()
                Player.STATE_READY -> waitingSince?.let { since ->
                    waitingSince = null
                    tell("waited") {
                        put("deck", deck)
                        put("live", isLive())
                        put("second", second(eventTime))
                        put("waited_ms", SystemClock.elapsedRealtime() - since)
                    }
                }
                else -> waitingSince = null
            }
        }

        override fun onLoadCompleted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
            if (mediaLoadData.dataType != C.DATA_TYPE_MEDIA) {
                return
            }
            tell("fetched") {
                put("deck", deck)
                put("kilobytes", loadEventInfo.bytesLoaded / 1024)
                put("ms", loadEventInfo.loadDurationMs)
            }
        }

        override fun onLoadError(
            eventTime: AnalyticsListener.EventTime,
            loadEventInfo: LoadEventInfo,
            mediaLoadData: MediaLoadData,
            error: IOException,
            wasCanceled: Boolean,
        ) {
            tell("fetch_failed") {
                put("deck", deck)
                put("error", causes(error))
                put("cancelled", wasCanceled)
            }
        }

        override fun onAudioInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
            tell("sound_read") {
                put("deck", deck)
                put("form", format.sampleMimeType ?: format.containerMimeType ?: "unknown")
                format.sampleRate.takeIf { it != Format.NO_VALUE }?.let { put("sample_rate", it) }
                format.channelCount.takeIf { it != Format.NO_VALUE }?.let { put("channels", it) }
                format.bitrate.takeIf { it != Format.NO_VALUE }?.let { put("bitrate", it) }
            }
        }

        override fun onAudioTrackInitialized(eventTime: AnalyticsListener.EventTime, audioTrackConfig: AudioSink.AudioTrackConfig) {
            tell("sound_out") {
                put("deck", deck)
                put("encoding", audioTrackConfig.encoding)
                put("sample_rate", audioTrackConfig.sampleRate)
                put("channels", Integer.bitCount(audioTrackConfig.channelConfig))
                put("buffer_bytes", audioTrackConfig.bufferSize)
                put("offload", audioTrackConfig.offload)
            }
        }

        override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime, audioSinkError: Exception) = failed("output", audioSinkError)

        override fun onAudioCodecError(eventTime: AnalyticsListener.EventTime, audioCodecError: Exception) = failed("decoding", audioCodecError)

        override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) = failed("player", error)

        private fun failed(during: String, error: Throwable) {
            tell("sound_failed") {
                put("deck", deck)
                put("during", during)
                put("error", causes(error))
            }
        }

        override fun onPositionDiscontinuity(
            eventTime: AnalyticsListener.EventTime,
            oldPosition: Player.PositionInfo,
            newPosition: Player.PositionInfo,
            reason: Int,
        ) {
            // A deck plays one song: going on from it is the player's, and
            // told by the player.
            if (reason == Player.DISCONTINUITY_REASON_AUTO_TRANSITION) {
                return
            }
            tell("jumped") {
                put("deck", deck)
                put("from_second", oldPosition.positionMs / 1000.0)
                put("to_second", newPosition.positionMs / 1000.0)
                put(
                    "why",
                    when (reason) {
                        Player.DISCONTINUITY_REASON_SEEK -> "seek"
                        Player.DISCONTINUITY_REASON_SEEK_ADJUSTMENT -> "seek_adjusted"
                        Player.DISCONTINUITY_REASON_SKIP -> "skipped"
                        Player.DISCONTINUITY_REASON_REMOVE -> "removed"
                        else -> "other"
                    },
                )
            }
        }
    }

    /** The sound taken or given back by somebody else, and what was done. */
    fun focus(change: Int, did: String) {
        val named = when (change) {
            AudioManager.AUDIOFOCUS_GAIN -> "gain"
            AudioManager.AUDIOFOCUS_LOSS -> "loss"
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> "loss_transient"
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> "duck"
            else -> return
        }
        tell("focus") {
            put("change", named)
            put("did", did)
        }
    }

    /** The next song taking over, [lateMs] past its moment. */
    fun handedOver(from: Int, to: Int, lateMs: Long, fadeSeconds: Double) {
        tell("handed_over") {
            put("from", from)
            put("to", to)
            put("late_ms", lateMs)
            put("fade_seconds", fadeSeconds)
        }
    }

    /**
     * How the sound held up since the last time: told every
     * [HEALTH_EVERY_MS] while a song is held, with how long bringing each
     * piece of sound to its level took on the thread that plays.
     */
    fun health(playing: Boolean, second: Double, aheadSeconds: Double, level: GainProcessor.Spent) {
        val now = SystemClock.elapsedRealtime()
        val over = now - stretchStarted
        val dry = ranDry
        stretchStarted = now
        ranDry = 0
        ranDryTold = 0
        tell("health") {
            put("over_ms", over)
            put("playing", playing)
            put("second", second)
            put("ahead_seconds", aheadSeconds)
            put("ran_dry", dry)
            put("level_longest_ms", level.longestMs)
            put("level_busy_ms", level.busyMs)
            put("heat", heat())
            put("battery_saver", power.isPowerSaveMode)
            put("in_car", inCar())
            put("outputs", outputs())
        }
    }

    private fun outputsChanged() {
        tell("outputs") {
            put("outputs", outputs())
            put("in_car", inCar())
        }
    }

    /** Whether a car holds the sound: Android Auto connected to the music,
     *  or the device in its car mode. */
    private fun inCar(): Boolean =
        MusicService.carConnected || modes.currentModeType == Configuration.UI_MODE_TYPE_CAR

    private fun outputs() = buildJsonArray {
        audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            .map { outputNamed(it.type) }
            .distinct()
            .forEach { add(it) }
    }

    private fun heat(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return "unknown"
        }
        return when (power.currentThermalStatus) {
            PowerManager.THERMAL_STATUS_NONE -> "none"
            PowerManager.THERMAL_STATUS_LIGHT -> "light"
            PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
            PowerManager.THERMAL_STATUS_SEVERE -> "severe"
            PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
            PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
            PowerManager.THERMAL_STATUS_SHUTDOWN -> "shutdown"
            else -> "unknown"
        }
    }

    private companion object {
        /** A way out of the sound, in the server's words. */
        fun outputNamed(type: Int): String = when (type) {
            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, AudioDeviceInfo.TYPE_BUILTIN_SPEAKER_SAFE -> "speaker"
            AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "earpiece"
            AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_LINE_ANALOG,
            AudioDeviceInfo.TYPE_LINE_DIGITAL, AudioDeviceInfo.TYPE_AUX_LINE -> "wired"
            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "bluetooth_a2dp"
            AudioDeviceInfo.TYPE_BLUETOOTH_SCO -> "bluetooth_sco"
            AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER, AudioDeviceInfo.TYPE_BLE_BROADCAST -> "bluetooth_le"
            AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_ACCESSORY -> "usb"
            AudioDeviceInfo.TYPE_HDMI, AudioDeviceInfo.TYPE_HDMI_ARC, AudioDeviceInfo.TYPE_HDMI_EARC -> "hdmi"
            AudioDeviceInfo.TYPE_BUS -> "bus"
            AudioDeviceInfo.TYPE_REMOTE_SUBMIX -> "remote_submix"
            else -> "other"
        }

        fun causes(error: Throwable): String =
            generateSequence(error) { it.cause }.take(6)
                .joinToString(" <- ") { "${it.javaClass.simpleName}: ${it.message}" }
                .take(WORDS_SENT)
    }
}
