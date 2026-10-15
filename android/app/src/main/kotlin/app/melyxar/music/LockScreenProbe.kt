package app.melyxar.music

import android.app.KeyguardManager
import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.provider.Settings
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.media3.session.MediaSession

/**
 * Debug only: what the phone knows of the music card each time the screen
 * goes off, comes on or is unlocked, to find why the lock screen leaves it
 * out. Read with `adb logcat -s MelyxarLock`.
 */
class LockScreenProbe(private val context: Context, private val session: () -> MediaSession?) {
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = report(intent.action ?: "?")
    }

    fun start() {
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_OFF)
            addAction(Intent.ACTION_SCREEN_ON)
            addAction(Intent.ACTION_USER_PRESENT)
        }
        ContextCompat.registerReceiver(context, receiver, filter, ContextCompat.RECEIVER_NOT_EXPORTED)
        report("service created")
    }

    fun stop() = context.unregisterReceiver(receiver)

    fun report(moment: String) {
        val notifications = context.getSystemService(NotificationManager::class.java)
        val keyguard = context.getSystemService(KeyguardManager::class.java)
        Log.d(TAG, "---- $moment, locked=${keyguard.isKeyguardLocked}, secure=${keyguard.isKeyguardSecure}")
        Log.d(TAG, "notifications enabled=${notifications.areNotificationsEnabled()}")
        session()?.player?.let { player ->
            Log.d(
                TAG,
                "session playWhenReady=${player.playWhenReady} state=${player.playbackState} " +
                    "title=${player.mediaMetadata.title} commands=${player.availableCommands.size()}",
            )
        } ?: Log.d(TAG, "no session")
        Log.d(TAG, "android=${Build.VERSION.SDK_INT} maker=${Build.MANUFACTURER} model=${Build.MODEL}")
        SETTINGS.forEach { key ->
            val value = runCatching { Settings.Secure.getString(context.contentResolver, key) }
                .getOrElse { "unreadable (${it.javaClass.simpleName})" }
            Log.d(TAG, "setting $key=$value")
        }
        notifications.activeNotifications.forEach { shown ->
            val card = shown.notification
            val channel = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) card.channelId else "-"
            Log.d(
                TAG,
                "notification id=${shown.id} channel=$channel visibility=${card.visibility} " +
                    "category=${card.category} flags=0x${Integer.toHexString(card.flags)} " +
                    "template=${card.extras.getString("android.template")} " +
                    "mediaSession=${card.extras.containsKey("android.mediaSession")} " +
                    "public=${card.publicVersion != null}",
            )
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            notifications.notificationChannels.forEach { channel ->
                Log.d(
                    TAG,
                    "channel id=${channel.id} name=${channel.name} importance=${channel.importance} " +
                        "lockscreen=${channel.lockscreenVisibility}",
                )
            }
        }
    }

    private companion object {
        const val TAG = "MelyxarLock"
        val SETTINGS = listOf(
            "lock_screen_show_notifications",
            "lock_screen_allow_private_notifications",
            "lock_screen_show_silent_notifications",
            "media_controls_lock_screen",
            "media_controls_resume",
            "qs_media_controls",
        )
    }
}
