// PDF-Over NG – native Android-Seite des Plugins `pdfover-mobile` (EUPL-1.2).
// Die Tauri-API (`.tauri/tauri-api`) kopiert das Build-Skript von `tauri-plugin` hierher.
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "at.pdfoverng.mobile"
    compileSdk = 35

    defaultConfig {
        // Plan §5.1: Android 9 (API 28)
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.9.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation(project(":tauri-android"))
    // M-Android: Kotlin-Komponente von rustls-platform-verifier (TLS-Prüfung über den Android-Truststore,
    // src/tls.rs). Version = `rustls-platform-verifier-android` in Cargo.lock; Repository siehe
    // docs/integration/mobile.md.
    // implementation("rustls:rustls-platform-verifier:0.2.0")
}
