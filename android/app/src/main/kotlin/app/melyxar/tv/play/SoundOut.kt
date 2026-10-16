package app.melyxar.tv.play

import android.content.Context
import androidx.core.content.edit

/*
 * Whether this television hands a film's sound it cannot decode as it is to
 * the receiver behind it, kept on the television alone. Off, the server
 * makes that sound into one the television decodes, and the television's
 * volume keeps reaching it.
 */

private const val KEPT = "television"
private const val PASSED_ON = "sound_passed_on"

fun soundPassedOn(context: Context): Boolean =
    context.getSharedPreferences(KEPT, Context.MODE_PRIVATE).getBoolean(PASSED_ON, false)

fun passSoundOn(context: Context, passed: Boolean) =
    context.getSharedPreferences(KEPT, Context.MODE_PRIVATE).edit { putBoolean(PASSED_ON, passed) }
