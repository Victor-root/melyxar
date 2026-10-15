// Every plugin is declared here once, so the whole build resolves them on one
// classpath: the Kotlin the Compose plugin brings is then the one the Android
// plugin compiles with.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}
