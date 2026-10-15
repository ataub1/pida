# PIDA – Proguard-Regeln, die mit dem Plugin in die App gehen.
# rustls-platform-verifier ruft seine Kotlin-Komponente nur über JNI auf (für Proguard toter Code).
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
# Plugin-Klasse wird über Reflexion (Tauri PluginManager) geladen.
-keep class at.pida.mobile.PidaMobilePlugin { *; }
