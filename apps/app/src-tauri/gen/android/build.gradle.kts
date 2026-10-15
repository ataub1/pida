// PIDA – Android-Projekt (EUPL-1.2), erzeugt mit `tauri android init`; siehe
// docs/integration/mobile-android.md.
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:9.3.1")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.2.10")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
        // Kotlin-Komponente von rustls-platform-verifier (TLS über den Android-Truststore, Plugin
        // pida-mobile src/tls.rs). Offizielles Maven-Archiv des Projekts; nur die Gruppe `org.rustls`.
        maven {
            url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
            content { includeGroup("org.rustls") }
        }
    }
}

tasks.register("clean").configure {
    delete("build")
}
