# ADR 0010 – pdf-cos und Append-only-Writer: Umsetzungsentscheidungen (Paket A)

| | |
|---|---|
| **Status** | angenommen (14.10.2026) |
| **Kontext** | Paket A: `crates/pdf-cos`, `crates/pdf-incremental` (ADR 0001 §4, §8; Plan §6.1–§6.4; analyse/03) |
| **Ergebnis** | alle 65 öffnbaren Korpus-PDFs: Increment strukturell korrekt, `qpdf --check` ohne neue Befunde, openssl-signiert und von pyHanko (API **und** CLI `sign validate --pretty-print --no-strict-syntax`) als intakt/gültig validiert, Vorsignaturen höchstens `FORM_FILLING` (`testdata/pdf-incremental/RESULTS.md`) |

## Entscheidungen

1. **Eigener COS-Lexer neben lopdf.** lopdf 0.45 exportiert seinen Parser nicht. `pdf-cos` hat einen
   lenienten Lexer (`lexer.rs`) für Trailer, xref-Tabellen/-Streams, Objekte an Offsets und
   Content-Streams; er erzeugt lopdf-Objekte. lopdf bleibt das Objektmodell (ADR 0001 §5.4).
2. **Eigene Querverweisanalyse, lopdf-Modell wird korrigiert.** Revisionen sind `%%EOF`-Marken mit
   gültigem `startxref` (die Dummy-Marke `startxref 0` linearisierter Dateien zählt nicht). Die Kette
   folgt `/Prev` und `/XRefStm`. lopdf 0.45 wertet `/XRefStm` älterer Revisionen nicht aus und mischt
   Hybrid-Abschnitte in falscher Reihenfolge; fehlende oder falsch zugeordnete Objekte werden nach der
   eigenen Tabelle nachgeladen (Container vor komprimierten Objekten). lopdf lädt nur bis zum Ende der
   letzten gültigen Revision (Leerraum > 512 Byte nach `%%EOF` stört so nicht).
3. **Zustand des Querverweises** (`XrefHealth`, additiv): `Intact`; `Repaired` (Offset-Korrektur im
   ±64-Byte-Fenster, Objekte per Kopfsuche – K2, normales Increment möglich, `/Prev` zeigt dann auf den
   *korrigierten* Abschnitt, `SourcePdf::prev_offset`); `Reconstructed` (kein verwertbares `startxref` –
   `IncrementBuilder::build` lehnt ab, ein Reparatur-Increment ist nicht umgesetzt).
4. **Verschlüsselung.** Schlüsselableitung (R2–R6) über lopdf mit leerem Benutzerpasswort. Handler werden
   vorab aus den Rohbytes bestimmt (PubSec/fremd → `UnsupportedSecurityHandler`). lopdf entschlüsselt
   `/Contents` von Signaturdictionaries fälschlich mit; `pdf-cos` setzt dort den Rohwert ein und
   entschlüsselt die übrigen Strings einzeln (ISO 32000-2 §7.6.1). `Encryptor` wählt Crypt-Filter selbst
   (fehlendes `/StmF`/`/StrF` bei V4/V5 = `/Identity`). lopdfs `EncryptionState` (leitet `Debug` mit
   Dateischlüssel ab) wird aus `document()` entfernt (Regel 9).
5. **Originalschreibweise von Reals.** lopdf speichert Reals als `f32`; ein fortgeschriebenes Seitenobjekt
   mit `/MediaBox [0 0 595.303937007874 …]` würde zu `595.303955`, pyHanko stuft das als `OTHER` ein
   (beobachtet an `POS_LAST_ALREADY_SIGNED`). `SourcePdf::real_literals` liefert den Originaltext je Pfad;
   der Writer verwendet ihn für unveränderte Werte (wie PDFBox `COSFloat`).
6. **Writer-Format** (analyse/03 Phase E): Increment beginnt mit CRLF; Objekte zeilenweise wie PDFBox;
   Signaturdictionary in PDF-AS-Reihenfolge, `/Contents` `0`-gefüllt (Hex, Großbuchstaben beim
   Einsetzen), `/ByteRange`-Platzhalter 36 Zeichen, nach dem Patchen linksbündig mit Leerzeichen.
   Tabelle immer mit Freieintrag 0; xref-Stream mit Eigeneintrag, minimalem `/W [1 n m]`, Flate ohne
   Predictor; Hybrid: zuerst xref-Stream (ohne `/Prev`), dann Tabelle mit `/XRefStm` und `/Prev`.
   Trailer: `/Size /Root /Info /Encrypt /ID /Prev`, übrige Schlüssel übernommen. `/ID[1]` = SHA-256
   (PDFBox 2.0.34). Neue Strings/Streams werden mit dem Handler des Dokuments verschlüsselt, außer
   `/Contents`, xref-Streams und `/Metadata` bei `EncryptMetadata false`; XMP wird nie komprimiert.
7. **Grenzen.** Dekompression je Filterstufe begrenzt auf `max_decoded_stream` und – oberhalb von
   16 MiB – auf `Eingabe × max_decompression_ratio` (Flate erreicht höchstens ≈ 1030:1; darunter wären
   gleichförmige Bilder fälschlich Bomben). Verschachtelung im Lexer ≤ 128, Seitenbaum/Feldbaum/Forms
   ≤ `max_depth`, Zyklen werden erkannt.
8. **Bild-`Do`-Aufrufe** wie PDF-AS (`q`/`Q`/`cm`/`Do`, Inline-Bilder übersprungen, Forms nur mit
   `descend_forms`); CTM im Seitenraum ab Einheitsmatrix. PDFium dient im Test als Orakel (alle
   Matrizen ≤ 0,01, ADR 0001 §8.2). `decode_image` dekodiert Flate/LZW/A85/AHx/RL/DCT,
   Gray/RGB/CMYK/ICC/Indexed/Lab/Separation/DeviceN, 1–16 Bit, `/Decode`, `/ImageMask`, `/SMask`.

9. **Rechte wie PDFBox.** PDFBox prüft beim Laden zuerst, ob das leere Passwort das **Owner**-Passwort
   ist, und gewährt dann Owner-Rechte unabhängig von `/P`; PDF-AS `checkPDFPermissions` lässt das
   Signieren dann zu. `pdf-cos` prüft das ebenso (lopdf `authenticate_raw_owner_password` auf einem
   Hilfsdokument mit dem rohen Encrypt-Dictionary): `Rights::permissions` ist dann
   `Permissions::ALL`, `EncryptionInfo::permissions` bleibt der Rohwert `/P`. Gegengeprüft mit
   PDFBox 2.0.34 an `testdata/pdf-cos/empty-owner-*.pdf` (RC4, AES-128, AES-256).
10. **Härtung gegen feindliche Eingaben** (Review, Mutations-Fuzzing über den Korpus,
    `crates/pdf-cos/tests/robustness.rs`):
    - Predictor-Parameter: lopdf reserviert die Zeile `Columns · Colors · Bits / 8` ungeprüft (und
      multipliziert ohne Überlaufprüfung) – `/Columns 4294967295 /Colors 255` ließ den Prozess
      Terabytes anfordern und vom System beenden. Zeilen über 16 MiB (`filters::MAX_PREDICTOR_ROW`)
      werden abgelehnt, bevor lopdf dekodiert: in `decode_stream`, bei xref-Streams (Ablehnung der
      Datei), bei Object-Stream-Containern der Querverweistabelle (Ablehnung, auch für
      verschlüsselte Dateien) und über den Ladefilter von lopdf (Streams werden verworfen).
    - Querverweis: Abschnitte werden je Position nur einmal gelesen (viele `%%EOF`-Marken auf
      dieselbe große Tabelle waren quadratisch), Zyklen über verschiedene deklarierte Offsets
      werden erkannt; die Suche nach `endstream` nutzt einen einmal aufgebauten Index; die Prüfung
      auf Objektköpfe liest keine Strings/Kommentare über Zeilen hinweg.
    - Seiteninhalt: Gesamtlänge ≤ `max_decoded_stream` (ein Array kann denselben Stream beliebig oft
      referenzieren); `image_invocations` begrenzt Bild-Aufrufe (100 000), Form-Aufrufe (100 000,
      Auffächerung „billion laughs“) und die Summe dekodierter Form-Inhalte.
    - Bilder: Pixelzahl muss zu `/Width`·`/Height` passen (sonst Fehler statt Indexfehler bei `/SMask`).
    - Writer: Objektnummern am Ende des `u32`-Raums (`/Size 4294967295`) → `IncError::Serialize`
      statt Überlauf; `/Contents`-Reserve ≤ `RESERVE_MAX` (32 MiB); von Hand gesetzte, inkonsistente
      `Increment`-Bereiche führen zu Fehlern statt Panics.

## Additive API (Bericht an die Integration)

`pdf-cos`: `XrefHealth`, `SourcePdf::{xref_health, xref_offset_base, prev_offset, catalog, page_content,
pdfa_identification, doc_mdp, has_usage_rights, needs_rendering, is_tagged, acroform_info, decode_image,
decode_image_stream, real_literals, limits}`, `DocMdp`, `AcroFormInfo`, `PdfAIdentification`,
`parse_pdfa_identification`, `DecodedImage`, `PixelFormat`, `RealLiterals`, `PathSegment`,
`decode_text_string`, `encode_text_string`, `IDENTITY`, `matrix_multiply`, `Encryptor::{decrypt, algorithm}`.
`pdf-incremental`: `Increment::{byte_range_parts, byte_range_data, to_vec}`, `RESERVE_MAX`. Geänderte Semantik ohne
Signaturänderung: `SourcePdf::trailer()` liefert den Rohtrailer der letzten Revision (inkl. `/Encrypt`,
ergänzt um `/Root /Info /ID` älterer Trailer) statt lopdfs Trailer ohne `/Encrypt`;
`SignatureDictSpec` hat statt `#[derive(Debug)]` eine redigierende `Debug`-Implementierung (`/Name`
enthält Zertifikats-Personendaten, Regel 9); `Encryptor` und `DecodedImage` ebenso.
`Rights::permissions` sind die **wirksamen** Rechte (Owner-Rechte bei leerem Owner-Passwort, siehe 9);
`image_invocations`/`page_content` liefern bei den neuen Grenzen `CosError::LimitExceeded`;
`IncrementBuilder::add_signature_dictionary` lehnt Reserven über `RESERVE_MAX` ab.

## Offene Punkte

- Reparatur-Increment für `XrefHealth::Reconstructed` (Plan §6.1 K2) – erst nach Korpus-Freigabe.
- JPX, JBIG2, CCITT werden nicht dekodiert (QR-Platzhalter sind Flate/DCT); Adobe-invertierte
  CMYK-JPEGs ohne `/Decode` erscheinen invertiert.
- Speicher: Original und Increment liegen vollständig im RAM (kein Memory-Map wegen `unsafe`-Verbot);
  `byte_range_parts` erlaubt gestreamtes Hashen.
- `/DocChecksum` linearisierter Dateien wird wie bei PDFBox in den Increment-Trailer übernommen.
- Bei rekonstruiertem Querverweis (K2, ohne eigenes Encrypt-Dictionary) gelten weiter die Rechte aus
  `/P`; die Owner-Prüfung (Entscheidung 9) setzt das rohe Encrypt-Dictionary voraus.
- Bilddekodierung: Speicherbedarf bis etwa 3 × `Breite·Höhe·4` Byte innerhalb von
  `max_decoded_stream`; mobile Aufrufer sollten kleinere `Limits` übergeben.
- Mutations-Fuzzing läuft in der CI nur kurz (6 Mutationen je Datei); längere Läufe mit
  `PIDA_FUZZ_ITERS=<n>` (Review: 455 000 Mutationen in zwei Läufen ohne Panic/Abbruch, Release-Build).
