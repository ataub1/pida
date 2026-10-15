// PDF-Over NG – sicherer Speicher der Android-Seite (EUPL-1.2), Plan §7.7.
//
// Werte (Keystore-Passwörter u. Ä.; das ID-Austria-Passwort erreicht diesen Speicher nie) liegen
// AES-256-GCM-verschlüsselt in privaten SharedPreferences. Der Schlüssel entsteht im Android Keystore
// (StrongBox, falls vorhanden, sonst TEE), ist nicht exportierbar und gerätegebunden; die App-Daten sind
// von Sicherung und Gerätewechsel ausgenommen (Manifest der App). Eintragsname = SHA-256 des Schlüssels,
// der Schlüssel ist zusätzlich AAD (ein Wert lässt sich nicht unter einen anderen Schlüssel kopieren).
// Unlesbare Einträge (Schlüssel verloren, manipuliert) gelten als nicht vorhanden und werden entfernt.
//
// Keine Werte, Schlüssel oder Ausnahmetexte in Logs.

package at.pdfoverng.mobile

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import android.util.Base64
import android.util.Log
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Fehler des sicheren Speichers mit kurzem Code (für `invoke.reject`). */
class SecureStoreException(val code: String) : Exception(code)

class SecureStore(context: Context) {
    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    @Synchronized
    fun get(key: String): String? {
        val entry = entryName(key)
        val stored = prefs.getString(entry, null) ?: return null
        val plain = try {
            decrypt(key, stored)
        } catch (e: Exception) {
            Log.w(TAG, "Eintrag nicht entschlüsselbar (${e.javaClass.simpleName}) – entfernt")
            prefs.edit().remove(entry).apply()
            return null
        }
        return plain
    }

    @Synchronized
    fun set(key: String, value: String) {
        val sealed = try {
            encrypt(key, value)
        } catch (e: Exception) {
            Log.w(TAG, "Verschlüsseln fehlgeschlagen (${e.javaClass.simpleName})")
            throw SecureStoreException("keystore-failed")
        }
        if (!prefs.edit().putString(entryName(key), sealed).commit()) {
            throw SecureStoreException("store-write-failed")
        }
    }

    @Synchronized
    fun delete(key: String) {
        if (!prefs.edit().remove(entryName(key)).commit()) {
            throw SecureStoreException("store-write-failed")
        }
    }

    private fun encrypt(key: String, value: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        cipher.updateAAD(key.toByteArray(StandardCharsets.UTF_8))
        val plain = value.toByteArray(StandardCharsets.UTF_8)
        try {
            val sealed = cipher.doFinal(plain)
            val iv = cipher.iv
            val out = ByteArray(1 + iv.size + sealed.size)
            out[0] = iv.size.toByte()
            System.arraycopy(iv, 0, out, 1, iv.size)
            System.arraycopy(sealed, 0, out, 1 + iv.size, sealed.size)
            return VERSION + Base64.encodeToString(out, Base64.NO_WRAP)
        } finally {
            plain.fill(0)
        }
    }

    private fun decrypt(key: String, stored: String): String {
        require(stored.startsWith(VERSION)) { "version" }
        val raw = Base64.decode(stored.substring(VERSION.length), Base64.NO_WRAP)
        val ivLen = raw[0].toInt()
        require(ivLen in 12..16 && raw.size > 1 + ivLen + GCM_TAG_BITS / 8) { "format" }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            existingKey() ?: throw IllegalStateException("no-key"),
            GCMParameterSpec(GCM_TAG_BITS, raw, 1, ivLen),
        )
        cipher.updateAAD(key.toByteArray(StandardCharsets.UTF_8))
        val plain = cipher.doFinal(raw, 1 + ivLen, raw.size - 1 - ivLen)
        try {
            return String(plain, StandardCharsets.UTF_8)
        } finally {
            plain.fill(0)
        }
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    private fun existingKey(): SecretKey? = (keyStore().getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.secretKey

    private fun secretKey(): SecretKey = existingKey() ?: generateKey()

    private fun generateKey(): SecretKey {
        fun spec(strongBox: Boolean): KeyGenParameterSpec {
            val b = KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .setRandomizedEncryptionRequired(true)
            if (strongBox && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                b.setIsStrongBoxBacked(true)
            }
            return b.build()
        }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        return try {
            generator.init(spec(strongBox = true))
            generator.generateKey()
        } catch (e: StrongBoxUnavailableException) {
            generator.init(spec(strongBox = false))
            generator.generateKey()
        }
    }

    companion object {
        private const val TAG = "PdfOverNg/SecureStore"
        private const val PREFS = "pdfover_secure_store"
        private const val KEY_ALIAS = "pdfover-ng-secure-store-v1"
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val VERSION = "v1:"

        /** Name des Eintrags: SHA-256 (hex) des Schlüssels – die Schlüsselnamen stehen nicht im Klartext. */
        fun entryName(key: String): String {
            val digest = MessageDigest.getInstance("SHA-256").digest(key.toByteArray(StandardCharsets.UTF_8))
            return digest.joinToString("") { "%02x".format(it) }
        }
    }
}
