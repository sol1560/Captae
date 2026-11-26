package app.captae.app.ui.computer

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors

// 扫配对二维码：CameraX 取景 + ZXing（只用 core，Apache-2.0）解码。也能从一张图片里认（模拟器没有真相机时用截图，
// 也给拿到二维码截图的人用）。只认 QR，解码在后台线程，同一个内容只报一次。

private val hints = mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE), DecodeHintType.TRY_HARDER to true)

/** 相机取景，认出二维码就回调 onText（主线程）。相机画面上不放玻璃（SurfaceView 采不到） */
@Composable
fun QrCameraPreview(onText: (String) -> Unit, modifier: Modifier = Modifier) {
  val context = LocalContext.current
  val owner = LocalLifecycleOwner.current
  val latest by rememberUpdatedState(onText)
  val executor = remember { Executors.newSingleThreadExecutor() }
  val view = remember { PreviewView(context).apply { scaleType = PreviewView.ScaleType.FILL_CENTER } }
  DisposableEffect(owner) {
    val future = ProcessCameraProvider.getInstance(context)
    var provider: ProcessCameraProvider? = null
    var last: String? = null
    val reader = MultiFormatReader().apply { setHints(hints) }
    future.addListener({
      val p = future.get()
      provider = p
      val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
      val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
      analysis.setAnalyzer(executor) { img: ImageProxy ->
        try {
          val plane = img.planes[0]
          val buf = plane.buffer
          val data = ByteArray(buf.remaining()).also { buf.get(it) }
          val src = PlanarYUVLuminanceSource(data, plane.rowStride, img.height, 0, 0, img.width, img.height, false)
          val text = runCatching { reader.decodeWithState(BinaryBitmap(HybridBinarizer(src))).text }.getOrNull()
          reader.reset()
          if (text != null && text != last) {
            last = text
            ContextCompat.getMainExecutor(context).execute { latest(text) }
          }
        } finally {
          img.close()
        }
      }
      try {
        p.unbindAll()
        p.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
      } catch (e: Exception) {
        // 没有后置相机（模拟器的设置、平板）：换前置再试
        runCatching { p.bindToLifecycle(owner, CameraSelector.DEFAULT_FRONT_CAMERA, preview, analysis) }.onFailure { Log.w("CaptaeScan", "开不了相机", e) }
      }
    }, ContextCompat.getMainExecutor(context))
    onDispose {
      provider?.unbindAll()
      executor.shutdown()
    }
  }
  AndroidView({ view }, modifier)
}

/** 从一张图片里认二维码（相册里的截图）。认不出返回 null */
fun decodeQrFromImage(context: Context, uri: Uri): String? = try {
  val bmp: Bitmap = if (Build.VERSION.SDK_INT >= 28) {
    ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, uri)) { d, _, _ -> d.allocator = ImageDecoder.ALLOCATOR_SOFTWARE }
  } else {
    @Suppress("DEPRECATION") MediaStore.Images.Media.getBitmap(context.contentResolver, uri)
  }
  val px = IntArray(bmp.width * bmp.height)
  bmp.getPixels(px, 0, bmp.width, 0, 0, bmp.width, bmp.height)
  val src = RGBLuminanceSource(bmp.width, bmp.height, px)
  MultiFormatReader().apply { setHints(hints) }.decodeWithState(BinaryBitmap(HybridBinarizer(src))).text
} catch (e: Exception) {
  Log.w("CaptaeScan", "图片里没认出二维码", e)
  null
}
