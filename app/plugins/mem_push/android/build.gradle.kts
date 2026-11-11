group = "dev.meshm.mem_push"
version = "0.1.0"

buildscript {
    repositories {
        google()
        mavenCentral()
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    id("com.android.library")
}

val agpMajor = com.android.Version.ANDROID_GRADLE_PLUGIN_VERSION.substringBefore('.').toInt()

// AGP 9 provides Kotlin itself unless the app opts out with android.builtInKotlin=false, as
// MEM's app does (the Flutter template default).
val builtInKotlinEnabled =
    agpMajor >= 9 &&
        (providers.gradleProperty("android.builtInKotlin").orNull?.toBoolean() ?: true)

if (!builtInKotlinEnabled) {
    apply(plugin = "org.jetbrains.kotlin.android")
}

project.extensions.configure(org.jetbrains.kotlin.gradle.dsl.KotlinAndroidProjectExtension::class.java) {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

android {
    namespace = "dev.meshm.mem_push"
    compileSdk = flutter.compileSdkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        minSdk = 24
    }


    dependencies {
        // UnifiedPush connector (Apache-2.0) and WorkManager (Apache-2.0).
        implementation("org.unifiedpush.android:connector:3.3.5")
        implementation("androidx.work:work-runtime:2.11.2")
        implementation("androidx.core:core-ktx:1.16.0")
    }
}
