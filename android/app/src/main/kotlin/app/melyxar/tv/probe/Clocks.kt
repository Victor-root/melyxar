package app.melyxar.tv.probe

import android.util.Log
import java.io.File

/**
 * How fast the box's chips run at a given moment, as far as the system lets
 * an application read it: a graphics chip slowed down while it idled draws
 * the next frames late however little they ask of it.
 */
internal object Clocks {
    private const val TAG = "MlxPerf.Clock"

    /** Where the system lists the chips whose speed it changes. */
    private const val DEVFREQ = "/sys/class/devfreq"

    /** Where the graphics chip of this kind of box says how busy it is. */
    private val LOADS = listOf("/sys/devices/57000000.gpu/load", "/sys/devices/gpu.0/load")

    private var speeds = emptyList<Pair<String, File>>()
    private var load: File? = null

    /** Finds what can be read, and says so once. */
    fun find() {
        val chips = File(DEVFREQ).listFiles().orEmpty().sortedBy { it.name }
        speeds = chips.map { it.name to File(it, "cur_freq") }.filter { (_, file) -> readable(file) }
        load = LOADS.map(::File).firstOrNull(::readable)
        val unread = chips.map { it.name }.filter { name -> speeds.none { it.first == name } }
        Log.i(TAG, "speeds read: ${speeds.joinToString { it.first }.ifEmpty { "none" }}; not readable: ${unread.joinToString().ifEmpty { "none" }}; graphics load: ${load?.path ?: "not readable"}")
    }

    /** Each chip's speed now, in MHz, and the graphics chip's load. */
    fun now(): String {
        val parts = speeds.map { (name, file) -> "$name=${read(file)?.let { it / 1_000_000 } ?: "?"}" }
        val busy = load?.let { file -> read(file)?.let { "gpuload=${it / 10}%" } }
        return (parts + listOfNotNull(busy)).joinToString(" ").ifEmpty { "clocks unreadable" }
    }

    private fun readable(file: File) = read(file) != null

    private fun read(file: File): Long? = try {
        file.readText().trim().toLongOrNull()
    } catch (_: Exception) {
        null
    }
}
