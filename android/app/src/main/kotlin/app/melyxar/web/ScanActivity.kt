package app.melyxar.web

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.widget.FrameLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import app.melyxar.R
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/**
 * The phone's camera, reading the QR code a television or a browser shows
 * at its door, for the page that lets it in. Whatever the code holds goes
 * back to the page as it is read, and the page keeps only a code of its
 * own server's: a QR code leading elsewhere is never followed from here.
 */
class ScanActivity : ComponentActivity() {
    private lateinit var picture: PreviewView
    private lateinit var said: TextView
    private val reading = Executors.newSingleThreadExecutor()
    private val done = AtomicBoolean(false)

    private val askCamera = registerForActivityResult(ActivityResultContracts.RequestPermission()) { given ->
        if (given) look() else said.text = getString(R.string.scan_no_camera)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        picture = PreviewView(this)
        said = TextView(this).apply {
            text = getString(R.string.scan_aim)
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 17f)
            setShadowLayer(6f, 0f, 1f, Color.BLACK)
            gravity = Gravity.CENTER
        }
        val padding = (28 * resources.displayMetrics.density).toInt()
        setContentView(
            FrameLayout(this).apply {
                setBackgroundColor(Color.BLACK)
                addView(picture, FrameLayout.LayoutParams(MATCH, MATCH))
                addView(
                    said,
                    FrameLayout.LayoutParams(MATCH, WRAP, Gravity.BOTTOM).apply {
                        setMargins(padding, padding, padding, padding * 2)
                    },
                )
            },
        )
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            look()
        } else {
            askCamera.launch(Manifest.permission.CAMERA)
        }
    }

    private fun look() {
        val asked = ProcessCameraProvider.getInstance(this)
        asked.addListener({
            val cameras = asked.get()
            val preview = Preview.Builder().build().also { it.surfaceProvider = picture.surfaceProvider }
            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
                .also { it.setAnalyzer(reading, ::read) }
            cameras.unbindAll()
            cameras.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
        }, ContextCompat.getMainExecutor(this))
    }

    private val reader = MultiFormatReader().apply {
        setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE)))
    }

    /** One image of the camera, its brightness alone, which is all a QR code
     *  is read from. */
    private fun read(image: ImageProxy) {
        image.use {
            if (done.get()) return
            val plane = it.planes[0]
            val bytes = ByteArray(plane.buffer.remaining()).also { into -> plane.buffer.get(into) }
            val source = PlanarYUVLuminanceSource(bytes, plane.rowStride, it.height, 0, 0, it.width, it.height, false)
            val found = try {
                reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text
            } catch (_: NotFoundException) {
                null
            } finally {
                reader.reset()
            }
            if (found != null && done.compareAndSet(false, true)) {
                runOnUiThread {
                    setResult(RESULT_OK, Intent().putExtra(READ, found))
                    finish()
                }
            }
        }
    }

    override fun onDestroy() {
        reading.shutdown()
        super.onDestroy()
    }

    companion object {
        /** What the QR code held, handed back to whoever opened this. */
        const val READ = "read"
        private const val MATCH = FrameLayout.LayoutParams.MATCH_PARENT
        private const val WRAP = FrameLayout.LayoutParams.WRAP_CONTENT
    }
}
