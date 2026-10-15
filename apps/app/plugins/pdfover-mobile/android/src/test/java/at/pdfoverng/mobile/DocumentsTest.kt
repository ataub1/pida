// PDF-Over NG – JVM-Unit-Tests der reinen Logik der Android-Seite (ohne Gerät):
//   cd apps/app/src-tauri/gen/android && ./gradlew :tauri-plugin-pdfover-mobile:testDebugUnitTest
package at.pdfoverng.mobile

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentsTest {
    @Test
    fun namesNeverLeaveTheFolder() {
        // wie `safe_name` der Rust-Shell (apps/app/src-tauri/src/mobile.rs)
        assertEquals("_x.pdf", Documents.safeName("../x.pdf"))
        assertEquals("a_b.pdf", Documents.safeName("a/b.pdf"))
        assertEquals("a_b_c", Documents.safeName("a\\b:c"))
        assertEquals("Dokument.pdf", Documents.safeName(""))
        assertEquals("Dokument.pdf", Documents.safeName(null))
        assertEquals("Datei", Documents.safeName("...", "Datei"))
        assertEquals("x_y.pdf", Documents.safeName("x\u0000y.pdf"))
        assertEquals("Vertrag 1.pdf", Documents.safeName(" Vertrag 1.pdf "))
    }

    @Test
    fun pdfExtensionIsAddedOnlyWhenMissing() {
        assertEquals("document.pdf", Documents.withPdfExtension("document"))
        assertEquals("A.PDF", Documents.withPdfExtension("A.PDF"))
    }

    @Test
    fun uniqueTargetsKeepTheNameInOwnFolders() {
        val dir = Files.createTempDirectory("pdfover-docs").toFile()
        try {
            val first = Documents.uniqueTarget(dir, "Vertrag.pdf")
            assertEquals(File(File(dir, "1"), "Vertrag.pdf"), first)
            first.parentFile!!.mkdirs()
            first.writeText("x")
            val second = Documents.uniqueTarget(dir, "Vertrag.pdf")
            assertEquals(File(File(dir, "2"), "Vertrag.pdf"), second)
            assertTrue(Documents.within(dir, second))
            assertFalse(Documents.within(dir, File(dir, "../fremd.pdf")))
            assertFalse("der Ordner selbst ist keine Datei darin", Documents.within(dir, dir))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test
    fun intentUrisArriveExactlyOnce() {
        val u = "content://com.android.providers.downloads.documents/document/42"
        // Kaltstart: Intent vorgemerkt, danach Start-Abfrage und RunEvent::Opened mit derselben URI
        val p = PendingDocuments()
        p.remember(listOf(u))
        assertEquals(listOf(u), p.take(emptyList()))
        assertEquals(emptyList<String>(), p.take(listOf(u)))
        // andere Reihenfolge: Opened zuerst
        val q = PendingDocuments()
        q.remember(listOf(u))
        assertEquals(listOf(u), q.take(listOf(u)))
        assertEquals(emptyList<String>(), q.take(emptyList()))
        // unbekannte übergebene URI (nur tao hat den Intent gesehen) → einmal
        val r = PendingDocuments()
        assertEquals(listOf(u), r.take(listOf(u, u)))
        assertEquals(emptyList<String>(), r.take(listOf(u)))
        // dieselbe Datei später erneut geteilt (neuer Intent) → erneut
        r.remember(listOf(u))
        assertEquals(listOf(u), r.take(listOf(u)))
    }

    @Test
    fun seenSetIsBounded() {
        val p = PendingDocuments(maxSeen = 2)
        p.remember(listOf("content://a/1", "content://a/2", "content://a/3"))
        assertEquals(3, p.take(emptyList()).size)
        // die älteste URI ist vergessen und käme als übergebene URI erneut an
        assertEquals(listOf("content://a/1"), p.take(listOf("content://a/1", "content://a/3")))
    }
}
