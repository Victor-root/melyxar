import com.android.build.api.artifact.ArtifactTransformationRequest
import com.android.build.api.artifact.SingleArtifact
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// The signing key, read from android/keystore.properties on the machine that
// builds (see keystore.properties.example). It signs debug builds as well as
// release ones, so the one installed from the Run button updates the one
// installed from a release. Without the file, the build falls back to the
// debug key Android Studio makes, and still builds.
val keystore = Properties().apply {
    val file = rootProject.file("keystore.properties")
    if (file.exists()) {
        file.inputStream().use { load(it) }
    }
}
val signsWithOwnKey = keystore.getProperty("storeFile")
    ?.takeIf { it.isNotBlank() }
    ?.let { rootProject.file(it).exists() } == true

android {
    namespace = "app.melyxar"
    compileSdk = 37

    defaultConfig {
        applicationId = "app.melyxar.vroot"
        minSdk = 24
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }

    signingConfigs {
        if (signsWithOwnKey) {
            create("own") {
                storeFile = rootProject.file(keystore.getProperty("storeFile"))
                storePassword = keystore.getProperty("storePassword")
                keyAlias = keystore.getProperty("keyAlias")
                keyPassword = keystore.getProperty("keyPassword")
            }
        }
    }

    // Debug is built as a release is, shrunk and optimised and not
    // debuggable, so that how smooth it runs is how smooth a release runs:
    // only its traces set it apart (BuildConfig.TRACES). It can still be
    // profiled, but no debugger attaches to it.
    buildTypes {
        configureEach {
            if (signsWithOwnKey) {
                signingConfig = signingConfigs.getByName("own")
            }
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        debug {
            isDebuggable = false
            isProfileable = true
            buildConfigField("boolean", "TRACES", "true")
        }
        release {
            buildConfigField("boolean", "TRACES", "false")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.all { it.useJUnitPlatform() }
    }
}

// A release comes out named Melyxar-<version>.apk rather than app-release.apk,
// wherever it is built from.
abstract class NamedApk : DefaultTask() {
    @get:InputFiles
    abstract val built: DirectoryProperty

    @get:OutputDirectory
    abstract val named: DirectoryProperty

    @get:Input
    abstract val fileName: Property<String>

    @get:Internal
    abstract val request: Property<ArtifactTransformationRequest<NamedApk>>

    @TaskAction
    fun rename() {
        request.get().submit(this) { apk ->
            val target = named.get().file(fileName.get()).asFile
            File(apk.outputFile).copyTo(target, overwrite = true)
            target
        }
    }
}

androidComponents {
    onVariants(selector().withBuildType("release")) { variant ->
        val naming = tasks.register<NamedApk>("name${variant.name.replaceFirstChar(Char::uppercase)}Apk") {
            fileName.set(variant.outputs.single().versionName.map { "Melyxar-$it.apk" })
        }
        val request = variant.artifacts.use(naming)
            .wiredWithDirectories(NamedApk::built, NamedApk::named)
            .toTransformMany(SingleArtifact.APK)
        naming.configure { this.request.set(request) }
    }
}

composeCompiler {
    stabilityConfigurationFiles.add(layout.projectDirectory.file("compose-stability.conf"))
}

dependencies {
    implementation(libs.androidx.core)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.webkit)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.exoplayer.hls)
    implementation(libs.media3.exoplayer.rtsp)
    implementation(libs.media3.effect)
    implementation(libs.media3.ui)
    implementation(libs.media3.session)
    implementation(libs.media3.datasource.okhttp)
    implementation(libs.androidx.tvprovider)
    implementation(libs.okhttp)
    implementation(libs.okhttp.sse)
    implementation(libs.coil.compose)
    implementation(libs.coil.network.okhttp)

    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
}
