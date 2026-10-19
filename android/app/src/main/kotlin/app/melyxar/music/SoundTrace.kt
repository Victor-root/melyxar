package app.melyxar.music

import android.app.NotificationManager
import android.content.Context
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log

/**
 * Probe for the notification sound heard when the first song plays after a
 * cold start: everything that could make a sound around that moment, under
 * the tag MlxSound, in release builds too, to be read in a logcat taken on
 * the device that hears it. To be removed once the cause is found.
 */
object SoundTrace {
    private const val TAG = "MlxSound"

    private val main = Handler(Looper.getMainLooper())

    fun say(what: String) {
        Log.i(TAG, "t=${SystemClock.uptimeMillis()} $what")
    }

    /** Everything at once now, then again a little later, when what was
     *  asked has had the time to happen. */
    fun lookAround(context: Context, why: String) {
        lookNow(context, why)
        for (later in longArrayOf(300, 1000, 3000)) {
            main.postDelayed({ lookNow(context, "$why +${later}ms") }, later)
        }
    }

    private fun lookNow(context: Context, why: String) {
        say("look [$why]")
        channels(context)
        notifications(context)
        sounds(context)
    }

    private fun channels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        val all = manager.notificationChannels
        if (all.isEmpty()) {
            say("  channels: none")
        }
        for (channel in all) {
            say(
                "  channel id=${channel.id} name=${channel.name} importance=${channel.importance} " +
                    "sound=${channel.sound} vibrates=${channel.shouldVibrate()} lights=${channel.shouldShowLights()}",
            )
        }
    }

    private fun notifications(context: Context) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager ?: return
        val shown = manager.activeNotifications
        if (shown.isEmpty()) {
            say("  notifications: none")
        }
        for (one in shown) {
            val notification = one.notification
            val channel = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) notification.channelId else "-"
            @Suppress("DEPRECATION")
            say(
                "  notification id=${one.id} tag=${one.tag} channel=$channel flags=${notification.flags} " +
                    "category=${notification.category} sound=${notification.sound} defaults=${notification.defaults} " +
                    "posted=${one.postTime}",
            )
        }
    }

    private fun sounds(context: Context) {
        val audio = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager ?: return
        say("  audio mode=${audio.mode} ringer=${audio.ringerMode} music_active=${audio.isMusicActive}")
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        val playing = audio.activePlaybackConfigurations
        if (playing.isEmpty()) {
            say("  playing: none")
        }
        for (one in playing) {
            val attributes = one.audioAttributes
            say("  playing usage=${attributes.usage} content=${attributes.contentType} flags=${attributes.flags} $one")
        }
    }
}
