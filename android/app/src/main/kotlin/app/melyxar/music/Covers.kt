package app.melyxar.music

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import app.melyxar.server.PageSession
import okhttp3.Request
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.security.MessageDigest

/** Where the covers are handed out from, the application's own name before
 *  it: a Melyxar made under another name installs beside this one. */
private fun authorityOf(context: Context) = "${context.packageName}.covers"

/** The address a car reads the cover at [address] of the server from. */
fun coverAddress(context: Context, address: String): String =
    Uri.Builder().scheme("content").authority(authorityOf(context)).path("cover").appendQueryParameter("of", address).build().toString()

/** Lets [reader], a car, read every cover handed out, for as long as the
 *  phone runs. */
fun letReadTheCovers(context: Context, reader: String) {
    context.grantUriPermission(
        reader,
        Uri.Builder().scheme("content").authority(authorityOf(context)).build(),
        Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION,
    )
}

/**
 * The covers of the music, handed to what cannot ask the server itself: a
 * car has no session, and opens a cover by an address of the phone's. Each
 * is fetched once as the page fetches it, from the server the music plays
 * from and from nowhere else, and kept among the application's cached files,
 * which Android empties when it needs the room.
 *
 * Not open to every application: only those it was given to read, see
 * [letReadTheCovers].
 */
class CoverProvider : ContentProvider() {
    override fun onCreate() = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val address = uri.getQueryParameter("of") ?: throw FileNotFoundException("no cover named")
        val reading = requireNotNull(context)
        val api = PageSession.api(reading) ?: throw FileNotFoundException("nobody signed in")
        if (!address.startsWith("${api.origin.value}/")) {
            throw FileNotFoundException("not the server's")
        }
        val folder = File(reading.cacheDir, FOLDER).apply { mkdirs() }
        val cover = File(folder, nameOf(address))
        if (!cover.exists()) {
            try {
                api.http.newCall(Request.Builder().url(address).build()).execute().use { answer ->
                    if (!answer.isSuccessful) {
                        throw FileNotFoundException("refused: ${answer.code}")
                    }
                    val beside = File(folder, "${cover.name}.new")
                    beside.outputStream().use { answer.body.byteStream().copyTo(it) }
                    beside.renameTo(cover)
                }
            } catch (failure: IOException) {
                throw FileNotFoundException(failure.message)
            }
        }
        return ParcelFileDescriptor.open(cover, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri): String = "image/*"

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    private companion object {
        const val FOLDER = "covers"

        /** A file's name for an address, the same each time. */
        fun nameOf(address: String): String =
            MessageDigest.getInstance("SHA-256").digest(address.toByteArray()).joinToString("") { "%02x".format(it) }
    }
}
