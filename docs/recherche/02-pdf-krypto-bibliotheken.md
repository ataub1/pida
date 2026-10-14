# 02 – PDF- und Krypto-Bibliotheken für einen eigenen PAdES-Kern (PDF-AS-Ersatz)

Stand der Recherche: **2026-10-14** (Paketregister und Repositories an diesem Tag abgefragt).
Referenz-Code (nur gelesen):

| Präfix | Bedeutung (relativ zu `scratchpad/ref/`) |
|---|---|
| `PDFOVER:` | `PDF-Over/` – PDF-Over `main` @ `c07c5ec`, 4.4.10-SNAPSHOT |
| `PDFAS:` | `pdf-as-4/` – PDF-AS 4.4.2 |
| `CFG:` | `cfgzip/` – von PDF-Over mitgelieferte PDF-AS-Konfiguration |

## Legende für den Verifikationsgrad

| Tag | Bedeutung |
|---|---|
| **[C]** | Im Originalcode (PDF-Over / PDF-AS / Config) verifiziert, Pfad:Zeile angegeben |
| **[W]** | Offizielle Quelle am 2026-10-14 geprüft: Paketregister-API (crates.io, npm, NuGet, pub.dev, deps.dev), Repository-Datei, offizielle Doku |
| **[S]** | Sekundärquelle bzw. Suchmaschinen-Zusammenfassung offizieller Doku, die nicht im Volltext gelesen wurde |
| **[I]** | Eigene Interpretation, Bewertung, Vermutung |
| **[E]** | Erfahrungswissen aus der Zeit vor dieser Recherche, **nicht** am Stichtag nachgeprüft |

Versions- und Datumsangaben in den Tabellen sind **[W]** (Registry-Metadaten), außer anders markiert.
„EUPL ok" heißt: Lizenz ist permissiv oder steht auf der EUPL-1.2-Kompatibilitätsliste und erzwingt **keinen** Lizenzwechsel des Gesamtwerks (Details Abschnitt 3).

---

## 0. Kurzfazit

1. **Rust ist der einzige Stack, in dem alle Bausteine (a)–(g) als permissiv lizenzierte, reine Sprach-Crates vorliegen, die für Desktop, iOS, Android und `wasm32` kompilieren** [W]/[I]:
   `lopdf` 0.45 (MIT) mit `IncrementalDocument`, xref-Streams und inkrementellem Speichern verschlüsselter Dokumente;
   RustCrypto `cms` (Apache-2.0/MIT) mit externem Message-Digest, frei setzbaren Signed Attributes und (ab 0.3-rc) asynchronen Signern;
   `x509-cert`, `der`, `spki`, `p12-keystore`, `rsa`, `p256`/`p384`, `x509-tsp`/`x509-ocsp`;
   Fonts über `skrifa` + `subsetter`; QR über `rqrr`/`rxing`; Bild-Filter über `zune-jpeg` + `hayro-jpeg2000`/`-jbig2`/`-ccitt`;
   Vorschau über `pdfium-render` (PDFium-Binärdateien für alle Plattformen inkl. WASM) oder den reinen Rust-Renderer `hayro`.
   Es gibt bereits Rust-Referenzimplementierungen für PAdES (`underskrift`, BSD-2-Clause, B-B bis B-LTA, Remote-Signing in drei Phasen),
   allerdings alle **jünger als ein Jahr** → als Vorlage studieren, nicht blind als Kernabhängigkeit übernehmen [I].
2. **PDF-Over braucht zwei Arten von externem Signieren, und kaum eine Bibliothek kann beide** [C]/[I]:
   - **Modell A „Raw-Signatur"**: Die Bibliothek baut das CMS und die SignedAttributes, ein externer Signer signiert nur den Hash
     (Keystore, PKCS#11, Plattform-Keychain). Entspricht `PDFAS:signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESSignerKeystore.java:158-182,308-366`.
   - **Modell B „CMS-Container"**: Die Bibliothek liefert die Bytes des ByteRange, die Gegenstelle gibt ein **vollständiges CMS SignedData** zurück,
     das nur noch in `/Contents` eingesetzt wird. Das ist der Weg über Security Layer 1.2 (BKU lokal, A-Trust/ID Austria):
     PDF-AS schickt das **ganze PDF** mit geschwärztem `/Contents` plus `ExcludedByteRange` (`PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/sl/util/BaseSLConnector.java:97-136`)
     und prüft das zurückkommende CMS danach selbst (`PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/util/SignatureUtils.java:26-80`).
   Modell B können u. a. `@signpdf` (JS), PoDoFo (C++), PDFsharp (.NET, aber inzwischen AGPL), `oxidize-pdf` (Rust) und Nutrient (kommerziell).
   Modell A können `underskrift`, `digitorus/pdfsign`, `@libpdf/core`, Syncfusion, Apryse, RustCrypto `cms`.
   In einem eigenen Kern auf Basis von `lopdf` sind beide Modelle wenig zusätzliche Arbeit (Platzhalter + ByteRange + Hex-Einsetzen) [I].
3. **Lizenz-Überraschung am Stichtag:** **PDFsharp 6.5.0 (veröffentlicht 2026-10-14) und 7.0.0-preview-2 stehen unter AGPL-3.0 oder kommerzieller Lizenz.**
   Die letzte MIT-Version ist 6.2.4 vom 2026-01-06 [W]. Außerdem setzt PDFsharp `/SubFilter /adbe.pkcs7.detached` fest im Code und hat keinen Incremental-Update-Pfad [W]/[I].
   Damit fehlt dem .NET-Ökosystem eine permissive, gepflegte PDF-Schreibbibliothek mit Incremental Update.
4. **Zweitbeste Pfade:**
   - **Go:** `digitorus/pdfsign` v0.9.0 (BSD-2-Clause) ist der reifste quelloffene PAdES-Signer außerhalb von Java, Python und C++
     (B-B/B-T, `crypto.Signer`, CSC- und PKCS#11-Signer, sichtbare Signatur, Prüfung). Mobile über gomobile; WASM ist schwergewichtig [W]/[E].
   - **TypeScript:** `@libpdf/core` 0.5.2 (MIT, Beta, von Documenso) kann PAdES B-B bis B-LTA, Incremental Saves und Verschlüsselung im Browser.
     Es fehlen aber sichtbare Signatur-Appearances und eine Schnittstelle für fertige CMS-Container [W].
   - **C++:** PoDoFo 1.1.2 (LGPL-2.0-or-later **oder** MPL-2.0) kann fachlich alles Nötige (Incremental Update, PAdES-B, Deferred Signing),
     verlangt aber eine C++-Toolchain auf fünf Plattformen plus WASM und bringt Speichersicherheitsrisiken beim Parsen nicht vertrauenswürdiger PDFs [W]/[I].
5. **Dart/Flutter** hat ordentliche Krypto-Primitive (`pointycastle`, `asn1lib`), aber **kein reifes CMS und keinen reifen PDF-Editor** mit Incremental Update.
   Die neuen reinen Dart-Stacks (`pdf_cos`/`pdf_document`, `nkpdfsign`, `dpdf`) sind alle aus 2026 und kaum verbreitet; Syncfusion ist kommerziell.
   → Flutter nur als UI über einen Rust-Kern (`flutter_rust_bridge`) [I]. Das passt zur Empfehlung in `docs/recherche/01-cross-platform-frameworks.md` (Abschnitt 0).
6. **Nicht verwenden** (AGPL/GPL bzw. proprietär → Gesamtwerk müsste AGPL/GPL werden oder ist nicht frei verteilbar):
   MuPDF/`mupdf.js`/`go-fitz`/`mupdf`-Crate (AGPL), iText (AGPL), PDFsharp ≥ 6.5 (AGPL), `pdfluent` (AGPL/kommerziell), `pdf_signer`-Crate (GPL-3.0+),
   `pdfium`-Crate von newinnovations (GPL-3.0), unipdf (EULA), Syncfusion/Apryse/Nutrient (kommerziell) [W]/[I].
7. **Test-Orakel** für die Neuimplementierung: pyHanko 0.37.0 (MIT) und EU DSS 6.5 (LGPL-2.1) zur Validierung; beide werden nur im Test eingesetzt, nicht ausgeliefert [W]/[I].

---

## 1. Ziel, Umfang, Methode

- **Frage:** Welche Bibliotheken je Ökosystem tragen einen **selbst implementierten** PAdES-Kern (Ersatz für PDF-AS 4 + PDFBox 2 + IAIK-CMS),
  der auf Windows, macOS, Linux, iOS, Android und möglichst im Web (WASM) läuft?
- **Ökosysteme:** Rust, Dart/Flutter, TypeScript/JavaScript, C#/.NET, C/C++, Go, kurz Kotlin Multiplatform; zusätzlich kommerzielle SDKs und Testwerkzeuge.
- **Kategorien:** (a) PDF lesen + Incremental Update, (b) fertige PDF-Signatur, (c) CMS/CAdES/X.509/ASN.1, (d) Rendering,
  (e) Text/Font-Layout, (f) Bilder/EXIF/QR, (g) HTTP mit Cookies und Proxy.
- **Methode:** Metadaten per Registry-API abgefragt (crates.io, registry.npmjs.org + api.npmjs.org, NuGet-Suche + Katalog, pub.dev, api.deps.dev für Go/Maven/PyPI),
  READMEs/Quelltexte aus den offiziellen Repositories gelesen (raw.githubusercontent.com, pdfium.googlesource.com), einzelne Doku-Seiten per WebFetch.
  Wo GitHub-API-Limits griffen, kamen Release-Daten aus den Release-Seiten.
  Downloadzahlen dienen nur als grober Reifeindikator.
- **Nicht** gemacht: keine Builds, keine Funktionstests, keine Messung von Binärgrößen.

---

## 2. Anforderungen aus PDF-AS/PDF-Over, die die Bibliothekswahl bestimmen

### 2.1 PDF-Mechanik der Signatur

| # | Anforderung | Fundstelle | Konsequenz für die Bibliothek |
|---|---|---|---|
| P1 | Signatur per **Incremental Update**: PDFBox `doc.addSignature(...)`, danach `doc.saveIncremental(bos)` | `PDFAS:pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/PADESPDFBOXSigner.java:295,394` [C] | Original-Bytes dürfen nicht neu geschrieben werden, sonst werden frühere Signaturen ungültig |
| P2 | `/Filter`, `/SubFilter`, `/M` (SignDate), `/Name`, `/Reason`, ggf. `/Location` (Platzhalter-ID) setzen | `PADESPDFBOXSigner.java:155-157,167,189,194` [C] | Freier Zugriff auf das Signatur-Dictionary |
| P3 | SubFilter **`ETSI.CAdES.detached`** (PAdES) oder **`adbe.pkcs7.detached`** | `PDFAS:signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESConstants.java:4`, `PAdESSigner.java:57`, `PDFAS:signature-standards/sigs-pkcs7detached/src/main/java/at/gv/egiz/pdfas/sigs/pkcs7detached/PKCS7DetachedConstants.java:4` [C] | SubFilter muss frei wählbar sein (PDFsharp scheidet hier aus, s. 4.4) |
| P4 | Platzgröße für `/Contents`: Standard **0x1000 Bytes**, über Profil-Key `signatureSize` konfigurierbar | `PADESPDFBOXSigner.java:206,582-594`; `PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/api/IConfigurationConstants.java:114` [C] | Größe des Platzhalters muss vorgegeben werden können. Für B-T/B-LT reichen 4 KiB vermutlich nicht [I] |
| P5 | Ein vorhandenes **leeres Signaturfeld** wird wiederverwendet, sonst wird ein neues angelegt und benannt | `PADESPDFBOXSigner.java:144-151,297-339` [C] | AcroForm-Felder lesen und schreiben |
| P6 | Verschlüsselte PDFs werden signiert, **wenn die Rechte es erlauben** (`canModify && canModifyAnnotations` oder `canFillInForm`) | `PDFAS:pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/utils/PdfBoxUtils.java:17-37` [C] | Entschlüsseln, Rechte auswerten **und** neue Objekte im Update korrekt verschlüsseln (außer `/Contents`) |
| P7 | PDF/A: bei Bedarf OutputIntent (sRGB-ICC) ergänzen und das Ergebnis mit PDFBox **Preflight** prüfen | `PADESPDFBOXSigner.java:199-202,268-276,398-401,669` [C] | Außerhalb von Java gibt es keinen Preflight-Ersatz außer veraPDF (Java) → offene Frage |
| P8 | PDF/UA: Struktur-Inhalt für das Signaturfeld einfügen | `PADESPDFBOXSigner.java:350,427` [C] | StructTree auf Objektebene bearbeiten können |
| P9 | xref-Streams und Object Streams in Eingabedateien (seit PDF 1.5 üblich) | [I] aus PDFBox-Verhalten | Lesen **und** Update im passenden Format schreiben |

### 2.2 Kryptografie (CMS/CAdES)

| # | Anforderung | Fundstelle |
|---|---|---|
| K1 | Keystore-Pfad: CMS **SignedData, detached**, ein SignerInfo; SignedAttributes `contentType`=`id-data`, `signingCertificate` (bei SHA-1) bzw. **`signingCertificateV2`** (ESSCertIDv2), `signingTime`, optional `mime-type` | `PAdESSignerKeystore.java:158-182,298-366` [C] |
| K2 | Algorithmen: RSA → `sha256WithRSAEncryption`; EC → Feldgröße ≥ 512 Bit: ECDSA-SHA512, ≥ 256: **ECDSA-SHA256** (also auch P-384 mit SHA-256!), sonst SHA-1; DSA → SHA-256 | `PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/util/CertificateUtils.java:36-75` [C] |
| K3 | Nach **jedem** Signiervorgang wird das CMS geprüft: genau ein SignerInfo, Signatur gültig, `signingCertificate(V2)` passt zum Signer-Zertifikat | `SignatureUtils.java:26-80`; Aufrufer `PAdESSignerKeystore.java:192`, `PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/sl/util/ISignatureConnectorSLWrapper.java:102`, `PDFAS:pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/PdfAsImpl.java:434` [C] |
| K4 | **Security-Layer-Pfad (BKU/A-Trust):** `CreateCMSSignatureRequest`, `Structure=detached`, Content = komplettes PDF mit geschwärzter Signatur **oder** Referenz `formdata:fileupload`, dazu `ExcludedByteRange` und optional `PAdESFlag` | `BaseSLConnector.java:97-154` [C]; PDF-Over-Seite `PDFOVER:pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4BKUSLConnector.java:107-119` [C] |
| K5 | Keystores: **PKCS#12** und **„JKS"**, wobei `*.jks`/`*.ks` intern als Typ **`JCEKS`** geöffnet werden | `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/KeystoreConfigurationComposite.java:187,344-350,545-546`; `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/keystore/KeystoreUtils.java:14` [C] |
| K6 | IAIK-CMS (proprietär) als heutige CMS-Engine | `PDFAS:pdf-as-lib/build.gradle:80` [C] |

### 2.3 Visuelle Signatur, Fonts, Bilder, Platzhalter, Vorschau, HTTP

| # | Anforderung | Fundstelle |
|---|---|---|
| V1 | Der Signaturblock wird als eigenes PDF-Template gebaut und als Appearance übernommen (PDFBox `PDFTemplateCreator`) | `PDFAS:pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFAsTemplateCreator.java:47,71,103` [C] |
| V2 | Fonts: Standard-14 (`HELVETICA`, `COURIER`) **und** eingebettete TrueType-Fonts (`TTF:DejaVuSansMono.ttf`, `TTF:DejaVuSansCondensed-Bold.ttf`), PDF/A-Profile nur mit TTF | `CFG:cfg/profiles/BASE_SMALL.properties:12-13`, `CFG:cfg/profiles/SIGNATURBLOCK_SMALL_DE_PDFA.properties:14-15` [C] |
| V3 | TTF-Einbettung als `PDType0Font` (CID/Identity-H, bei PDFBox standardmäßig als Subset) | `PDFAS:pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFBoxFont.java:192` [C]; Subset-Default [E] |
| V4 | Mitgelieferte Fonts: DejaVu (3×) **und `arial.ttf`** | `CFG:fonts/` (Verzeichnisinhalt) [C] → Weitergabe von Arial ist lizenzrechtlich fraglich (offene Frage) |
| B1 | Logo-Bilder: EXIF-Orientierung wird ausgewertet und das Bild gedreht/gespiegelt (metadata-extractor) | `PDFOVER:pdf-over-commons/src/main/java/at/asit/pdfover/commons/utils/ImageUtil.java:35-39,77-83,150` [C] |
| Q1 | **QR-Platzhalter**: Bild-XObjects der Seiten werden voll dekodiert (`PDImageXObject.getImage()`) und mit ZXing (`HybridBinarizer`, nur `QR_CODE`) gelesen | `PDFAS:pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/placeholder/SignaturePlaceholderExtractor.java:192-194,423-454` [C] → alle PDF-Bildfilter (DCT, Flate+Predictor, JPX, JBIG2, CCITT) und Farbräume dekodieren können. PDF-AS bindet dafür JAI-JPEG2000 und levigo-JBIG2 ein (`PDFAS:pdf-as-lib/build.gradle`) [C] |
| Q2 | Leere Signaturfelder als Platzhalter (Alternative zum QR-Code) | `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/OpenState.java:127-138` [C] |
| R1 | Vorschau/Positionierung: PDFBox `PDFRenderer.renderImage(page, scale)` | `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/SignaturePanel.java:153,283` [C] |
| H1 | HTTP: Apache HttpClient 5 mit `useSystemProperties()` (System-Proxy), manuellem Redirect-Handling, Cookies (Standard-Cookie-Store), HTML-Parsing mit Jsoup | `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/utils/HttpClientUtils.java:7-9`, `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/MobileBKUConnector.java:74,120,148,178-193` [C] |
| H2 | Proxy-Host/-Port/-User/-Passwort konfigurierbar (auch per CLI) | `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/config/ConfigurationManager.java:136-138,527-553`; `PDFOVER:pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/AdvancedConfigurationComposite.java:580-610` [C] |
| H3 | FIDO2-Assertion für A-Trust wird im Client erzeugt und base64-kodiert per JSON gesendet | `MobileBKUConnector.java:426-440` [C] → Thema von Recherche 01, hier nur HTTP-Transport |

### 2.4 Abgeleitete Prüfkriterien für jede PDF-Bibliothek

**IU** Incremental Update ohne Neuschreiben · **XS** xref-Streams lesen und Update schreiben · **ENC** verschlüsselte PDFs lesen und im Update korrekt schreiben ·
**EXT-A** Raw-Signatur-Modell · **EXT-B** CMS-Container-Modell · **SF** SubFilter frei wählbar (PAdES) · **AP** eigene Appearance/Form-XObjects ·
**WASM / MOB** läuft im Browser bzw. auf iOS und Android · **LIZ** EUPL-tauglich.

---

## 3. Lizenzrahmen EUPL-1.2

- **Kompatibilitätsklausel (Art. 5):** Wird ein Werk, das auf EUPL-Code **und** auf Code unter einer „kompatiblen Lizenz" beruht, weitergegeben,
  darf das unter der kompatiblen Lizenz geschehen; bei Widersprüchen gehen deren Pflichten vor (`PDFOVER:LICENSE:120-127`) [C].
- **Kompatible Lizenzen (Anhang):** GPL v2/v3, AGPL v3, OSL 2.1/3.0, EPL 1.0, CeCILL 2.0/2.1, MPL 2, LGPL 2.1/3, CC BY-SA 3.0 (nicht für Software), EUPL 1.1/1.2, LiLiQ-R/R+ (`PDFOVER:LICENSE:266-278`) [C].
- **Praktische Folgen [I]:**
  - **MIT, BSD-2/3, Apache-2.0, ISC, Zlib, Unlicense:** unproblematisch, ein EUPL-Gesamtwerk ist möglich (Hinweise und NOTICE beibehalten).
  - **MPL-2.0:** unproblematisch, Copyleft gilt nur dateiweise. Betrifft z. B. `uniffi`, `cryptographic-message-syntax`, `x509-certificate` und die MPL-Option von PoDoFo.
  - **LGPL-2.1/3:** kompatibel. Bei statischem Linken (iOS, teils Android) muss aber das Neu-Linken durch Nutzer möglich sein → in der Praxis lästig. Wenn verfügbar, MPL-Option wählen (PoDoFo).
  - **GPL/AGPL:** stehen zwar auf der Liste, aber dann **muss das Gesamtwerk unter GPL/AGPL** weitergegeben werden (Klausel „Pflichten der kompatiblen Lizenz gehen vor").
    Dazu kommen die bekannten Konflikte zwischen GPL/AGPL und den App-Store-Bedingungen [E]. → für einen EUPL-Klon ausschließen.
  - **Proprietär/Community-Lizenz** (Syncfusion, Apryse, Nutrient, unipdf): keine Open-Source-Lizenz. Jede weitergebende Partei braucht eine eigene Lizenz → für ein frei verteiltes EUPL-Projekt ungeeignet.
  - **Six Labors Split License** (SixLabors.Fonts, ImageSharp): Apache-2.0 nur unter Bedingungen, sonst kommerziell [W] → vermeiden.
- **Herkunft der Referenzartefakte:** PDF-Over ist EUPL-1.2 (`PDFOVER:LICENSE`). PDF-AS liefert eine **EUPL v1.1**-Lizenzdatei mit (`PDFAS:license/EUPL v.1.1 - Licence.pdf`) [C].
  Werden Konfiguration, Profile, Logos oder Code-Teile übernommen, ist der Klon ein abgeleitetes Werk → EUPL; bei reiner Neuimplementierung frei wählbar [I].

---

## 4. Ökosysteme im Detail

### 4.1 Rust

#### (a) PDF lesen + Incremental Update

| Crate | Version (Datum) | Lizenz | EUPL | Reife/Wartung | Relevante Features | Lücken |
|---|---|---|---|---|---|---|
| **`lopdf`** | 0.45.0 (2026-09-08) | MIT | ok | 24 Mio. Downloads, ca. 2.270 GitHub-Sterne, Push 2026-10-14; seit 2016 | `IncrementalDocument` (eigenes Modul `src/incremental_document.rs`: alte Bytes + neue Objekte, `opt_clone_object_to_new_document`) [W]; xref-Streams und Object Streams lesen/schreiben (`use_xref_streams`, `use_object_streams`) [W]; Entschlüsselung (RC4/AES) [W]; 0.44: **inkrementelles Speichern verschlüsselter Dokumente** (#522) [W]; 0.44/0.45: Schutz vor Dekompressionsbomben, Robustheitsfixes [W]; Feature `wasm_js` für Verschlüsselung auf WASM [W]; Font-Embedding inzwischen über `skrifa` [W] | Low-Level-API ohne AcroForm-/Signatur-Abstraktion; laut Doku-Kommentar müssen alte Bytes und geparstes Dokument **exakt** zusammenpassen, sonst entstehen kaputte PDFs [W]; ob das Update bei xref-Stream-Vorgängern selbst als xref-Stream geschrieben wird, ist **nicht geprüft** (offene Frage) |
| `hayro-syntax` | 0.8.0 (2026-10-04) | Apache-2.0 OR MIT | ok | Teil von hayro (L. Voss, typst-Umfeld), aktiv | Robuster Leser (Grundlage des Renderers) | nur lesen |
| `pdf` (pdf-rs) | 0.10.0 (2026-03-02) | MIT | ok | seit 2015, ca. 1.700 Sterne | Lesen, teils Schreiben | Incremental-Update-Unterstützung nicht dokumentiert [I] |
| `pdf-writer` | 0.15.0 (2026-05-27) | MIT OR Apache-2.0 | ok | typst, aktiv | Schnelles Low-Level-Schreiben von Objekten (z. B. Appearance-Streams, CID-Fonts) | kein Parser, kein Update-Modus → nur als Serialisierungshilfe [I] |
| `krilla` | 0.8.2 (2026-06-04) | MIT OR Apache-2.0 | ok | aktiv | High-Level-Erzeugung inkl. Font-Subsetting, Bildern, PDF/A-Unterstützung [E] | erzeugt neue Dokumente; möglicher Weg: Signaturblock wie PDF-AS (V1) als Mini-PDF bauen und Objekte übernehmen [I] |
| `oxidize-pdf` | 5.4.2 (2026-10-11) | MIT | ok | seit 2025-07, 137 Tsd. Downloads | Parsing, Verschlüsselung (RC4/AES-128/AES-256 R5 lesen+schreiben, R6 nur lesen), Signatur-Erkennung, PKCS#7-Prüfung, **„incremental-signature preparation"**: externer Signer liefert CMS, Finalisierung bettet es ein (= EXT-B) [W] | PAdES ausdrücklich außerhalb des Umfangs [W]; jung |
| `pdf_oxide` | 0.3.78 (2026-09-08) | MIT OR Apache-2.0 | ok (Name/Logo markenrechtlich geschützt [W]) | seit 2025-11, 1,26 Mio. Downloads, Bindings für viele Sprachen und WASM | Incremental Saves, Verschlüsselung, Modul `signatures` (hinter Feature-Flag), Abhängigkeiten `cms`, `p12-keystore` [W] | Signaturformat und externe Signer nicht dokumentiert [W]; sehr jung |
| `qpdf` (Crate) | 0.3.7 (2026-09-27) | MIT/Apache-2.0 (Binding), qpdf selbst Apache-2.0 | ok | Binding an C++ | Lesen, Umschreiben, Verschlüsselung | qpdf schreibt keine Incremental Updates (4.6) → für Signaturen ungeeignet [I] |
| `printpdf` | 0.12.8 (2026-09-05) | MIT | ok | aktiv | Erzeugen | kein Update |
| `pdf-toolkit-edit`, `justpdf-core`, `shojiku-signing` | 2026 | MIT bzw. MIT/Apache/BSD | ok | < 1.000 Downloads, wenige Wochen bis Monate alt | Incremental Updates/Platzhalter laut Beschreibung | zu jung, nur beobachten [I] |

#### (b) Fertige PDF-Signatur

| Crate | Version (Datum) | Lizenz | EUPL | Reife | Features | Lücken |
|---|---|---|---|---|---|---|
| **`underskrift`** (Kushal Das) | 0.1.4 (2026-06-01), Push 2026-06-22 | BSD-2-Clause | ok | seit 2026-03, ca. 3.700 Downloads, 13 Sterne | PAdES **B-B/B-T/B-LT/B-LTA**, `adbe.pkcs7.detached`, sichtbare Signaturen mit Font-Subsetting (`subsetter`, `ttf-parser`), RFC 3161 mit TSA-Pool, LTV/DSS, Prüfung, **Remote-Signing in drei Phasen** (`prepare_signature` → externer Signer signiert `attrs_hash` → `finalize_signature`) = EXT-A, Trait `CryptoSigner`; RSA PKCS#1 v1.5/PSS, ECDSA P-256/384/521, Ed25519 [W]. Basis: `lopdf` 0.39 + RustCrypto (`cms` 0.2, `x509-cert` 0.2) [W] | EXT-B (fertiges CMS einsetzen) nicht dokumentiert; zieht standardmäßig `reqwest`+`tokio` und `josekit` (SVT) → Features gezielt abschalten; sehr jung, Ein-Personen-Projekt [W]/[I] |
| `pdf_signing` (ralpha) | 0.3.0 (2024-11-08) | MIT OR Apache-2.0 | ok | ca. 9.500 Downloads, seit 2024-11 ohne Commit | `lopdf` 0.34 + `cryptographic-message-syntax` 0.27; Signieren und Signaturen hinzufügen [W] | nur 20 % dokumentiert, keine Angaben zu PAdES/Incremental [W]; faktisch inaktiv |
| `pdf_signer` (StrategicProjects) | 0.3.2 (2026-09-25) | **GPL-3.0-or-later** | **nein** (erzwingt GPL) | 2026-07 | PKCS#12, PAdES B-B → B-LTA laut Beschreibung | Lizenz |
| `pdfluent`/`pdfluent-sign` | 1.0.0 (2026-09-16) | **AGPL-3.0 OR kommerziell** | **nein** | jung | Validierung PAdES/LTV, WASM | Lizenz |
| `ades-rs`, `normordis-pdf`, `pdfsigner`, `podpis-rs` | 2026 | div. | – | < 300 Downloads | – | zu jung [I] |

#### (c) CMS / X.509 / ASN.1 / Keystores / Primitive

| Crate | Version (Datum) | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|
| **`cms`** (RustCrypto) | stabil **0.2.3 (2024-01-08)**; **0.3.0-rc.0 (2026-10-13)** | Apache-2.0 OR MIT | ok | `SignedDataBuilder`/`SignerInfoBuilder` mit **`external_message_digest`** (detached signieren über einen extern berechneten Digest) und `add_signed_attribute`, Hilfen für `contentType`, `messageDigest`, `signingTime` – schon in 0.2.3 (`src/builder.rs:106-138,183-187` im Crate-Archiv) [W]. **Asynchrone Signer** (`AsyncSigner`, `add_signer_info_async`) erst in der 0.3-Linie (`cms/src/builder.rs:500-507` auf `master`) [W]. `signingCertificateV2` (ESSCertIDv2) muss man selbst als Attribut kodieren [I] |
| `x509-cert`, `der`, `spki`, `pkcs8`, `const-oid` | x509-cert 0.3.0 (2026-07-09), der 0.8.2, spki 0.8.1, pkcs8 0.11.0 | Apache-2.0 OR MIT | ok | Sehr verbreitet (hunderte Mio. Downloads). **Achtung: Generationswechsel** – `cms` 0.2.3 hängt an `der` 0.7 / `x509-cert` 0.2 / `spki` 0.7 [W]; die neue Generation (`der` 0.8, `x509-cert` 0.3, `ecdsa` 0.17, `p256`/`p384` 0.14, `rsa` 0.10-rc) braucht `cms` 0.3-rc → bewusst **eine** Generation pinnen [I] |
| `pkcs12` (RustCrypto) | stabil 0.1.0 (2024-01-04); 0.2.0-pre.0 (2026-01-12) | Apache-2.0 OR MIT | ok | Grundtypen, setzt auf `cms` 0.2 |
| **`p12-keystore`** | 0.4.1 (2026-10-13) | MIT/Apache-2.0 | ok | Hohe Abstraktion nach Java-KeyStore-Vorbild, moderne **und** Legacy-Verschlüsselung, Java-kompatibel; **keine MD5-basierten Schemata**, keine unverschlüsselten Stores [W] |
| `p12` | 0.6.3 (2022-02-18) | MIT OR Apache-2.0 | ok | veraltet (von `underskrift` noch genutzt) [W] |
| `jks` | 0.3.3 (2026-01-30) | MIT | ok | JKS + PKCS#12, Projekt aus 2026-01, JCEKS-Unterstützung nicht geprüft [W]/[I] |
| `rasn` + `rasn-cms` | 0.29.0 (2026-10-05) | MIT OR Apache-2.0 | ok | Alternative ASN.1-Codegen-Bibliothek mit CMS-Typen, aktiv |
| `cryptographic-message-syntax`, `x509-certificate` (indygreg) | 0.28.0 / 0.25.0 (2025-08-17) | **MPL-2.0** | ok (dateiweises Copyleft) | SignedData-Builder, TSP-Client; seit 2025-08 kein Release |
| `x509-parser`, `asn1-rs` (rusticata) | 0.18.1 / 0.7.2 | MIT OR Apache-2.0 | ok | nur Parsen (gut für Prüfung K3) |
| `rsa` | stabil 0.9.10 (2026-01-06); 0.10.0-rc.19 | MIT OR Apache-2.0 | ok | **RUSTSEC-2023-0071 (Marvin, Timing-Seitenkanal) ohne Fix.** Laut Advisory ist lokale Nutzung auf einem nicht kompromittierten Rechner unkritisch [W]; für Keystore-Signieren in einer Desktop-App vertretbar, alternativ RSA über `aws-lc-rs`/`ring` [I] |
| `p256`, `p384`, `ecdsa`, `sha2`, `sha1` | 0.14 / 0.14 / 0.17 / 0.11.1 / 0.11 | Apache-2.0 OR MIT | ok | rein Rust, WASM-fähig |
| `ring` | 0.17.14 (2025-03-11) | Apache-2.0 AND ISC | ok | Das „unmaintained"-Advisory RUSTSEC-2025-0007 wurde **zurückgezogen** [W]; RSA/ECDSA-Signatur, kein CMS |
| `aws-lc-rs` | 1.18.2 (2026-10-12) | ISC AND (Apache-2.0 OR ISC) | ok | FIPS-fähig, braucht C/CMake-Toolchain, **kein WASM** [E] |
| `openssl` | 0.10.81 (2026-06-12) | Apache-2.0 | ok | voller CMS/TS-Support über OpenSSL, aber C-Abhängigkeit auf allen Zielplattformen [E] |
| `x509-tsp` / `x509-ocsp` | 0.1.0 (2023-06-26) / 0.2.1 (2024-01-08) | Apache-2.0 OR MIT | ok | Datentypen für RFC 3161/6960 (für spätere B-T/B-LT) |
| `tsp-ltv` | 0.5.0 (2026-09-28) | BSD-2-Clause | ok | TSA/OCSP/CRL-Infrastruktur (von `underskrift` genutzt), Repository nicht angegeben [W] |
| `rustls-webpki` | 0.103.15 | ISC | ok | Kettenprüfung (für K3 bzw. eine spätere Validierung) |
| `cryptoki` | 0.12.1 (2026-09-16) | Apache-2.0 | ok | PKCS#11 (Smartcards/HSM, nur Desktop) |

#### (d) Rendering

| Crate | Version | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|
| **`pdfium-render`** | 0.9.4 (2026-09-06) | MIT OR Apache-2.0 | ok | Bindet PDFium dynamisch oder statisch; **WASM unterstützt** (PDFium als eigenes WASM-Modul) [W]; Signatur-Introspektion (nur lesen) [W]. Binärdateien: `bblanchon/pdfium-binaries` chromium/8100 (2026-10-12) für Android (arm/arm64/x64/x86), iOS (Device, Simulator, Catalyst), Linux, macOS, Windows, **WASM (experimentell)** [W] |
| **`hayro`** | 0.8.0 (2026-10-04) | Apache-2.0 OR MIT | ok | Reiner Rust-Renderer (PNG/SVG); laut README experimentell und in Arbeit, Performance noch kein Schwerpunkt, rendert aber über 1.400 Test-PDFs aus den PDFBox- und pdf.js-Regressionssuiten; Web-Demo [W] |
| `mupdf` | 0.8.0 | **AGPL-3.0** | **nein** | – |
| `pdfium` (newinnovations) | 0.10.4 | **GPL-3.0** | **nein** | – |

#### (e) Fonts / Text-Layout

| Crate | Version (Datum) | Lizenz | Bewertung |
|---|---|---|---|
| `skrifa` / `read-fonts` (Google fontations) | 0.48.0 / 0.45.0 (2026-10-03) | MIT OR Apache-2.0 | Parsen, Metriken, Glyph-IDs; `lopdf` ist von `ttf-parser` auf `skrifa` umgestiegen (Changelog bezeichnet `ttf-parser` als nicht mehr gepflegt) [W] |
| `subsetter` (typst) | 0.2.6 (2026-06-04) | MIT OR Apache-2.0 | TrueType/CFF-Subsetting für PDF |
| `ttf-parser` | 0.25.1 (2024-11-29) | MIT OR Apache-2.0 | stabil, aber seit 2024 kein Release |
| `allsorts` | 0.17.0 (2026-05-13) | Apache-2.0 | Shaping + Subsetting |
| `rustybuzz` | 0.20.1 (2024-11-12) | MIT | HarfBuzz-Port; für Latin-Tabellen (PDF-AS) nicht zwingend nötig [I] |
| `krilla` | 0.8.2 | MIT OR Apache-2.0 | Komplette Kette Font → CIDFontType2 + ToUnicode + W-Array als Vorlage [E] |

Lücke [I]: Die AFM-Metriken der Standard-14-Fonts (Helvetica, Courier; V2) liefert keines dieser Crates „fertig für PDF". Die Metriken müssen als Daten mitgeliefert werden;
PDF-AS bringt dafür die Adobe-AFM-Lizenz mit (`CFG:licenses/adobe/AFM_License.html`) [C].

#### (f) Bilder, EXIF, QR

| Crate | Version (Datum) | Lizenz | Bewertung |
|---|---|---|---|
| `image` | 0.25.10 (2026-03-10) | MIT OR Apache-2.0 | PNG/JPEG u. v. m.; ob EXIF-Orientierung eingebaut ausgewertet wird, ist nicht geprüft [I] |
| `png`, `zune-jpeg` | 0.18.1 / 0.5.15 | MIT/Apache (zune: + Zlib) | schnell, rein Rust |
| `hayro-jpeg2000`, `hayro-jbig2` (+ `hayro-ccitt`) | 0.4.1 / 0.3.1 (2026-10-04) | Apache-2.0 OR MIT | **Decoder für JPX, JBIG2 und CCITT in reinem Rust** → deckt Q1 ohne C-Bibliotheken ab [W]/[I] |
| `kamadak-exif` | 0.6.1 (2024-11-06) | BSD-2-Clause | EXIF-Orientierung (B1) |
| `nom-exif` | 3.8.0 | „non-standard" auf crates.io | Lizenz vor Nutzung prüfen |
| **`rqrr`** | 0.11.0 (2026-09-02) | (MIT OR Apache-2.0) AND ISC | QR-only, Port von quirc |
| **`rxing`** | 0.9.3 (2026-09-16) | Apache-2.0 | ZXing-Port; dieselbe Pipeline wie PDF-AS (`HybridBinarizer`) möglich [I] |
| `bardecoder` | 0.5.0 (2023-07-29) | MIT | inaktiv |

#### (g) HTTP / Cookies / Proxy / HTML

| Crate | Version | Lizenz | Bewertung |
|---|---|---|---|
| **`reqwest`** | 0.13.5 (2026-09-08) | MIT OR Apache-2.0 | Feature `cookies` (über `cookie_store`); **`system-proxy` standardmäßig an**: liest Windows- und macOS-Proxy-Einstellungen; außerdem `HTTP(S)_PROXY`/`ALL_PROXY`, SOCKS als Feature (`Cargo.toml:39-77`, `src/lib.rs:137-217`) [W]. Auf `wasm32` nutzt es Browser-`fetch`, TLS und Cookies kommen dann aus dem Browser (`lib.rs:178-184`) → CORS-Grenzen [W]/[I]. Standard-TLS ist `rustls` mit **`aws-lc-rs`** (C-Toolchain) und `rustls-platform-verifier`; für schlanke Mobile-Builds `rustls-no-provider` + `ring` wählen [W]/[I] |
| `ureq` | 3.4.2 (2026-09-13) | MIT OR Apache-2.0 | synchron, schlank; Proxy aus der Umgebung [E] |
| `cookie_store` / `reqwest_cookie_store` | 0.22.1 / 0.10.0 | MIT/Apache | persistierbarer Cookie-Speicher |
| `scraper` | 0.27.0 (2026-05-11) | ISC | Ersatz für Jsoup (H1) |

#### Integration auf Zielplattformen

`uniffi` 0.32.2 (MPL-2.0) für Swift/Kotlin, `flutter_rust_bridge` 2.13.0 (MIT) für Dart, `tauri` 2.12.3 (Apache-2.0 OR MIT), `wasm-bindgen` für das Web [W].

**Bewertung Rust [I]:** bester Gesamtpfad. Alles Kritische (IU, XS, ENC, EXT-A über `cms`, EXT-B trivial selbst gebaut, SF, AP über Low-Level-Objekte) ist mit permissiven Crates erreichbar, und dieselbe Codebasis läuft nativ und als WASM.
Eigenleistung bleibt: Signaturfeld/Widget/AcroForm-Logik, Appearance-Bau (Tabellenlayout nach PDF-AS), Zusammenbau von CID-Fonts, PAdES-Attribute (ESSCertIDv2), Prüfung nach K3 sowie das robuste Lesen kaputter PDFs.

---

### 4.2 TypeScript / JavaScript

#### (a) PDF lesen + Incremental Update

| Paket | Version (Datum) | Lizenz | EUPL | Reife | IU / XS / ENC | Bemerkungen |
|---|---|---|---|---|---|---|
| `pdf-lib` (Hopding) | 1.17.1 (**2021-11-06**), letzter Push 2024-07 | MIT | ok | 18 Mio. Downloads/Woche, aber **ungepflegt** | IU **nein**, ENC **nein** (README: verschlüsselte Dokumente nicht unterstützt, `EncryptedPDFError`) [W] | Speichern schreibt die ganze Datei neu → vorhandene Signaturen brechen [I] |
| **`@cantoo/pdf-lib`** | 2.11.1 (2026-09-15) | MIT | ok | aktiver Fork, 1,1 Mio./Woche | **IU ja** (`forIncrementalUpdate`, `saveIncremental`, `commit()`, ausdrücklich fürs Signieren gedacht), **ENC ja** (`encrypt()` AES-256 R6, Entschlüsseln inkl. indirekter Crypt-Filter) [W] | API-kompatibel zu pdf-lib |
| **`@libpdf/core`** (Documenso) | 0.5.2 (2026-10-07) | MIT | ok | seit 2026-01, 155 Tsd./Woche, ausdrücklich **Beta**, im Produktiveinsatz bei Documenso [W] | **IU ja**, **ENC ja** (RC4, AES-128, AES-256 R2–R6), nachsichtiger Parser „wie PDFBox/pdf.js" [W] | JBIG2/JPX werden nur durchgereicht, nicht dekodiert (relevant für Q1) [W]; Abhängigkeiten `pkijs`, `asn1js`, `@noble/*`, `pako` [W] |
| `@pdfme/pdf-lib` | 6.2.5 (2026-10-14) | MIT | ok | Fork im pdfme-Monorepo | nicht geprüft | – |
| `pdfjs-dist` | 6.4.299 (2026-10-03) | Apache-2.0 | ok | Mozilla | nur lesen/rendern | Bild-XObjects über Operator-Listen zugänglich [E] |
| `mupdf` (mupdf.js, WASM) | 1.28.1 (2026-09-06) | **AGPL-3.0-or-later** | **nein** | Artifex | IU + Signieren [E] | Lizenz |

#### (b) Fertige PDF-Signatur

| Paket | Version | Lizenz | Modell | Bewertung |
|---|---|---|---|---|
| **`@signpdf/signpdf`** + `utils`/`signer-p12`/`placeholder-*` | 3.3.0 (2025-12-29) | MIT | **EXT-B**: `Signer.sign(pdfBuffer, signingTime)` bekommt die ByteRange-Daten und liefert das CMS (`packages/utils/src/Signer.js:4-15`) [W] | Passt 1:1 zum SL/BKU-Modell K4 [I]. Platzhalter und ByteRange werden aber **per String-Suche** gefunden (`packages/signpdf/src/signpdf.js:40-63`) [W]. `placeholder-plain` ist laut README fragil, kann keine Streams und funktioniert nur bis **PDF 1.3** [W]. `placeholder-pdf-lib` + pdf-lib schreibt die ganze Datei neu [I]. `signer-p12` nutzt node-forge → kein ECDSA [I] |
| **`@libpdf/core`** `pdf.sign()` | 0.5.2 | MIT | **EXT-A**: `Signer`-Interface mit `certificate`, `certificateChain`, `keyType`, `signatureAlgorithm`, `sign(data, algorithm)` → rohe Signatur (RSA PKCS#1/PSS, ECDSA als DER) (`dist/index.d.mts`) [W] | PAdES **B-B/B-T/B-LT/B-LTA**, `subFilter` `ETSI.CAdES.detached` (Standard) oder `adbe.pkcs7.detached`, `estimatedSize`, `HttpTimestampAuthority`, Signer für P12, Google KMS und WebCrypto [W]. **Lücken:** keine sichtbaren Signatur-Appearances („not yet implemented"), keine Signaturprüfung, kein Hook für ein fertiges CMS (EXT-B) [W] |

#### (c) CMS / X.509 / ASN.1

| Paket | Version (Datum) | Lizenz | Bewertung |
|---|---|---|---|
| **`pkijs`** + `asn1js` | 3.4.1 (2026-09-20) / 3.0.10 | BSD-3-Clause | CMS SignedData (Signieren/Prüfen, detached), TSP-Request/-Response, OCSP, PKCS#12; nutzt WebCrypto → läuft im Browser und in Node/Deno [W]. Ob Legacy-PKCS#12 (3DES/RC2) unterstützt wird, ist offen (WebCrypto kennt diese Verfahren nicht) [I] |
| `@peculiar/asn1-schema`, `@peculiar/asn1-cms`, `@peculiar/x509` | 2.11.0 / 2.11.0 / 2.1.0 | MIT | typisierte ASN.1-Schemata, sehr verbreitet |
| `@peculiar/webcrypto` | 1.7.1 | MIT | WebCrypto-Polyfill für Node |
| `node-forge` | 1.4.0 (2026-03-24) | BSD-3-Clause **OR** GPL-2.0 (BSD wählen) | PKCS#7, PKCS#12 inkl. Legacy, RSA, RSA-PSS, Ed25519 – **kein ECDSA** (im README nicht aufgeführt) [W]/[I] |
| `jks-js` | 1.1.7 (2026-05-08) | MIT | JKS → PEM; JCEKS nicht geprüft [W]/[I] |

#### (d) Rendering

`pdfjs-dist` 6.4.299 (Apache-2.0); `@embedpdf/pdfium` 2.15.1 (MIT, PDFium-WASM, 1 Mio./Woche); `@hyzyla/pdfium` 3.0.0 (MIT); `mupdf` (AGPL, ausgeschlossen) [W].

#### (e) Fonts

`fontkit` 2.0.4 (2024-08-09, MIT); `@pdf-lib/fontkit` 1.1.1 (**2020**, MIT); `harfbuzzjs` 1.6.3 (2026-10-06, MIT, mit hb-subset als WASM) [W]; `@libpdf/core` bringt eigenes TTF/OTF-Subsetting mit (TTC nicht unterstützt) [W].

#### (f) Bilder / EXIF / QR

Browser-Decoder (Canvas/`createImageBitmap`) [E]; `exifr` 7.1.3 (2021, MIT); `jsqr` 1.4.0 (2021, Apache-2.0); `@zxing/library` 0.23.0 (2026-04, Apache-2.0, README: **nur noch Wartungsmodus**) [W]; **`zxing-wasm`** 3.1.5 (2026-10-06, MIT, Wrapper um zxing-cpp) [W].

#### (g) HTTP

Browser-`fetch`: CORS, keine Cookie-Kontrolle über Domänen hinweg → A-Trust-Flow nicht direkt möglich [I] (vgl. Recherche 01, Abschnitt 0 Punkt 1).
Node: `undici` 8.12.1 (MIT) + `tough-cookie` 6.0.2 (BSD-3) oder `fetch-cookie` 3.2.0 (Unlicense) + `proxy-agent` 8.0.2 (MIT) [W]. Electron `net` und Tauri `plugin-http` (reqwest) [E].
HTML: DOMParser bzw. cheerio [E].

**Bewertung TS [I]:** Für eine Web-/Electron-/Tauri-UI sehr attraktiv (pdf.js, pkijs, `@libpdf/core`).
Als *Kern* aber riskant: kein vollständiges Paket mit Appearance **und** EXT-B, Beta-Status, Mobile nur über WebView/React Native (dort fehlen `Buffer`/WebCrypto, Polyfills nötig) und Performance bei großen PDFs.

---

### 4.3 Dart / Flutter

| Kat. | Paket | Version (Datum) | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|---|
| a | `pdf` (DavBfr) | 3.13.1 (2026-09-19) | Apache-2.0 | ok | Sehr verbreitet (1,7 Mio. Downloads/30 Tage). Erzeugen; `PdfDocument.load(PdfDocumentParserBase)` hängt Updates mit `/Prev` an, **der Parser ist aber nur eine abstrakte Klasse** (`pdf/lib/src/pdf/document_parser.dart`, `document.dart:93-111,255-256`) [W]; Signatur-Hook `PdfSignatureBase.sign(...)` mit Offsets (`pdf/lib/src/pdf/obj/signature.dart:37-112`) [W] |
| a/b | `pdf_cos` + `pdf_document` (+ `dart_pdf_editor`) | 8.1.0 (2026-10-10) | Apache-2.0 | ok | **Erst seit 2026-06**, rund 10 Tsd. Downloads/30 Tage, 1 Like. README: COS-Reader mit xref-/Object-Streams, signaturerhaltender Incremental-Writer, RC4/AES lesen **und** Encrypt-on-Write (`/Contents` bleibt unverschlüsselt), CCITT/JBIG2/JPX in reinem Dart, Signaturprüfung (RSA, ECDSA, PSS) und Signieren (`saveSigned`), **aber nur `adbe.pkcs7.detached` mit RSA-SHA256** [W] |
| b | `syncfusion_flutter_pdf` | 35.1.40 (2026-10-13) | **proprietär** (Community-Lizenz: < 1 Mio. USD Umsatz und < 5 Entwickler) [W] | **nein** | Digitale Signatur mit `IPdfExternalSigner` (EXT-A), `CryptographicStandard.cades` [S]; `PdfFileStructure.incrementalUpdate` vorhanden, Standardwert nicht dokumentiert [W] |
| b | `nkpdfsign` | 0.1.1 (2026-07-17) | MIT | ok | PAdES-B-B + `adbe.pkcs7.detached` mit Incremental Updates, `pointycastle`; 22 Downloads/30 Tage [W] |
| a/b | `dpdf` | 1.0.0 (2026-09-14) | MIT | ok | „Pure Dart"-Rundumpaket mit Signaturen und PDF/A laut Beschreibung; genau ein Release [W] |
| b | `signer_pdf` | 0.0.23 | MIT | ok | nur Android/Windows/Linux (Plugin) [W] |
| c | **`pointycastle`** | 4.0.0 (2025-02-19) | MIT | ok | Primitive: RSA, (DET-)ECDSA, Digests, HMAC, PBKDF2, ASN.1-Klassen [W]; **kein CMS, kein PKCS#12-Container** |
| c | `asn1lib` | 1.6.5 (2025-06-24) | BSD-2-Clause | ok | ASN.1 DER |
| c | `basic_utils` (X509Utils) / `x509` | 5.8.2 (2025-02) / 0.2.4+3 (2024-09) | MIT / BSD-3 | ok | Zertifikate parsen |
| c | `cryptography` / `webcrypto` | 2.9.0 (2025-11) / 0.6.1 (2026-05) | Apache-2.0 | ok | `webcrypto` (Google, BoringSSL über FFI bzw. WebCrypto) |
| d | **`pdfrx`** | 2.6.5 (2026-09-18) | MIT | ok | PDFium auf allen 6 Plattformen, **Web über PDFium-WASM**; optional CoreGraphics/PDFKit auf Apple (experimentell) [W] |
| d | `pdfx` / `printing` | 2.11.0 / 5.15.1 | MIT / Apache-2.0 | ok | native Renderer bzw. Druck |
| e | `pdf`: `TtfParser`, `TtfWriter.withChars` (Subsetting) | – | Apache-2.0 | ok | `pdf/lib/src/pdf/font/ttf_writer.dart:27,83` [W] |
| f | `image` | 4.12.0 (2026-10-11) | MIT | ok | JPEG/PNG/EXIF |
| f | `exif` / `native_exif` | 3.3.0 (2023) / 0.8.0 | MIT | ok | EXIF |
| f | `zxing2` / **`flutter_zxing`** / `qr_code_dart_decoder` | 0.2.4 (2025-06) / 3.1.0 (2026-09) / 0.3.0 | BSD-3 (laut pub-Tag; Port von Apache-ZXing → prüfen) / MIT / MIT | ok | `flutter_zxing` = zxing-cpp über FFI, **kein Web** [W] |
| g | `http`, `dio` + `cookie_jar`/`dio_cookie_manager`, `cupertino_http` (NSURLSession), `cronet_http` (Android), `native_dio_adapter` | 1.6.0 / 5.11.1 / 4.0.9 / 3.1.0 / 1.10.0 | BSD-3 / MIT / Apache-2.0 | ok | `dart:io`-HttpClient übernimmt den System-Proxy nicht automatisch, die nativen Adapter schon [E] |
| g | `html` (dart-lang) | 0.15.7 | BSD-3 (pub zeigt „unknown") | ok | Jsoup-Ersatz |

**Bewertung Dart [I]:** Als UI-Schicht stark (pdfrx, Flutter). Als Kern fehlt ein **reifes** CMS/CAdES und ein reifer PDF-Editor mit Incremental Update.
Ein eigenes CMS auf `asn1lib`/`pointycastle` wäre machbar, das PDF-Low-Level aber nicht ohne große Eigenleistung. → Rust-Kern + `flutter_rust_bridge`.

---

### 4.4 C# / .NET

| Kat. | Paket | Version (Datum) | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|---|
| a/b | **PDFsharp** | **6.5.0 (2026-10-14)**, 7.0.0-preview-2 (2026-10-13) | **AGPL-3.0-only oder kommerziell** ab 6.5.0/7.0.0-p2; ≤ 6.2.4 (2026-01-06) MIT [W] | ab 6.5 **nein** | Signaturen seit 6.2.0, `IDigitalSigner` mit `GetSignatureSizeAsync()` und `GetSignatureAsync(Stream)` → liefert die Signaturbytes über den Bereichsstream = **EXT-B** (`src/foundation/src/shared/src/PdfSharp.System/Pdf.Signatures/IDigitalSigner.cs`) [W]. **SubFilter fest `/adbe.pkcs7.detached`** (`src/foundation/src/PDFsharp/src/PdfSharp/Pdf.Signatures/DigitalSignatureHandler.cs:244`) [W]. Im ganzen Quellbaum gibt es keine Datei zu Incremental Updates; Signieren läuft beim Gesamtspeichern → schreibt neu [W]/[I]. Laut Doku wird die Signaturgröße über eine Dummy-Signatur ermittelt [W] |
| a | `PdfSharpCore` | 1.3.67 (2025-04-03) | MIT (laut Repository) | ok | alter Fork von PDFsharp 1.5, kein Signieren [E] |
| a | **`PdfPig`** | 0.1.16 (2026-08-22) | Apache-2.0 | ok | Lesen/Textextraktion, kein Incremental Update [E] |
| a | `Melville.Pdf.Model` | 0.6.8 (2026-08-07) | MIT | ok | klein (13 Tsd. Downloads) |
| a/b | `itext` / `itext7` | 9.8.0 (2026-09-30) | **AGPL** / kommerziell | **nein** | Referenzklasse (Append-Modus, `IExternalSignatureContainer` = EXT-B, PAdES-Signer) [E] |
| a/b | `Syncfusion.Pdf.Net.Core` | 35.1.40 (2026-10-13) | **proprietär** | **nein** | Deferred Signing (Hash extern signieren), CAdES [S] |
| c | **`BouncyCastle.Cryptography`** | 2.7.0 (2026-07-30) | MIT | ok | `CmsSignedDataGenerator`, `Pkcs12Store`, **`JksStore`** (nur JKS-Magic `0xFEEDFEED`, **kein JCEKS**, `crypto/src/security/JksStore.cs:21,512-515`) [W], TSP (`crypto/src/tsp/TimeStampRequestGenerator.cs`) [W]; rein verwaltet → läuft auch in Blazor-WASM [E] |
| c | `System.Security.Cryptography.Pkcs` | 10.0.13 (2026-10-13) | MIT | ok | `SignedCms`; Plattform-Krypto, auf Apple ohne Brainpool (s. Recherche 01, 5.7); in Browser-WASM nicht verfügbar [E] |
| d | `PDFiumCore` / `bblanchon.PDFium.*` / `PDFtoImage` / `Docnet.Core` | 156.0.8076 (2026-10-01) / 157.0.8100 (2026-10-12) / 5.4.0 / 2.6.0 (2023) | Apache-2.0 / Apache-2.0 / MIT / MIT | ok | PDFium-Bindings, inkl. `bblanchon.PDFium.WebAssembly` [W] |
| e | `SixLabors.Fonts` | 3.2.0 | **Six Labors Split License** | bedingt | vermeiden [W]/[I] |
| e | `HarfBuzzSharp`, `Topten.RichTextKit` | 14.2.1.301 / 0.6.170 | MIT / Apache-2.0 | ok | Shaping; Subsetting-API nicht geprüft |
| f | `SkiaSharp`; `SixLabors.ImageSharp` | 4.153.1; 4.1.3 | MIT; **Split License** | ok; bedingt | – |
| f | **`MetadataExtractor`** | 2.9.3 (2026-04-08) | Apache-2.0 | ok | .NET-Port genau der Java-Bibliothek, die PDF-Over für EXIF nutzt (B1) [W]/[C] |
| f | `ZXing.Net` | 0.16.11 (2025-10-25) | Apache-2.0 | ok | QR |
| g | `HttpClient` + `CookieContainer` + `HttpClient.DefaultProxy` | Runtime | MIT | ok | System-Proxy und Cookies an Bord [E] |
| g | `AngleSharp` / `HtmlAgilityPack` | 1.8.4 / 1.13.1 | MIT | ok | Jsoup-Ersatz |

**Bewertung .NET [I]:** Krypto erstklassig (BouncyCastle). Auf der PDF-Seite gibt es aber seit dem Lizenzwechsel von PDFsharp **keine** permissive, gepflegte Bibliothek mit Incremental Update und frei wählbarem SubFilter mehr → der PDF-Low-Level-Teil müsste selbst geschrieben werden (wie in Rust, nur ohne `lopdf`).
Linux-Mobile-Abdeckung hängt am UI-Framework (Avalonia statt MAUI, vgl. Recherche 01).

---

### 4.5 Go

| Kat. | Modul | Version (Datum) | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|---|
| b | **`github.com/digitorus/pdfsign`** | v0.9.0 (2026-08-06), Push 2026-10-14, 170 Sterne | BSD-2-Clause | ok | Fluent API, `Sign(...)` mit `crypto.Signer` (RSA, ECDSA, Ed25519, ML-DSA), `Format(pdfsign.PAdES_B_T)`, TSA, DocMDP/Certification/Approval/DocumentTimestamp, sichtbare Appearance mit TTF-Fonts und Bildern, Formularfelder, Signer-Beispiele für **CSC-API, KMS, PKCS#11**, Prüfung mit OCSP/CRL [W]. **Kein** B-LT/B-LTA (Revocation wird bei B-T nur online geprüft, nicht eingebettet) [W]. `SignContext` mit `NewXrefStart`/`ByteRangeValues` und Quelldateien `pdfxref_stream.go` → Incremental Update und xref-Streams sind **wahrscheinlich**, aber nicht dokumentiert [W]/[I]. Kein Hook für ein fertiges CMS (EXT-B) [W]. ENC: offen |
| a | `github.com/pdfcpu/pdfcpu` | v0.16.2 (2026-10-11), 8.866 Sterne | Apache-2.0 | ok | Validieren, Optimieren, **Ver-/Entschlüsseln**; Signaturen nur **prüfen/entfernen**, nicht erstellen (`pdfcpu.io/core/sign`) [W] |
| a/b | `github.com/unidoc/unipdf` | v4.11.0 (2026-07-03) | **UniDoc EULA** (kommerziell, Lizenzschlüssel nötig) | **nein** | [W] |
| a | `github.com/signintech/gopdf`, `github.com/benoitkugler/pdf` | v0.38.2 (2026-10-13) / v0.0.15 | MIT | ok | Erzeugen (gopdf mit TTF-Subsetting) [E] |
| c | stdlib `crypto/x509`, `crypto/ecdsa`, `crypto/rsa` | Go 1.27 | BSD-3 | ok | sehr gut, ML-DSA in Go 1.27 [W, laut pdfsign-README] |
| c | `github.com/digitorus/pkcs7` / `mozilla-services/pkcs7` / `smallstep/pkcs7` | 2026-09-14 / v0.10.0 (2026-07-21) / v0.2.3 (2026-07-29) | MIT | ok | CMS SignedData |
| c | `github.com/digitorus/timestamp` | 2026-09-14 | BSD-2-Clause | ok | RFC 3161 |
| c | `software.sslmate.com/src/go-pkcs12` | v0.7.4 (2026-10-10) | BSD-3-Clause | ok | PKCS#12 |
| c | `github.com/pavlo-v-chernykh/keystore-go/v4` | v4.5.0 (**2023-08-31**) | MIT | ok | JKS (JCEKS nicht geprüft) |
| d | `github.com/klippa-app/go-pdfium` | v1.21.3 (2026-10-09) | MIT | ok | PDFium über cgo oder als WebAssembly-Laufzeit [E] |
| d | `github.com/gen2brain/go-fitz` | v1.28.2 | **AGPL-3.0** | **nein** | MuPDF |
| e | `github.com/go-text/typesetting`, `golang.org/x/image/font/sfnt` | v0.3.5 / v0.47.0 | BSD-3 (+ Unlicense/MIT) | ok | Shaping/Parsing |
| f | stdlib `image/png`, `image/jpeg`; `rwcarlsen/goexif` (2019), `dsoprea/go-exif` v3.0.1 (2024) | – | BSD-3 / BSD-2 / MIT | ok | EXIF-Pakete alt |
| f | `github.com/makiuchi-d/gozxing` | v0.1.1 (2021, Push 2025-07) | Apache-2.0 | ok | QR |
| g | `net/http` + `net/http/cookiejar` + `ProxyFromEnvironment` | stdlib | BSD-3 | ok | System-Proxy unter Windows/macOS braucht Zusatzpakete [E] |
| g | `github.com/PuerkitoBio/goquery` | v1.13.0 | BSD-3 | ok | Jsoup-Ersatz |
| – | `golang.org/x/mobile` (gomobile bind) | 2026-10-11 | BSD-3 | ok | iOS-Framework, Android-AAR [W]/[E] |

**Bewertung Go [I]:** Der schnellste Weg zu einem *funktionierenden* PAdES-Signer (pdfsign + stdlib). Schwächen: WASM-Bundles werden groß, gomobile hat eine begrenzte API-Oberfläche, die UI-Integration ist schwächer, und EXT-B fehlt (Fork nötig).

---

### 4.6 C / C++

| Kat. | Bibliothek | Version (Datum) | Lizenz | EUPL | Bewertung |
|---|---|---|---|---|---|
| a/b | **PoDoFo** | 1.1.2 (2026-08-19) | **LGPL-2.0-or-later ODER MPL-2.0** (Tools GPL-2.0+) [W] | ok (MPL wählen) | Incremental Updates, **PAdES-B** mit RSA/ECDSA, **asynchrones/verzögertes Signieren**, Unterstützung für externe Signierdienste [W]; Low-Level-`PdfSigner`: `AppendData(buffer)` bekommt alle zu signierenden Daten, `ComputeSignature(buffer, dryrun)` liefert den Signaturpuffer = **EXT-B wie bei SL/BKU** [W]; Font-Subsetting, PDF/A- und PDF/UA-Erhalt [W]; **kein Rendering**, kein Text-Shaping [W]. Abhängigkeiten: OpenSSL, FreeType, libxml2, optional libjpeg/libtiff/libpng [W]; Cross-Compile für Android/iOS laut README möglich, aber ohne Support [W] |
| a | **qpdf** | 12.4.2 (2026-09-27) | Apache-2.0 | ok | Struktur-Transformation, Verschlüsselung, Linearisierung; in den Release Notes kein Schreiben von Incremental Updates zu finden → für Signaturen nicht geeignet [W]/[I] |
| a/d | **PDFium** | chromium/8100 (Binaries 2026-10-12) | BSD-3/Apache-2.0 | ok | `FPDF_SaveAsCopy` mit Flag **`FPDF_INCREMENTAL`** (`public/fpdf_save.h:46-67`) [W], aber Signatur-API **nur lesend** (`FPDF_GetSignatureCount`, Getter in `public/fpdf_signature.h`) [W] → keine Signaturfelder/-Dictionaries anlegen; `FPDFImageObj_GetBitmap`/`GetRenderedBitmap` (`public/fpdf_edit.h:807-848`) für Q1 [W] |
| a/b/d | MuPDF | 1.28.x | **AGPL-3.0** / kommerziell | **nein** | Signieren + Incremental Save + Rendering [E] |
| a | PDF-Writer (Hummus) | – | Apache-2.0 | ok | Änderung per Incremental Update [E], Wartung nicht geprüft |
| c | **OpenSSL** | 4.0.3 (2026-09-29) | Apache-2.0 | ok | `CMS_sign` (detached, eigene SignedAttributes), TS-Client [E] |
| c | Mbed TLS / Botan | 4.2.0 / – | Apache-2.0 (bzw. GPL-2.0+) / BSD-2 | ok | kein vollständiges CMS-Signieren [E] |
| e | FreeType / **HarfBuzz** (hb-subset) | – / 14.6.0 (2026-10-05) | FTL oder GPLv2 / MIT | ok (FTL) | Standard |
| f | libpng, libjpeg-turbo 3.2.0, stb_image; **zxing-cpp** v3.1.1 (2026-07-29); quirc | – | libpng/IJG-BSD/Public Domain; Apache-2.0; ISC | ok | QR: zxing-cpp ist der Unterbau von `zxing-wasm` und `flutter_zxing` [W] |
| g | **libcurl** 8.23.0 (2026-10-14) | curl (MIT-artig) | ok | Cookie-Engine, Proxies inkl. Auth [E] |

**Bewertung C++ [I]:** Funktional stark (PoDoFo + OpenSSL + PDFium + zxing-cpp + HarfBuzz). Aber teure Toolchains für 5 Plattformen + WASM, Speichersicherheitsrisiken beim Parsen fremder PDFs und eine aufwendige Bindings-Schicht. Am ehesten als Fallback oder Referenz.

---

### 4.7 Kotlin Multiplatform (kurz)

- `com.tom-roush:pdfbox-android` 2.0.27.0 (**2023-01-02**, Apache-2.0) – nur Android, veraltet; Apache PDFBox 3.0.8 (2026-07-08) läuft nur auf der JVM [W].
- BouncyCastle Java `bcpkix-jdk18on` 1.86 (2026-09-11, MIT) – nur JVM/Android [W].
- `dev.whyoleg.cryptography` 0.6.0 (2026-04-02, Apache-2.0) – Primitive über Plattform-Provider, **kein CMS** [W].
- Für Kotlin/Native (iOS) gibt es keine PDF-Bibliothek → C-Interop (PDFium/PoDoFo) oder Eigenbau [I]. **Kein tragfähiger Pfad** für einen gemeinsamen Kern.

---

### 4.8 Kommerzielle Cross-Plattform-SDKs (nur zur Einordnung)

| SDK | Relevante Fähigkeiten | Lizenz | Quelle |
|---|---|---|---|
| Apryse (ex PDFTron) | „Custom Signing": `CreateSigDictForCustomSigning(filter, subfilter, size)`, Beispiel mit `ETSI.CAdES.detached` im PAdES-Modus, Incremental Save, Digest über ByteRange, Helfer für das ESS-signing-certificate-Attribut | proprietär | [S] Doku via Suche (docs.apryse.com) |
| Nutrient (ex PSPDFKit) | Web SDK `SigningData`: `signatureType` CMS/CAdES, `signatureContainer` **raw oder pkcs7** (pkcs7 = EXT-B); PAdES B-B/B-T/B-LT, **kein** B-LTA laut Blog | proprietär | [S] |
| Syncfusion (.NET/Flutter/JS) | externes/verzögertes Signieren, CAdES | proprietär, Community-Lizenz | [S]/[W] |
| iText 9 (Java/.NET) | Referenz für PAdES inkl. LTV | AGPL / kommerziell | [W] Lizenz, [E] Features |
| unipdf (Go), PDFluent (Rust) | Signieren/Prüfen | EULA / AGPL+kommerziell | [W] |

→ Für einen EUPL-Klon ungeeignet (Abschnitt 3), aber als **Interop-Gegenprobe** nutzbar [I].

### 4.9 Referenz- und Testwerkzeuge (werden nicht ausgeliefert)

| Werkzeug | Version (Datum) | Lizenz | Zweck |
|---|---|---|---|
| **pyHanko** + `pyhanko-certvalidator` | 0.37.0 (2026-08-31) / 0.32.1 | MIT | PAdES-Signieren/-Prüfen als Vergleich, Einbetten externer CMS (EXT-B) [E] |
| endesive | 2.19.3 (2026-01-12) | MIT | einfacher Gegentest |
| **EU DSS** (`eu.europa.ec.joinup.sd-dss:dss-pades`) | 6.5 (2026-08-29) | LGPL-2.1 [E] | ETSI-konforme Validierung (PAdES-Baseline-Profile) als Test-Orakel |
| PDF-AS 4.4.2 / PDF-Over selbst | – | EUPL | Golden Files: gleiche Eingabe → Ausgaben strukturell vergleichen [I] |

---

## 5. Querschnittsmatrix der kritischen Fähigkeiten

Legende: ✓ = dokumentiert/im Code gesehen [W] · (✓) = wahrscheinlich, nicht dokumentiert [I] · ✗ = nicht vorhanden · ? = nicht geprüft · ◐ = teilweise.

| Kandidat | IU | XS | ENC | EXT-A | EXT-B | SF frei | AP | WASM | iOS/Android | Lizenz EUPL |
|---|---|---|---|---|---|---|---|---|---|---|
| **lopdf + RustCrypto (Eigenbau)** | ✓ | ✓ (Update-Format ?) | ✓ | ✓ (`cms`) | Eigenbau, trivial [I] | ✓ | Eigenbau | ✓ | ✓ | ✓ |
| underskrift | ✓ | (✓) | ? | ✓ | ? | ✓ | ✓ | ? (tokio/reqwest abschaltbar) | (✓) | ✓ |
| oxidize-pdf | ✓ | ? | ✓ (R6 nur lesen) | – | ✓ | ? | ? | ? | (✓) | ✓ |
| digitorus/pdfsign | (✓) | (✓) | ? | ✓ | ✗ | ✓ | ✓ | ◐ | ◐ (gomobile) | ✓ |
| @libpdf/core | ✓ | ? | ✓ | ✓ | ✗ | ✓ | ✗ | ✓ | ◐ (WebView/RN) | ✓ |
| @signpdf + placeholder-plain | ◐ (nur ≤ PDF 1.3) | ✗ | ✗ | – | ✓ | ✓ | ✗ | ✓ | ◐ | ✓ |
| @cantoo/pdf-lib + pkijs (Eigenbau) | ✓ | ? | ✓ | ✓ (pkijs) | Eigenbau | ✓ | ✓ (Zeichen-API) | ✓ | ◐ | ✓ |
| pdf_cos/pdf_document (Dart) | ✓ | ✓ | ✓ | ? | ? | ✗ (nur adbe) | ✓ | ✓ | ✓ | ✓ |
| PDFsharp ≥ 6.5 | ✗ [I] | ? | ? | – | ✓ | ✗ | ✓ | ? | ◐ | ✗ (AGPL) |
| BouncyCastle .NET + eigener PDF-Writer | Eigenbau | Eigenbau | Eigenbau | ✓ | ✓ | ✓ | Eigenbau | ✓ | ✓ | ✓ |
| PoDoFo + OpenSSL | ✓ | ? | (✓) [E] | ✓ (deferred) | ✓ | ✓ | ✓ | ◐ (Emscripten) | ◐ | ✓ (MPL) |
| PDFium | ✓ (Flag) | ✓ | ✓ (lesen) | ✗ | ✗ | ✗ | ◐ | ✓ | ✓ | ✓ |
| qpdf | ✗ | ✓ | ✓ | ✗ | ✗ | – | – | ◐ | ◐ | ✓ |
| MuPDF / iText | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓/✗ | ✓/◐ | ✗ |

---

## 6. Fazit: bester Pfad für einen überall laufenden PAdES-Kern

### 6.1 Empfehlung [I]

**Rust-Kern** („pdfas-core"), UI-unabhängig, eingebunden über `tauri`/`uniffi`/`flutter_rust_bridge`/`wasm-bindgen`. Begründung:

1. **Vollständigkeit bei permissiven Lizenzen:** Für jede Kategorie (a)–(g) gibt es mindestens eine MIT/Apache/BSD/ISC-Crate (Abschnitt 4.1); kein GPL/AGPL-Zwang.
2. **Ein Code für alle Ziele inklusive WASM:** `lopdf`, RustCrypto, `subsetter`/`skrifa`, `rqrr`/`rxing`, `zune-jpeg`, `hayro-*` sind reines Rust. Nur PDFium (Vorschau) ist C++, liegt aber als Binärdatei für alle Plattformen vor.
3. **Die entscheidenden Low-Level-Fähigkeiten sind belegt:** `IncrementalDocument`, xref-Streams, inkrementelles Speichern verschlüsselter Dokumente (lopdf 0.44), CMS mit externem Digest und eigenen SignedAttributes (`cms` 0.2.3), asynchrone Signer (`cms` 0.3-rc).
4. **Vorlagen existieren:** `underskrift` (PAdES B-B…B-LTA auf `lopdf` + RustCrypto), `oxidize-pdf` (Vorbereiten/Einbetten externer CMS), `pdf_signing`. Diese Projekte lassen sich studieren oder forken, ohne dass man von ihrer Reife abhängt.
5. **Konsistent mit Recherche 01** (Rust-Kern + Tauri 2, Plan B Flutter-UI).

### 6.2 Vorgeschlagene Bausteine (Startpunkt, Versionen am Stichtag)

| Aufgabe | Crate(s) | Lizenz |
|---|---|---|
| PDF-Objektmodell, Parser, Incremental Writer, Verschlüsselung | `lopdf` 0.45 (Features `chrono` statt `chrono-clock`, kein `rayon` auf WASM) | MIT |
| Appearance-/Font-Objekte serialisieren | `pdf-writer` 0.15 (optional) | MIT/Apache |
| CMS/CAdES, X.509, ESSCertIDv2 | `cms` + `x509-cert` + `der` + `spki` + `const-oid` – **eine** Generation pinnen (0.2/0.7 stabil oder 0.3-rc/0.8 neu) | Apache/MIT |
| Signatur-Primitive | `rsa`, `p256`, `p384`, `ecdsa`, `sha2`, `sha1` (oder `ring` für RSA) | Apache/MIT, ISC |
| Keystores | `p12-keystore` 0.4 (PKCS#12); **JCEKS selbst implementieren** (siehe 6.3) | MIT/Apache |
| PAdES B-T/B-LT (später) | `x509-tsp`, `x509-ocsp`, optional `tsp-ltv` | Apache/MIT, BSD-2 |
| Fonts | `skrifa` + `subsetter` (+ AFM-Daten für Standard-14) | MIT/Apache |
| Bilder | `png`, `zune-jpeg`, `image`; `hayro-jpeg2000`, `hayro-jbig2`, `hayro-ccitt`; EXIF: `kamadak-exif` | MIT/Apache, BSD-2 |
| QR-Platzhalter | `rxing` (ZXing-nah, wie PDF-AS) oder `rqrr` | Apache-2.0 / MIT+ISC |
| Vorschau | `pdfium-render` + pdfium-binaries **oder** `hayro`; in einer Tauri-UI alternativ pdf.js | MIT/Apache, BSD/Apache |
| HTTP/HTML | `reqwest` 0.13 (`cookies`, `system-proxy`, TLS `rustls-no-provider` + `ring`), `scraper` | MIT/Apache, ISC |
| Bindings | `uniffi` (MPL-2.0), `flutter_rust_bridge` (MIT), `wasm-bindgen`, `tauri` | – |
| Tests | pyHanko, EU DSS, PDF-AS als Golden-Generator | MIT, LGPL (nur Test) |

### 6.3 Risiken und Eigenleistung (Rust-Pfad) [I]

- **Generationswechsel bei RustCrypto** (`der` 0.7→0.8, `cms` 0.3-rc): Abhängigkeiten zentral pinnen; bei 0.2 gibt es nur synchrone Signer (Remote-Signer als Zweiphasen-Ablauf modellieren).
- **`rsa`-Advisory ohne Fix:** lokal vertretbar, sonst RSA über `ring`/`aws-lc-rs`.
- **lopdf-Robustheit bei kaputten PDFs:** lopdf hat 2026 viele Fixes für manipulierte Dateien bekommen [W]. Ein Fuzzing-Korpus (PDFBox/pdf.js-Suites wie bei hayro) gehört ins Projekt.
- **Update-Format** (xref-Tabelle vs. xref-Stream, Hybrid-Dateien): Verhalten von `IncrementalDocument` gegen PDFBox `saveIncremental` testen.
- **Verschlüsselte Eingaben:** Neue Strings/Streams im Update verschlüsseln, `/Contents` nicht (Hinweis im Readme von `pdf_cos` [W]); die Rechteprüfung nach P6 nachbauen.
- **Eigenbau nötig:** AcroForm/Widget/Signaturfeld-Logik (P5), Tabellenlayout des Signaturblocks (V1), CID-Font-Zusammenbau (V3), ESSCertIDv2 und die Prüfung nach K3, das CMS-Container-Modell für SL (K4), PDF/UA-Struktur (P8).
- **PDF/A-Preflight (P7)** hat außerhalb von Java kein Gegenstück (veraPDF ist Java) → offene Frage.
- **JCEKS (K5):** Keine geprüfte Nicht-Java-Bibliothek liest JCEKS (BouncyCastle .NET nur JKS [W], andere nicht geprüft) → selbst implementieren oder Konvertierungsweg anbieten.
- **WASM-Grenzen:** Im Browser blockiert CORS den A-Trust-Flow und Private-Network-Regeln den Zugriff auf die lokale BKU. Der Kern läuft dort, die Konnektoren nicht ohne Server-Proxy (vgl. Recherche 01).

### 6.4 Rangfolge der Alternativen [I]

1. **Rust** – bester Gesamtpfad (siehe oben).
2. **Go** – schnellster Weg zu einem funktionierenden Signer (pdfsign); Mobile über gomobile ok, Web schwach; EXT-B per Fork.
3. **TypeScript** – ideal für Web/Electron und als UI-Schicht (pdf.js, pkijs, `@libpdf/core`), als Kern nur mit Forks (Appearance, EXT-B) und mit Beta-Risiko.
4. **C++ (PoDoFo)** – fachlich am vollständigsten unter den quelloffenen Nicht-JVM-Optionen, aber hohe Build- und Sicherheitskosten.
5. **.NET** – Krypto top, aber PDF-Schicht nach dem PDFsharp-Lizenzwechsel Eigenbau.
6. **Dart** – nur UI; Kern über Rust.
7. **Kotlin MP** – nicht tragfähig.

---

## 7. Offene Fragen

1. Schreibt `lopdf::IncrementalDocument` bei Dateien mit xref-Stream das Update als xref-Stream, und bleibt das Ergebnis mit Adobe Reader, PDFBox und DSS gültig? (Spike nötig.)
2. Reicht die PDF-AS-Standardgröße von 4 KiB für `/Contents` mit den heutigen ID-Austria/A-Trust-Zertifikatsketten, und welche Größe brauchen B-T/B-LT? (Gegen echte A-Trust-CMS messen.)
3. Liefert die A-Trust-/ID-Austria-Schnittstelle weiterhin nur vollständige CMS über SL 1.2 (Modell B), oder gibt es einen Hash-basierten Weg (Modell A, z. B. CSC-API)? Davon hängt ab, ob `underskrift`/`pdfsign`-artige Signer reichen.
4. Ist PDF/A-Preflight (P7) Pflicht, und wenn ja: welcher Ersatz ohne JVM (veraPDF-Server? eigene Teilprüfung?)?
5. JCEKS-Keystores: Wie viele Nutzer verwenden sie tatsächlich? Reicht PKCS#12 plus Konvertierungsanleitung?
6. Darf `arial.ttf` aus der PDF-AS-Konfiguration (`CFG:fonts/arial.ttf`) weiterverteilt werden? Wahrscheinlich nicht → Ersatzfont (z. B. Liberation Sans, metrisch kompatibel) einplanen.
7. Werden Konfiguration, Profile oder Logos aus PDF-AS (EUPL **1.1**-Lizenzdatei) bzw. PDF-Over (EUPL-1.2) übernommen? Das bestimmt die Lizenz des Klons.
8. EUPL-1.2 und App-Store-Vertrieb (iOS/Android): juristische Prüfung (vgl. Recherche 01, 5.5).
9. Reifeprüfung der jungen Projekte (`underskrift`, `@libpdf/core`, `pdf_cos`, `pdf_oxide`, `oxidize-pdf`) über einen Spike mit realen behördlichen PDFs (verschlüsselt, mehrfach signiert, xref-Stream, PDF/A).
10. Kann PKI.js Legacy-PKCS#12 (3DES/RC2) lesen? Nur relevant, falls Keystore-Signieren im Web angeboten werden soll.
11. Renderer-Wahl: Eine einheitliche Engine (PDFium) für pixelgenaue Platzierung, oder Plattform-Renderer (PDFKit/PdfRenderer) für kleinere Binärdateien? (Siehe Recherche 01, 5.4.)

---

## 8. Quellen

**Registries/APIs (Metadaten, 2026-10-14):** <https://crates.io/api/v1/crates/> · <https://registry.npmjs.org/> · <https://api.npmjs.org/downloads/> · <https://azuresearch-usnc.nuget.org/query> / <https://api.nuget.org/v3/> · <https://pub.dev/api/packages/> · <https://api.deps.dev/v3/>

**Rust:**
<https://github.com/J-F-Liu/lopdf> (README, CHANGELOG, `src/incremental_document.rs`) ·
<https://github.com/LaurenzV/hayro> · <https://github.com/typst/pdf-writer> · <https://github.com/LaurenzV/krilla> · <https://github.com/typst/subsetter> · <https://github.com/googlefonts/fontations> ·
<https://github.com/ajrcarey/pdfium-render> · <https://github.com/bblanchon/pdfium-binaries> ·
<https://github.com/RustCrypto/formats> (`cms/src/builder.rs`) · <https://crates.io/crates/cms> · <https://github.com/ancwrd1/p12-keystore> ·
<https://github.com/kushaldas/underskrift> · <https://docs.rs/pdf_signing/latest/pdf_signing/> · <https://github.com/bzsanti/oxidizePdf> · <https://github.com/yfedoseev/pdf_oxide> · <https://docs.rs/pdf_oxide/latest/pdf_oxide/> ·
<https://github.com/indygreg/cryptography-rs> · <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> · <https://rustsec.org/advisories/RUSTSEC-2025-0007.html> ·
<https://github.com/seanmonstar/reqwest> (`Cargo.toml`, `src/lib.rs`) · <https://github.com/WanzenBug/rqrr> · <https://github.com/rxing-core/rxing> · <https://github.com/mozilla/uniffi-rs> · <https://github.com/fzyzcjy/flutter_rust_bridge>

**TypeScript/JavaScript:**
<https://github.com/vbuch/node-signpdf> (README, `packages/utils/src/Signer.js`, `packages/signpdf/src/signpdf.js`) · <https://github.com/cantoo-scribe/pdf-lib> · <https://github.com/Hopding/pdf-lib> ·
<https://github.com/LibPDF-js/core> · <https://libpdf.documenso.com/docs/guides/signatures> · <https://libpdf.documenso.com/docs/concepts/incremental-saves> ·
<https://github.com/PeculiarVentures/PKI.js> · <https://github.com/digitalbazaar/forge> · <https://github.com/mozilla/pdf.js> · <https://github.com/embedpdf/embed-pdf-viewer> ·
<https://github.com/Sec-ant/zxing-wasm> · <https://github.com/zxing-js/library> · <https://github.com/cozmo/jsQR> · <https://github.com/lenchv/jks-js>

**Dart/Flutter:**
<https://github.com/DavBfr/dart_pdf> (`document_parser.dart`, `document.dart`, `obj/signature.dart`, `font/ttf_writer.dart`) · <https://github.com/ben-milanko/dart-pdf> ·
<https://pub.dev/packages/syncfusion_flutter_pdf> · <https://pub.dev/packages/syncfusion_flutter_pdf/license> · <https://help.syncfusion.com/document-processing/pdf/pdf-library/flutter/working-with-digital-signature> ·
<https://pub.dev/documentation/syncfusion_flutter_pdf/latest/pdf/PdfFileStructure/incrementalUpdate.html> · <https://pub.dev/packages/nkpdfsign> · <https://pub.dev/packages/dpdf> ·
<https://github.com/bcgit/pc-dart> · <https://pub.dev/packages/asn1lib> · <https://github.com/espresso3389/pdfrx> · <https://pub.dev/packages/flutter_zxing> · <https://pub.dev/packages/dio> · <https://pub.dev/packages/cupertino_http> · <https://pub.dev/packages/cronet_http>

**.NET:**
<https://www.nuget.org/packages/PDFsharp> · <https://docs.pdfsharp.net/General/License/License.html> · <https://docs.pdfsharp.net/PDFsharp/Topics/PDF-Features/Signatures.html> ·
<https://github.com/empira/PDFsharp> (`DigitalSignatureHandler.cs`, `IDigitalSigner.cs`, README) · <https://github.com/bcgit/bc-csharp> (`JksStore.cs`, `Pkcs12Store.cs`, `CMSSignedDataGenerator.cs`) ·
<https://www.nuget.org/packages/itext> · <https://github.com/UglyToad/PdfPig> · <https://github.com/Dtronix/PDFiumCore> · <https://github.com/micjahn/ZXing.Net> · <https://github.com/drewnoakes/metadata-extractor-dotnet> · <https://github.com/SixLabors/Fonts> (LICENSE)

**Go:**
<https://github.com/digitorus/pdfsign> · <https://pkg.go.dev/github.com/digitorus/pdfsign/sign> · <https://github.com/digitorus/pkcs7> · <https://github.com/digitorus/timestamp> ·
<https://github.com/pdfcpu/pdfcpu> · <https://pdfcpu.io/core/sign> · <https://github.com/unidoc/unipdf> · <https://github.com/klippa-app/go-pdfium> · <https://github.com/SSLMate/go-pkcs12> · <https://github.com/pavlo-v-chernykh/keystore-go> · <https://github.com/golang/mobile>

**C/C++:**
<https://github.com/podofo/podofo> (README: Features, Licensing, FAQ Signing) · <https://github.com/qpdf/qpdf> · <https://qpdf.readthedocs.io/en/stable/release-notes.html> ·
<https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_save.h> · <https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_signature.h> · <https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_edit.h> ·
<https://github.com/zxing-cpp/zxing-cpp> · <https://github.com/harfbuzz/harfbuzz> · <https://github.com/openssl/openssl> · <https://github.com/curl/curl>

**Kotlin/Kommerziell/Test:**
<https://github.com/TomRoush/PdfBox-Android> · <https://github.com/whyoleg/cryptography-kotlin> · <https://docs.apryse.com/core/guides/features/signature/custom-signing> ·
<https://www.nutrient.io/api/web/interfaces/DigitalSignatures.SigningData.html> · <https://www.nutrient.io/blog/complete-guide-digital-signatures/> ·
<https://github.com/MatthiasValvekens/pyHanko> · <https://github.com/m32/endesive> · <https://github.com/esig/dss>

**Lizenz:** EUPL-1.2-Volltext in `PDFOVER:LICENSE` (Art. 5 Z. 120-127, Anhang Z. 266-278) · <https://interoperable-europe.ec.europa.eu/collection/eupl/eupl-text-eupl-12>
