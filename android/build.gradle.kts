// The plugins' own libraries written down as well, in
// buildscript-gradle.lockfile, beside the application's (see app), and as
// leniently: a build never stops on them.
buildscript {
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
    }
    dependencyLocking {
        lockMode.set(LockMode.LENIENT)
    }
}

// Every plugin is declared here once, so the whole build resolves them on one
// classpath: the Kotlin the Compose plugin brings is then the one the Android
// plugin compiles with.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}
