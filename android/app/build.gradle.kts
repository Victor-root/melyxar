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

    buildTypes {
        debug {
            if (signsWithOwnKey) {
                signingConfig = signingConfigs.getByName("own")
            }
        }
        release {
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
    implementation(libs.media3.effect)
    implementation(libs.media3.ui)

    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
}
