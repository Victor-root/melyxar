package app.melyxar.tv

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileNotFoundException

/** What a picture kept for the system's row is called, and nothing else. */
private val PICTURE_NAME = Regex("""[A-Za-z0-9_-]+\.jpg""")

/**
 * The pictures of the works in the system's Continue watching row, read by
 * the system's home screen, which cannot read the server's. Only those, and
 * only to read: nothing else of the application is reached through it.
 */
class WatchNextPictures : ContentProvider() {
    override fun onCreate() = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") {
            throw SecurityException("the pictures of the row are only read")
        }
        val name = uri.lastPathSegment?.takeIf { PICTURE_NAME.matches(it) } ?: throw FileNotFoundException(uri.toString())
        val kept = File(File(requireNotNull(context).filesDir, WatchNext.PICTURES), name)
        return ParcelFileDescriptor.open(kept, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri) = "image/jpeg"

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?) = 0

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?) = 0
}
