package app.melyxar.tv.probe

import android.os.SystemClock
import android.util.Log
import coil3.EventListener
import coil3.request.ErrorResult
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import java.util.concurrent.ConcurrentHashMap

/** Logs every picture the screens ask for: where it came from, its size
 *  and how long it took, a picture not from memory fading in on screen. */
object PictureProbe : EventListener() {
    private const val TAG = "MlxPerf.Picture"
    private val started = ConcurrentHashMap<ImageRequest, Long>()

    override fun onStart(request: ImageRequest) {
        started[request] = SystemClock.uptimeMillis()
    }

    override fun onSuccess(request: ImageRequest, result: SuccessResult) {
        Probe.pictureShown(result.dataSource.name)
        Log.d(TAG, "${result.dataSource.name} ${result.image.width}x${result.image.height} in ${took(request)} ms ${request.data}")
    }

    override fun onError(request: ImageRequest, result: ErrorResult) {
        Log.w(TAG, "failed in ${took(request)} ms ${request.data}: ${result.throwable}")
    }

    private fun took(request: ImageRequest) = started.remove(request)?.let { SystemClock.uptimeMillis() - it } ?: -1
}
