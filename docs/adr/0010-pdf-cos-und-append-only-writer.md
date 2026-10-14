# ADR 0010 – pdf-cos und Append-only-Writer: Umsetzungsentscheidungen (Paket A)

| | |
|---|---|
| **Status** | angenommen (14.10.2026) |
| **Kontext** | Paket A: `crates/pdf-cos`, `crates/pdf-incremental` (ADR 0001 §4, §8; Plan §6.1–§6.4; analyse/03) |
| **Ergebnis** | alle 61 öffnbaren Korpus-PDFs: Increment strukturell korrekt, `qpdf --check` ohne neue Befunde, openssl-signiert und von pyHanko als intakt/gültig validiert, Vorsignaturen höchstens `FORM_FILLING` (`testdata/pdf-incremental/RESULTS.md`) |

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

## Additive API (Bericht an die Integration)

`pdf-cos`: `XrefHealth`, `SourcePdf::{xref_health, xref_offset_base, prev_offset, catalog, page_content,
pdfa_identification, doc_mdp, has_usage_rights, needs_rendering, is_tagged, acroform_info, decode_image,
decode_image_stream, real_literals, limits}`, `DocMdp`, `AcroFormInfo`, `PdfAIdentification`,
`parse_pdfa_identification`, `DecodedImage`, `PixelFormat`, `RealLiterals`, `PathSegment`,
`decode_text_string`, `encode_text_string`, `IDENTITY`, `matrix_multiply`, `Encryptor::{decrypt, algorithm}`.
`pdf-incremental`: `Increment::{byte_range_parts, byte_range_data, to_vec}`. Geänderte Semantik ohne
Signaturänderung: `SourcePdf::trailer()` liefert den Rohtrailer der letzten Revision (inkl. `/Encrypt`,
ergänzt um `/Root /Info /ID` älterer Trailer) statt lopdfs Trailer ohne `/Encrypt`;
`SignatureDictSpec` hat statt `#[derive(Debug)]` eine redigierende `Debug`-Implementierung (`/Name`
enthält Zertifikats-Personendaten, Regel 9); `Encryptor` und `DecodedImage` ebenso.

## Offene Punkte

- Reparatur-Increment für `XrefHealth::Reconstructed` (Plan §6.1 K2) – erst nach Korpus-Freigabe.
- JPX, JBIG2, CCITT werden nicht dekodiert (QR-Platzhalter sind Flate/DCT); Adobe-invertierte
  CMYK-JPEGs ohne `/Decode` erscheinen invertiert.
- Speicher: Original und Increment liegen vollständig im RAM (kein Memory-Map wegen `unsafe`-Verbot);
  `byte_range_parts` erlaubt gestreamtes Hashen.
- `/DocChecksum` linearisierter Dateien wird wie bei PDFBox in den Increment-Trailer übernommen.
