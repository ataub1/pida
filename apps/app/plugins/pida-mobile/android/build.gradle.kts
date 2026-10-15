// PIDA – native Android-Seite des Plugins `pida-mobile` (EUPL-1.2), Paket M-Android.
// Die Tauri-API (`.tauri/tauri-api`) kopiert das Build-Skript von `tauri-plugin` hierher.
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// Kotlin-Komponente von rustls-platform-verifier: Version muss **genau** der des Crates
// `rustls-platform-verifier-android` in Cargo.lock entsprechen (src/tls.rs). Abweichung → Build-Fehler.
val rustlsPlatformVerifierVersion = "0.2.0"
run {
    val lock = projectDir.resolve("../../../../../Cargo.lock")
    if (lock.isFile) {
        val lines = lock.readLines()
        val idx = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
        val locked = lines.drop(idx + 1).firstOrNull { it.trimStart().startsWith("version = ") }
            ?.substringAfter('"')?.substringBefore('"')
        if (idx >= 0 && locked != rustlsPlatformVerifierVersion) {
            throw GradleException(
                "rustls-platform-verifier-android ist in Cargo.lock $locked, Gradle nutzt $rustlsPlatformVerifierVersion – " +
                    "apps/app/plugins/pida-mobile/android/build.gradle.kts anpassen"
            )
        }
    }
}

android {
    namespace = "at.pida.mobile"
    compileSdk = 36

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
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation(project(":tauri-android"))
    // TLS-Prüfung über den Android-Truststore (Maven-Archiv von rustls-platform-verifier, Repository im
    // Wurzelprojekt gen/android/build.gradle.kts); Proguard-Regel in consumer-rules.pro.
    implementation("org.rustls:rustls-platform-verifier:$rustlsPlatformVerifierVersion")
    testImplementation("junit:junit:4.13.2")
}
