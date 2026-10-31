# Every library used here ships its own rules for R8 (Compose, AndroidX,
# Media3, kotlinx.serialization, coroutines, OkHttp, Coil, CameraX), Media3's
# covering the effects it loads by name to turn a picture. ZXing's reader of
# QR codes loads nothing by name and needs none. The methods a page calls
# through a JavaScript interface are kept by the default optimize file.

# MlxPerf: names kept while the smoothness probe reads the main thread's
# stack. Removed with the probe.
-dontobfuscate
