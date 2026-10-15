//! Android: `rustls-platform-verifier` mit dem JNI-Kontext der App initialisieren (Plan §5.1
//! „TLS-Truststore: OS“, §7.7).
//!
//! Unter Android prüft `rustls-platform-verifier` Zertifikate über die Java-Klassen der Plattform
//! (`X509TrustManager`, Behörden-CAs aus dem Nutzer-Truststore). Dafür braucht es einmal je Prozess die
//! `JavaVM`, den Application-Context und dessen Class-Loader – ohne Initialisierung bricht die erste
//! TLS-Verbindung (A-Trust) mit einer Panik ab. tao legt beim Erzeugen der Activity beides in
//! `ndk-context` ab (`ndk_glue::create`: `JavaVM` und eine **globale** Referenz auf den
//! Application-Context, die bis zum Prozessende lebt); daraus initialisiert [`init_platform_verifier`]
//! den Verifier.
//!
//! Zusätzlich muss die Kotlin-Komponente `org.rustls.platformverifier` im APK sein (Aufgabe M-Android,
//! docs/integration/mobile.md): Maven-Repository
//! `https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/`,
//! Abhängigkeit `org.rustls:rustls-platform-verifier` in **genau** der Version des Crates
//! `rustls-platform-verifier-android` aus `Cargo.lock` (derzeit 0.2.0); die Proguard-Regel steht schon in
//! `android/consumer-rules.pro`. Auf anderen Plattformen ist [`init_platform_verifier`] ein No-op.

#[cfg(target_os = "android")]
use crate::Error;
use crate::Result;

/// Initialisiert den TLS-Verifier der Plattform (nur Android; sonst sofort `Ok`).
#[cfg(not(target_os = "android"))]
pub fn init_platform_verifier() -> Result<()> {
    Ok(())
}

/// Initialisiert den TLS-Verifier der Plattform (nur Android; sonst sofort `Ok`). Idempotent.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub fn init_platform_verifier() -> Result<()> {
    // `android_context()` bricht ab, wenn tao den Kontext (noch) nicht gesetzt hat
    let ctx = std::panic::catch_unwind(ndk_context::android_context)
        .map_err(|_| Error::Native("no-android-context".into()))?;
    if ctx.vm().is_null() || ctx.context().is_null() {
        return Err(Error::Native("no-android-context".into()));
    }
    // SAFETY: Der Zeiger stammt von tao (`JNIEnv::get_java_vm` der laufenden VM), ist nicht null und
    // bleibt für die Lebensdauer des Prozesses gültig; Android-VMs unterstützen JNI ≥ 1.6.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> std::result::Result<(), jni::errors::Error> {
        // SAFETY: `ctx.context()` ist eine globale Referenz (tao hält sie in `APPLICATION_CONTEXT` bis
        // zum Prozessende); globale Referenzen gelten in jedem Thread. Der Wrapper gibt sie nicht frei
        // (`JObject` hat kein `Drop`), `init_with_env` legt eine eigene globale Referenz an.
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|_| Error::Native("tls-init-failed".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_android_init_is_a_no_op() {
        if !cfg!(target_os = "android") {
            assert!(super::init_platform_verifier().is_ok());
        }
    }
}
