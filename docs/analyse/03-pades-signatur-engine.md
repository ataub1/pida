# 03 – PAdES-Signatur-Engine: Was PDF-AS beim Signieren exakt in die PDF-Datei schreibt

Stand der Analyse: 2026-10-14
Referenz: PDF-Over `main`, Commit `c07c5ec` (2026-09-03), Version `4.4.10-SNAPSHOT`; PDF-AS 4.4.2 (`pdf-as-4/build.gradle:21`, `pdf-over-signer/pom.xml:13`); PDFBox 2.0.34 zur Laufzeit von PDF-Over (`pom.xml:115`), PDF-AS selbst baut gegen PDFBox 2.0.32 (`pdf-as-4/pdf-as-pdfbox-2/build.gradle:36-38`).

Ziel dieses Dokuments: Die Signatur-Engine (PDF-AS + PDFBox + IAIK-CMS) so genau spezifizieren, dass sie ohne Java **funktional gleichwertig** und, wo sinnvoll, **strukturgleich** nachgebaut werden kann. Das Layout des sichtbaren Signaturblocks (Tabellen-Rendering, Positionierungsalgorithmus) und das A-Trust-Web-Protokoll werden hier nur so weit beschrieben, wie sie die PDF- und CMS-Struktur betreffen.

---

## 0. Lesehinweise

### 0.1 Pfadkonventionen und Abkürzungen

| Präfix | Bedeutung |
|---|---|
| *(kein Präfix)* | relativ zum PDF-Over-Repository-Root |
| `pdf-as-4/` | relativ zum PDF-AS-4-Repository |
| `cfgzip/` | relativ zur entpackten, von PDF-Over mitgelieferten PDF-AS-Konfiguration |
| `pdfbox:` | Klasse/Methode in `pdfbox-2.0.34.jar` (lokaler Gradle-Cache). Der Quellcode von PDFBox liegt nicht lokal vor. Das Verhalten wurde per `javap -c -p` aus dem Bytecode gelesen (keine Ausführung). |

Kürzel für Dateien in Fundstellen:

| Kürzel | Datei |
|---|---|
| `PPS` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/PADESPDFBOXSigner.java` |
| `PSW` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/PdfboxSignerWrapper.java` |
| `SDE` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/SignatureDataExtractor.java` |
| `PBO` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/PDFBOXObject.java` |
| `PBU` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/utils/PdfBoxUtils.java` |
| `VSB` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFAsVisualSignatureBuilder.java` |
| `VSP` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFAsVisualSignatureProperties.java` |
| `VSD` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFAsVisualSignatureDesigner.java` |
| `PTC` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFAsTemplateCreator.java` |
| `TDU` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/TableDrawUtils.java` |
| `PBF` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/PDFBoxFont.java` |
| `PAI` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/PdfAsImpl.java` |
| `ICC` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/api/IConfigurationConstants.java` |
| `BSL` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/util/BaseSLConnector.java` |
| `SCW` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/util/ISignatureConnectorSLWrapper.java` |
| `SU` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/util/SignatureUtils.java` |
| `CU` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/util/CertificateUtils.java` |
| `PU` | `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/utils/PDFUtils.java` |
| `SPS` | `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/settings/SignatureProfileSettings.java` |
| `PKS` | `pdf-as-4/signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESSignerKeystore.java` |
| `PS` | `pdf-as-4/signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESSigner.java` |
| `PC` | `pdf-as-4/signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESConstants.java` |
| `A4S` | `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4Signer.java` |
| `A4C` | `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4BKUSLConnector.java` |
| `A4P` | `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4SignatureParameter.java` |
| `MBC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/MobileBKUConnector.java` |
| `LBC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/LocalBKUConnector.java` |

Beispieldateien, die von PDF-AS signiert wurden. Sie liegen im PDF-AS-Repository und wurden **nur gelesen** (mit einem eigenen Python-Skript zerlegt, ohne Namen oder Zertifikatsinhalte auszugeben):

| Kürzel | Datei | Eigenschaften |
|---|---|---|
| `P1` | `pdf-as-4/pdf-as-web/src/test/resources/data/dummy-pdf-signed.pdf` | `/M` 2024-03-15; Dokument ohne AcroForm; neue Seite angehängt; PDF/A- und PDF/UA-Profil; RSA-Signatur (384 Byte, also RSA-3072); Original ist eine Hybrid-Datei (xref-Tabelle + `/XRefStm`) |
| `P2` | `pdf-as-4/pdf-as-pdfbox-2/src/test/resources/manySignFields_signed4.pdf` | `/M` 2021-07-22; vorhandenes leeres Signaturfeld wurde befüllt; reines xref-Stream-Dokument mit Object Streams; ECDSA |
| `P3` | `pdf-as-4/pdf-as-pdfbox-2/src/test/resources/new_qr_2_signed.pdf`, `…_signed_signed.pdf`, `…_signed_signed_signed.pdf` | `/M` 2021-07-20; QR-Platzhalter (`/Location`); 1 bis 3 aufeinanderfolgende Signaturen; ECDSA |

**Achtung:** Die Proben stammen aus älteren PDF-AS-/PDFBox-Ständen (2021–2024). Wo sie vom aktuellen Code abweichen, ist das vermerkt (Schlüsselreihenfolge, `/ID`-Hash, Hybrid-xref).

### 0.2 Verlässlichkeitskennzeichnung

- **[V]** im Quellcode von PDF-AS/PDF-Over verifiziert (mit Fundstelle).
- **[BC]** im Bytecode von `pdfbox-2.0.34.jar` bzw. im mitgelieferten IAIK-JAR per `javap` verifiziert.
- **[P]** an den Beispieldateien P1–P3 nachgemessen.
- **[E]** extern recherchiert (Spezifikation/Web, Stand 2026-10-14).
- **[I]** Interpretation oder Vermutung, nicht verifiziert.

Es wurde nichts gebaut und kein Referenzcode ausgeführt. In dieser Doku stehen keine Schlüssel, Passwörter, Zertifikatsinhalte oder Personendaten. Test-Keystores gibt es (`pdf-as-4/pdf-as-cli/src/test/resources/test.p12`, `pdf-as-4/pdf-as-tests/src/test/test-suites/public_pdfbox2/test.p12`, `pdf-over-gui/src/main/resources/keystore.jks`). Ihr Inhalt wurde nicht ausgewertet.

---

## 1. Kurzfassung

| Thema | Ergebnis |
|---|---|
| Signaturformat | PAdES mit `/Filter /Adobe.PPKLite`, `/SubFilter /ETSI.CAdES.detached` [V] `PC:3-4`, `PS:56-62`, `PKS:211-217` |
| Erreichtes Niveau | höchstens **PAdES-B-B** (CAdES-BES-artig): kein Zeitstempel, kein DSS/VRI, kein DocTimeStamp, kein LTV, keine DocMDP-Zertifizierung [V] (kein entsprechender Code; Suche ohne Treffer) |
| Schreibweise | **immer Incremental Update** (`PDDocument.saveIncremental`) [V] `PPS:394`. Das Original bleibt byte-identisch als Präfix erhalten [P]. |
| Platzhalter `/Contents` | Standard 4096 Byte (`0x1000`), konfigurierbar über `signatureSize`. Im PDF stehen 8192 Hexziffern in Großbuchstaben, nach dem CMS mit `0` aufgefüllt [V] `PPS:582-597`, [BC], [P] |
| `/ByteRange` | `[0 a b c]`: a = Offset von `<`, b = Offset nach `>`, b+c = Dateilänge. Bis zur ursprünglichen Platzhalterbreite mit Leerzeichen aufgefüllt [BC] [P] |
| Hash | SHA-256 über die Konkatenation beider ByteRange-Teile. Bei EC-Schlüsseln mit Feldgröße ≥ 512 Bit SHA-512, unter 256 Bit SHA-1 (nur Keystore) [V] `CU:34-75` |
| Keystore-CMS | SignedData v1, detached, nur das Signerzertifikat, SignerInfo v1 (IssuerAndSerialNumber). Signed Attributes: content-type, **mime-type (ETSI)**, **signing-time**, message-digest, signing-certificate-v2 (bei SHA-1: v1) [V] `PKS:144-209,343-356` [P] |
| SL-CMS (lokale BKU, A-Trust/ID Austria) | Das CMS wird **vollständig von der BKU** gebaut. PDF-AS sendet das **ganze PDF** mit genullter `/Contents`-Lücke plus `ExcludedByteRange`, nicht nur einen Hash [V] `BSL:85-162`, `A4C:115-117` |
| Prüfungen nach dem Signieren | Kryptografische Selbstprüfung des CMS gegen die ByteRange-Daten, genau ein SignerInfo, signing-certificate(-v2) muss vorhanden sein und passen, Zertifikat-Fingerprint muss dem vorher gelesenen Zertifikat entsprechen [V] `SU:26-137`, `SCW:100-123` |
| Verifikation in PDF-Over | **keine** (kein `verify`-Aufruf). Es gibt nur die implizite Selbstprüfung und für PDF/A-1-Profile einen Preflight, der ausschließlich loggt [V] |
| Feldname | `"PDF-AS Signatur" + n` (n = höchste vorhandene Nummer + 1) oder `<signature_field_name> + n` [V] `PPS:572-580`, `PBU:41-79` [P] |
| Bit-Kompatibilität | Byte-Gleichheit mit PDF-AS ist **nicht** erreichbar (Zeit, `/ID`, ECDSA-Zufall, CMS der BKU). Strukturgleichheit ist erreichbar und wird hier spezifiziert. |

---

## 2. Aufrufkette

```
PDF-Over                         PDF-AS                               PDFBox 2.0.34                  Signaturgerät
────────                         ──────                               ─────────────                  ─────────────
PdfAs4Signer.prepare()  ──────►  SignParameter (Config-Overlay,
                                 Profil, Position, Platzhalter)
PdfAs4Signer.sign()     ──────►  PdfAsImpl.sign()
                                   ├ getCertificate()  ───────────────────────────────────────────►  SL: InfoboxReadRequest
                                   │                                                                  KS: KeyStore-Eintrag
                                   └ PADESPDFBOXSigner.signPDF()
                                       ├ PDSignature, Visual-Template
                                       ├ doc.addSignature() ───────►  AcroForm/Feld/Widget/AP
                                       ├ Feld umbenennen, PDF/UA
                                       └ doc.saveIncremental() ────►  COSWriter: Increment schreiben,
                                                                      ByteRange patchen,
                                                                      SignatureInterface.sign(data)
                                          PdfboxSignerWrapper.sign ◄──┘
                                            └ IPlainSigner.sign(data, byteRange) ───────────────────►  SL: CreateCMSSignatureRequest
                                                                                                      KS: IAIK SignedData
                                                                      writeExternalSignature(cms) ◄── CMS (DER)
                                 SignResult ◄── signedDocument (byte[])
```

[V] `A4S:46-161`, `PAI:123-270`, `PPS:117-415`, `PSW:62-75`, [BC] `pdfbox:PDDocument#addSignature`, `pdfbox:COSWriter#doWriteSignature`.

Wichtig für den Nachbau: PDF-Over nutzt die **synchrone** API `PdfAs.sign()`. Das Signaturgerät wird **mitten im Schreiben** des Increments aufgerufen (aus `COSWriter.doWriteSignature`), und zwar blockierend, inklusive Benutzerinteraktion am Handy. Zu diesem Zeitpunkt sind alle Bytes außer `/Contents` endgültig festgelegt [BC]. PDF-AS hat zusätzlich eine asynchrone API (`startSign`/`process`/`finishSign`, [V] `PAI:317-479`), die PDF-Over aber nicht nutzt (siehe 6.4).

---

## 3. Ablauf Schritt für Schritt

### Phase A – Vorbereitung in PDF-Over

| # | Schritt | Fundstelle |
|---|---|---|
| A1 | Config-Overlay für den Lauf: `sig_obj.<Profil>.value.SIG_LABEL` (Bildmarke), `.value.SIG_NOTE`. Bei `BASE_LOGO` außerdem `padding=0`, `pos=w:…;f:0` und `imagescaletofit` | [V] `A4S:60-86` |
| A2 | `SignParameter` mit Eingabe-Bytes (`ByteArrayDataSource`) und Ausgabe-`ByteArrayOutputStream`, Positionsstring (`p:auto;x:auto;y:auto` bzw. `p:<n>;x:<f>;y:<f>` bzw. `p:new;…`), Profil-ID, Transaction-ID = zufällige UUID | [V] `A4S:88-97`, `A4P:133-154` |
| A3 | Bei Platzhaltersuche: `placeholder_mode=1`, `enable_placeholder_search=true`, optional `placeholder_id` | [V] `A4S:99-105` |
| A4 | Profil-ID = `<Profil>[_DE\|_EN][_NOTE][_PDFA]`, Sonderfälle `BASE_LOGO`, `INVISIBLE`, `AMTSSIGNATURBLOCK_<L>_RECOMMENDED` | [V] `A4P:156-190` |
| A5 | `sl.request.type` = `upload` (Standard, immer bei A-Trust) oder `b64` (lokale BKU mit `Server`-Header `trustDeskbasic`/`asignSecurityLayer`) | [V] `A4S:125-128`; `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/LocalBKUState.java:114-118`; `…/MobileBKUState.java:524` |
| A6 | Signer: `PAdESSigner(new PdfAs4BKUSLConnector(bkuConnector))` für lokale BKU und A-Trust, sonst `PAdESSignerKeystore(file, alias, kspass, keypass, type)` | [V] `A4S:130-139`; `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4SigningState.java:59-66` |

### Phase B – `PdfAsImpl.sign()`

| # | Schritt | Fundstelle |
|---|---|---|
| B1 | Parameter prüfen: Profil muss als Präfix `sig_obj.<id>` existieren (sonst 11009), Daten vorhanden (sonst 10001) | [V] `PAI:87-108` |
| B2 | „Preprocessing“: SPI-geladene `PreProcessor`-Plugins. Der einzige registrierte (`DebugPreProcessor`) ist in `META-INF/services` **auskommentiert**, also wird in der Praxis **nichts** vorverarbeitet | [V] `PAI:518-537`; `pdf-as-4/pdf-as-lib/src/main/resources/META-INF/services/at.gv.egiz.pdfas.lib.api.preprocessor.PreProcessor` |
| B3 | Backend `PDFBOXBackend` liefert den Signer `PADESPDFBOXSigner` | [V] `PAI:139`; `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/PDFBOXBackend.java:39-41` |
| B4 | PDF laden: `PDDocument.load(InputStream)` **ohne Passwort**. Ein PDF mit User-Passwort scheitert hier. PDF-Over fängt das schon vorher beim Öffnen ab („password protected and can therefore not be signed“) | [V] `PBO:51-62`; `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/PositioningState.java:76-88` |
| B5 | Berechtigungsprüfung (siehe 7) | [V] `PAI:159`, `PBU:17-39` |
| B6 | `RequestedSignature`: Profil = Parameter oder `sig_obj.type.default` (PDF-Over-Config: `SIGNATURBLOCK_SMALL_DE_NOTE`) | [V] `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/status/RequestedSignature.java:44-61`; `cfgzip/cfg/config.properties:103` |
| B7 | **Zertifikat holen, bevor das PDF verändert wird.** SL: `InfoboxReadRequest` (`Certificates`/`SecureSignatureKeypair`). Keystore: aus dem KeyStore. Danach Gültigkeitsprüfung `notBefore ≤ jetzt ≤ notAfter`, sonst Fehler 11021. Das Zertifikat liefert `/Name` und die Inhalte des Signaturblocks | [V] `PAI:170-171,272-286`; `SCW:66-90`; `BSL:71-83` |
| B8 | `signPDF(pdfObject, requestedSignature, new PdfboxSignerWrapper(plainSigner, …))` | [V] `PAI:212-215`; `PPS:722-725` |
| B9 | Signiertes Dokument (`byte[]`) in den Output-Stream schreiben, Position zurückgeben | [V] `PAI:539-552` |

### Phase C – `PADESPDFBOXSigner.signPDF()`

| # | Schritt | Fundstelle |
|---|---|---|
| C1 | Vorhandenes Signaturfeld suchen: Immer `doc.getDocumentCatalog().getAcroForm()` (mit PDFBox-Fixup, siehe 4.3), dann `acroForm.getField(signature_field_name)`. Ohne konfigurierten Namen wird kein Feld gefunden. Ist es schon signiert, folgt `IllegalStateException("The signature field … is already signed.")`. Sonst wird ein neues `PDSignature` erzeugt und als `/V` ins Feld gesetzt (`isAdobeSigForm=true`) | [V] `PPS:144-152,878-897` |
| C2 | `PDSignature` (Konstruktor setzt `/Type /Sig` [BC]); `setFilter(Adobe.PPKLite)`, `setSubFilter(ETSI.CAdES.detached)`, `setSignDate(Calendar.getInstance())` → `/M` | [V] `PPS:147,155-157` |
| C3 | QR-Platzhalter: wenn gefunden, `/Location` = Platzhaltername (z. B. Bild-Ressourcenname `Image5` [P] P3). Optional überschreibt das Platzhalter-Profil das angefragte Profil (`placeholder_profile_overwrite`, Standard true) | [V] `PPS:161-179` |
| C4 | `/Name` = aufgelöster Profilwert `SIG_SUBJECT`, Standard `${subject.title != null ? (subject.title + " ") : ""}${subject.CN}`. Hat das Profil keinen solchen Eintrag (z. B. `INVISIBLE`), ist der Wert `null` und es wird kein `/Name` geschrieben [I] | [V] `PPS:186-189`; `cfgzip/cfg/profiles/BASE_NORMAL.properties:43`; `SPS:166-183` |
| C5 | `/Reason` = `adobeSignReasonValue` des Profils, sonst `"PAdES Signature"` | [V] `PPS:192-194`; `SPS:189-191`; `cfgzip/cfg/profiles/BASE_NORMAL.properties:57`; [P] P1 |
| C6 | PDF/A-Version aus XMP (`pdfaid:part`) lesen, nur wenn das Profil PDF/A-fähig ist | [V] `PPS:200-203,850-875` |
| C7 | Reservierte Größe `signatureSize` (Profilwert, Fallback `default.signatureSize`, sonst `0x1000`) → `SignatureOptions.setPreferredSignatureSize` | [V] `PPS:206,582-597`; `ICC:114` |
| C8 | Nur bei sichtbarer Signatur: Position bestimmen, eventuell **neue Seite** anhängen (nicht im Formularfeld-Fall), Signaturblock als eigenes Template-PDF bauen (4.7). Bei PDF/A-Profil `/OutputIntents` setzen (4.10). `options.setPage(page-1)`, `options.setVisualSignature(templateBytes)` | [V] `PPS:210-293` |
| C9 | `doc.addSignature(signature, signer, options)` → PDFBox, Phase D | [V] `PPS:295` |
| C10 | Feldnamen bilden und setzen (`/T`), Alternativnamen `/TU` setzen (nicht im Formularfeld-Fall) | [V] `PPS:297-340` |
| C11 | PDF/UA-Strukturelemente einfügen, wenn ein Strukturbaum existiert (4.9) | [V] `PPS:342-350,427-570` |
| C12 | `performTechnicalSignature`: `doc.saveIncremental(bos)` (Phase E), Ergebnis als `signedDocument`. Danach bei PDF/A-1-Profil Preflight, der nur loggt | [V] `PPS:352,389-415,669-714` |

### Phase D – `PDDocument.addSignature(sig, iface, options)` in PDFBox [BC]

1. Nur eine Signatur pro `PDDocument`-Instanz („Only one signature may be added in a document“).
2. `/Contents` = `new byte[preferredSize]`, also Nullbytes, ohne Angabe 9472. `setContents` setzt `COSString.setForceHexForm(true)`.
3. `/ByteRange` = `[0 1000000000 1000000000 1000000000]` (`RESERVE_BYTE_RANGE`). Das Array ist direkt eingebettet (`setDirect(true)`).
4. Leeres Dokument (0 Seiten): `IllegalStateException("Cannot sign an empty document")`.
5. `catalog.getAcroForm(null)` (**ohne** Fixup). Der Katalog wird als geändert markiert. Fehlt ein AcroForm, wird `new PDAcroForm(doc)` angelegt (`/Fields []`) und im Katalog gesetzt. Sonst wird das AcroForm als geändert markiert.
6. Gibt es ein `/Fields`-Array, wird es als geändert markiert und ein Feld gesucht, dessen `/V` **dasselbe Objekt** wie das neue Signaturdictionary ist (`findSignatureField`). So wird das in C1 vorbereitete Formularfeld gefunden. Ohne `/Fields` wird ein leeres Array gesetzt.
7. Kein Feld gefunden: `new PDSignatureField(acroForm)`. Der Konstruktor setzt `/FT /Sig`. Feld und Widget sind **dasselbe Dictionary**, der Konstruktor ergänzt `/Type /Annot`, `/Subtype /Widget`, `/F` mit Locked (128) und Print (4) = 132, sowie `/T (SignatureN)`. Danach `setValue(sig)` → `/V`, Widget-`/P` = Seite `min(max(options.page,0), n-1)`. Feld gefunden: Signaturdictionary als geändert markieren, Seite = `null` (die `/Annots` werden dann nicht angefasst).
8. `widget.setPrinted(true)` (auch bei vorhandenen Feldern).
9. `acroForm.getCOSObject().setDirect(true)`, `/SigFlags` |= 1 (SignaturesExist) und |= 2 (AppendOnly), also **3**.
10. Neues Feld an `/Fields` anhängen. Bei vorhandenem Feld nur „geändert“ markieren.
11. Ohne Visual-Template: `/Rect [0 0 0 0]` und `/AP << /N <leerer Form-XObject-Stream mit /BBox [0 0 0 0]> >>`.
    Mit Template: Im Template-COSDocument das **erste** Dictionary mit `/Type /Annot` suchen und dessen `/Rect` übernehmen, aber **nur**, wenn das Widget noch kein 4-elementiges `/Rect` hat (vorhandene Felder behalten ihr Rechteck). Das erste Dictionary mit `/FT /Sig` und `/AP`-Dictionary liefert `/AP` (direkt eingebettet) und `/DR`: Hat das AcroForm kein `/DR`, wird das Template-`/DR` übernommen (direkt). Sonst werden nur die `/XObject`-Einträge gemischt, und nur wenn beide Seiten ein `/XObject`-Dictionary haben. Fehlt eines davon: „Template is missing required objects“.
12. Bei neuem Feld: `page.setAnnotations(page.getAnnotations())`. Damit wird `/Annots` zu einem **direkten** Array in der Seite. Widget anhängen, falls noch nicht enthalten, Seite als geändert markieren.

### Phase E – `saveIncremental` und `COSWriter` in PDFBox [BC]

1. `subsetDesignatedFonts()`. Zum Subsetten registrierte Schriften werden hier subsetted. Die Schriften des Signaturblocks sind allerdings schon beim Speichern des Templates subsetted worden (4.7).
2. `COSWriter(output, originalInput)` im Incremental-Modus. Ohne eingelesene Quelle: „document was not loaded from a file or a stream“.
3. `/ID` im Trailer: Das erste Element bleibt, falls vorhanden. Das zweite wird neu berechnet als **SHA-256**(Zeit in ms als Dezimalstring + `toString()` aller `/Info`-Werte). In 2.0.34 sind das 32 Byte [BC]. Ältere Stände nutzten MD5 mit 16 Byte [P] P1/P2. Fehlt `/ID`, werden beide Elemente auf denselben neuen Wert gesetzt.
4. Increment-Beginn: `\r\n` (`writeCRLF`), dann die Objekte [BC] [P]. Bei P2 stand vor diesem `\r\n` noch ein zweites `\r\n` [P], vermutlich vom Dateiende des Originals.
5. Geschrieben werden: `/Root` und `/Info` als Startobjekte, dann rekursiv alle Objekte mit `needToBeUpdated=true` oder **ohne** bisherige Objektnummer (neu). Neue Objekte bekommen Nummern ab der bisherigen Höchstnummer + 1 [P] (P1: Original `/Size 83`, neue Objekte 83–115, xref-Stream 116). **Voraussetzung:** Geänderte Objekte müssen über eine Kette geänderter Objekte vom Katalog aus erreichbar sein. Darum markiert PDF-AS so viele Objekte explizit [I].
6. Erkennung des Signaturdictionarys: Dictionary mit `/Type /Sig` oder `/DocTimeStamp`, dessen `/ByteRange` 4 Integer hat und für das `br[2]+br[3] > Länge des Originals` gilt (der Platzhalter). Beim Schreiben werden Offset und Länge von `/Contents` (inklusive `<` und `>`) sowie Offset und Länge des ByteRange-Inhalts gemerkt.
7. Formatierung: Dictionary `<<\n`, je Eintrag `/Key Wert\n`, `>>\n`. Arrays `[a b c]`, nach 10 Elementen ein Zeilenumbruch. Strings als `(...)` mit Escaping von `( ) \`, sobald ein Byte ≥ 0x80 oder CR/LF vorkommt als `<HEX>` (Großbuchstaben). Streams `stream\r\n…\r\nendstream` [P].
8. xref: Hatte das Original eine **xref-Tabelle** → klassische Tabelle mit `trailer << … /Prev <alter startxref> >>`. War es ein **xref-Stream** → neuer xref-Stream (`/Type /XRef /W [1 3 0] /Filter /FlateDecode /Index […] /Prev …`, plus `/Root /Info /ID /Size`) [P] P2. Bei einer **Hybrid-Datei** (`/XRefStm` im Trailer) werden **beide** geschrieben: zuerst ein xref-Stream, dann Tabelle und Trailer. Der Code setzt dabei im Trailer `/XRefStm` = Offset des neuen Streams [BC]. Die ältere Probe P1 zeigt beide Teile, aber **ohne** `/XRefStm` im neuen Trailer. Hier gibt es eine Versionsabweichung [P]. Object Streams werden **nie** geschrieben, geänderte Objekte aus einem ObjStm landen als normale Objekte im Increment [I] (konsistent mit P2).
9. `startxref\n<offset>\n%%EOF\n`.
10. `doWriteSignature`: `beforeLength = signatureOffset`, `afterOffset = signatureOffset + signatureLength`, `afterLength = Gesamtlänge − afterOffset`. String `"0 " + before + " " + afterOffset + " " + afterLength + "]"` über den Platzhalterinhalt schreiben, Rest mit `0x20` auffüllen. Passt der String nicht: „Can't write new byteRange …“.
11. `getDataToSign()` = Original ‖ Increment-Teil ohne den `/Contents`-Wert (`<…>`). Das ist genau die ByteRange-Konkatenation.
12. `signatureInterface.sign(dataToSign)` → PDF-AS (Phase F).
13. `writeExternalSignature(cms)`: `Hex.getBytes(cms)` in Großbuchstaben. Ist `hex.length > signatureLength − 2`, gibt es eine IOException „Can't write signature, not enough space; adjust it with SignatureOptions.setPreferredSignatureSize“. Sonst ab `signatureOffset+1` ins Increment kopieren, die restlichen Platzhalter-`0` bleiben. Ausgabe = Original + Increment.

### Phase F – Signaturgerät (`IPlainSigner.sign(input, byteRange, …)`)

- `PdfboxSignerWrapper.sign`: liest den Datenstrom vollständig in ein `byte[]` (= ByteRange-Konkatenation) und **parst die ByteRange aus genau diesen Daten**. Dazu sucht er von hinten nach dem Byte-Muster `\n/Contents \n/ByteRange [`, das nur entsteht, weil der ausgeschnittene `<…>`-Wert fehlt. Dann ruft er `signer.sign(data, byteRange, …)` auf [V] `PSW:62-75`, `PU:40-48,151-186`.
  - **Konsequenz [I]:** Im Signaturdictionary müssen `/Contents` und `/ByteRange` **direkt aufeinander folgen**, mit genau dieser Formatierung. Bei PDFBox ist das durch die Einfügereihenfolge (erst `setContents`, dann `setByteRange`) garantiert [BC]. Ein Nachbau braucht diesen Trick nicht, wenn er die ByteRange direkt weitergibt.
- Keystore → 5.1. SL → 6.2.

---

## 4. PDF-Strukturen

### 4.1 Signaturdictionary (`/Type /Sig`)

Indirektes Objekt im Increment. Schlüsselreihenfolge im aktuellen Code (entspricht der Probe P1) [V] `PPS:147-194` [BC] [P]:

| # | Key | Wert | Herkunft |
|---|---|---|---|
| 1 | `/Type` | `/Sig` | `PDSignature()`-Konstruktor [BC] |
| 2 | `/Filter` | `/Adobe.PPKLite` | `IPlainSigner.getPDFFilter()` [V] `PS:60-62` |
| 3 | `/SubFilter` | `/ETSI.CAdES.detached` | `getPDFSubFilter()` [V] `PS:56-58`. `adbe.pkcs7.detached` wäre mit `PKCS7DetachedSigner` möglich, PDF-Over nutzt es nicht |
| 4 | `/M` | `(D:YYYYMMDDHHmmSS+HH'mm')`, lokale Zeitzone, z. B. `(D:20240315193333+01'00')` | `Calendar.getInstance()` zu Beginn von `signPDF`, also **vor** der Benutzerinteraktion [V] `PPS:157` [P] |
| 5 | `/Location` | Platzhaltername | nur bei QR-Platzhalter [V] `PPS:167` [P] P3 |
| 6 | `/Name` | Name aus dem Zertifikat (`SIG_SUBJECT`) | [V] `PPS:186-189`. Fehlt bei `null` [I] |
| 7 | `/Reason` | z. B. `(Signaturpruefung unter https://www.signaturpruefung.gv.at)` | [V] `PPS:192-194` |
| 8 | `/Contents` | `<` + 2·N Hexziffern (Großbuchstaben) + `>`, N = 4096 Standard | [BC] [P] |
| 9 | `/ByteRange` | `[0 a b c]` + Leerzeichen-Padding | [BC] [P] |

Nicht gesetzt [V] (Suche ohne Treffer): `/ContactInfo`, `/Prop_Build`, `/Reference` (keine DocMDP/FieldMDP-Transformation), `/Cert`, `/R`, `/V`.
Kodierung: ASCII → Literal-String. Umlaute → PDFDocEncoding und damit Hex-String. Zeichen außerhalb von PDFDocEncoding → UTF-16BE mit BOM, ebenfalls hex [I] (PDFBox-Standardverhalten `COSString(String)`).
Ältere Stände hatten eine andere Reihenfolge (P3: `/Location /Name /M /Reason`; P2: `/Name /M /Reason`) [P]. Die Reihenfolge ist also funktional irrelevant.

### 4.2 Byte-Layout von `/Contents` und `/ByteRange`

Tatsächlich geschrieben (P1, Objektnummer und Offsets beispielhaft) [P]:

```
89 0 obj\n
<<\n
/Type /Sig\n
/Filter /Adobe.PPKLite\n
/SubFilter /ETSI.CAdES.detached\n
/M (D:20240315193333+01'00')\n
/Name (…)\n
/Reason (PAdES Signature)\n
/Contents <3082077F06092A86…000000>\n        ← '<' bei Offset a = br[1], '>' bei Offset br[2]-1
/ByteRange [0 80285 96479 71069]               \n   ← 36 Zeichen zwischen '[' und '\n' (= Platzhalterbreite), Rest Leerzeichen
>>\n
endobj\n
```

Regeln [BC] [P]:
- `br[0] = 0`, `br[1]` = Offset von `<`, `br[2]` = Offset direkt nach `>`, `br[2] + br[3]` = Dateilänge (in allen Proben nachgerechnet).
- Hexwert = DER des CMS in Großbuchstaben, danach `0` bis zum Ende. In den Proben: CMS mit 1919–2156 Byte bei 4096 bzw. 8096 reservierten Byte, Rest durchgehend `00`.
- Der Platzhalter `[0 1000000000 1000000000 1000000000]` ist 36 Zeichen lang. Der endgültige Wert wird linksbündig eingesetzt und mit Leerzeichen aufgefüllt. Der Zeilenumbruch danach bleibt erhalten.
- P1 zeigt 8096 statt 4096 reservierte Byte. Für die Web-Tests war `signatureSize` vermutlich anders konfiguriert [I]. In der PDF-Over-Konfiguration ist `signatureSize` nirgends gesetzt, es gilt also 4096 [V] (Suche in `cfgzip/cfg` ohne Treffer).

### 4.3 AcroForm

| Key | Neues AcroForm (Dokument hatte keins) | Vorhandenes AcroForm | Fundstelle |
|---|---|---|---|
| Lage | **direkt** im Katalog eingebettet (`/AcroForm << … >>`) | bleibt indirekt, wird als neue Version des Objekts geschrieben | [BC] [P] P1 (direkt), P2 (indirekt `62 0 R`) |
| `/Fields` | `[<neues Feld>]` | bisherige Liste + neues Feld. Bei vorhandenem Feld unverändert | [BC] [P] |
| `/SigFlags` | `3` | `3` (OR-Verknüpfung mit dem alten Wert) | [BC] [P] |
| `/DR` | `<< /XObject << /FRM1 <inner form> >> /ProcSet [/PDF /Text /ImageC /ImageB /ImageI] /Font <</Helv …/ZaDb …>> >>` (direkt). Die Einträge `/XObject` und `/ProcSet` kommen aus dem Template-Widget, `/Font` aus dem PDFBox-Fixup | `/Font` fehlende `/Helv`/`/ZaDb` werden ergänzt. `/XObject` wird nur gemischt, wenn beide Seiten eins haben | [BC] `pdfbox:AcroFormDefaultsProcessor#verifyOrCreateDefaults`, `pdfbox:PDDocument#assignAcroFormDefaultResource` [P] |
| `/DA` | `(/Helv 0 Tf 0 g )` | nur gesetzt, wenn leer oder nicht vorhanden | [BC] [P] |
| `/NeedAppearances` | nicht gesetzt | **Seiteneffekt [BC]:** Stand es auf `true`, generiert der PDFBox-Fixup Appearance-Streams (`refreshAppearances`) und setzt es auf `false` | [BC] `pdfbox:AcroFormGenerateAppearancesProcessor#process` |

Wann greift der Fixup? Bei **jedem** Aufruf von `PDDocumentCatalog.getAcroForm()` ohne Argument wird ein neuer `AcroFormDefaultFixup` angewendet [BC]. PDF-AS ruft das **immer** vor `addSignature` auf (`PPS:144,881`, unabhängig davon, ob ein Feldname konfiguriert ist) und danach noch einmal (`PPS:302`, `PPS:343`). Ein vorhandenes AcroForm wird also immer „gefixt“, ein frisch angelegtes durch den zweiten Aufruf [P].
[I] Ob die von `refreshAppearances` erzeugten Widget-Appearances im Increment landen, hängt davon ab, ob PDFBox die Widgets als geändert markiert. Das ist unklar und sollte getestet werden.

### 4.4 Neues Signaturfeld und Widget (Standardfall)

Verschmolzenes Feld- und Widget-Dictionary, indirekt, Reihenfolge laut Probe P1 [BC] [P]:

```
84 0 obj
<<
/FT /Sig
/Type /Annot
/Subtype /Widget
/F 132                         % Print (4) + Locked (128)
/T (PDF-AS Signatur1)          % zuerst „Signature1“ (PDFBox), dann von PDF-AS umbenannt
/V 89 0 R                      % Signaturdictionary
/P 87 0 R                      % Seite
/Rect [50.0 745.4 562.0 800.0] % [x, yTop−h, x+w, yTop] (Tabellenbreite/-höhe), unsichtbar: [0 0 0 0]
/AP << /N 90 0 R >>            % direkt eingebettetes AP-Dictionary
/TU <…>                        % Alternativtext als Hex-String (wegen \n), siehe unten
/StructParent 2                % nur bei PDF/UA-Injektion (4.9)
>>
```

- `/T`: `buildNextSignatureFieldName` = `(signature_field_name ?: "PDF-AS Signatur") + countSignatures(doc, prefix)`. `countSignatures` durchläuft **nur die oberste Ebene** von `/Fields`, betrachtet Felder mit `/FT /Sig` und einem `/T`, das mit dem Präfix beginnt, parst den Rest als Integer und liefert das Maximum + 1 [V] `PPS:572-580`, `PBU:41-79` [P] (`PDF-AS Signatur1`, `…2`, `…3` in P3).
- `/TU`: bei sichtbarer Signatur der Text aller Caption- und Value-Zellen der **äußersten** Signaturtabelle, je Zelle mit `\n` abgeschlossen. Durch die `\n` wird es ein Hex-String. Bei unsichtbarer Signatur ist `/TU` = Feldname [V] `PPS:331-335`, `TDU:85,113-123,161,696-699`. Die Profil-Doku beschreibt dafür `adobeSignFieldValue`, das wird im Code aber **nicht** verwendet [V] (`SPS:193-195` ohne Aufrufer; `pdf-as-4/doc/PDFAS4_Profile_Dokumentation.docx`, Abschnitt Profilparameter).
- Die Seite bekommt `/Annots` als direktes Array mit dem Widget [BC] [P] (P1: `/Annots [84 0 R]`).

### 4.5 Vorhandenes, leeres Signaturfeld („Adobe-Signaturfeld“)

Wird aktiviert, wenn `signature_field_name` gesetzt ist und ein Feld dieses (voll qualifizierten) Namens existiert. PDF-Over schreibt den in der GUI gewählten Feldnamen dazu **persistent** in `~/.pdf-over/cfg/advancedconfig.properties` [V] `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/OpenState.java:66,138-176,239-251`. Die Kandidaten liefert `findEmptySignatureFields`: Felder der obersten Ebene mit `PDSignatureField`, Partial-Name ≠ null und ohne `/V` [V] `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/placeholder/SignatureFieldsAndPlaceHolderExtractor.java:18-35`.

Änderungen am vorhandenen Feld-/Widget-Dictionary [V] [BC] [P] P2:
- `/V` wird **hinzugefügt** und landet in der Reihenfolge ganz am Ende.
- `/AP` wird durch die PDF-AS-Appearance **ersetzt**, wenn das Profil sichtbar ist. `/Rect` bleibt (vorhandenes 4-elementiges Rechteck) → der Viewer skaliert die Appearance (`/BBox`) in das vorhandene Rechteck. P2 zeigt dabei einen extremen Fall: `/Rect` ist etwa 3 × 224 pt groß, die `/BBox` 232 × 84 pt.
- `/F`: Print-Bit wird gesetzt, **Locked wird nicht** gesetzt (P2: `/F 4`).
- `/T` bleibt unverändert, es gibt kein `/TU` und keine Umbenennung [V] `PPS:299-300`.
- `/P`, `/Annots` werden nicht angefasst. Die Positionsberechnung läuft zwar (`PPS:223-224`), eine neue Seite wird aber nicht angelegt.
- [I] Risiko: Ist das Widget ein **eigenes Kind-Dictionary** (Feld mit `/Kids`), wird nur das Feld als geändert markiert, nicht das Widget. Dann fehlt die neue `/AP` im Increment. Zu testen.
- [I] Nebenwirkung in PDF-Over: Weil der Feldname persistent gespeichert wird, bekommen spätere **neue** Felder (in Dokumenten ohne dieses Feld) den Namen `<gespeicherter Name><n>`, z. B. `Signature_41` [V] `PPS:572-580`. Ob der Wert in derselben Sitzung wirkt, hängt davon ab, wann das PDF-AS-Singleton die Konfiguration liest (`pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4Helper.java:40-58`).

### 4.6 Unsichtbare Signatur (Profil `INVISIBLE`, `isvisible=false`)

[V] `cfgzip/cfg/profiles/INVISIBLE.properties:8-10`, `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/configuration/SignatureProfileConfiguration.java:64-80`, [BC] `pdfbox:PDDocument#prepareNonVisibleSignature`:
- Widget auf Seite `options.page` = 0, also der **ersten** Seite (`setPage` wird nur im sichtbaren Zweig aufgerufen, `PPS:290`).
- `/Rect [0 0 0 0]`. `/AP << /N <Stream> >>`: leeres Form-XObject mit `/BBox [0 0 0 0]` [I zu `/Type /XObject /Subtype /Form`].
- `/F 132`, `/T PDF-AS Signatur<n>`, `/TU` = Feldname, kein `/Name` im Signaturdictionary [I] (siehe C4).

### 4.7 Appearance-Stream des sichtbaren Signaturblocks

PDF-AS baut den Block als **separates Template-PDF** mit eigenem AcroForm, Feld, Widget und AP. Das wird gespeichert (`template.save`) und in der `SignatureOptions` wieder als COSDocument geparst. PDFBox übernimmt daraus nur `/Rect`, `/AP` und `/DR` (Phase D, Schritt 11) [V] `PTC:58-198`, `VSP:116-125`. Die Werte, die nur im Template stehen (`/SigFlags 3`, `/DA (/sylfaen 0 Tf 0 g)`, ein Template-`PDSignature` mit 4096 Byte, `/V`), gelangen **nicht** ins Ergebnis [V] `VSB:377-402` [P].

Struktur im Ergebnis-PDF [V] [P] P1, P2:

```
Widget /AP << /N 90 0 R >>
90 0 obj  % „Holder Form“
<< /Length 39 /Type /XObject /Subtype /Form
   /Resources << /XObject << /FRM1 85 0 R >> /ProcSet [/PDF /Text /ImageC /ImageB /ImageI] >>
   /BBox [513.0 55.600002 0.0 0.0]          % [w+1 h+1 0 0] – absichtlich „vertauschte“ Ecken (normalisiert)
   /FormType 1
   /Matrix [1.0 0.0 0.0 1.0 0.0 0.0]        % Rotation um (Signaturrotation + Seitenrotation)
>>
stream\r\n q 1.0 0.0 0.0 1.0 0.0 0.0 cm /FRM1 Do Q \r\nendstream     % unkomprimiert
85 0 obj  % „Inner Form“ (zugleich /DR /XObject /FRM1 im AcroForm)
<< /Length … /Type /XObject /Subtype /Form
   /Resources << /XObject << /Im1 … >> /Font << /F1 … /F2 … >> /ProcSet [...] >>
   /BBox [513.0 55.600002 0.0 0.0] /FormType 1 >>
stream … Tabellen-Zeichenoperatoren (re/f, BT/Tf/Tj/ET, cm/Do) … endstream   % unkomprimiert
```

Details:
- ProcSet `[/PDF /Text /ImageC /ImageB /ImageI]` steht im Holder, im Inner und auf der Template-Seite [V] `VSB:95-105,288-304`.
- Holder-Inhalt: `"q " + m00 + " " + m10 + " " + m01 + " " + m11 + " " + m02 + " " + m12 + " cm /" + innerName + " Do Q"`. Die Affine ist immer die Identität `{1,0,0,1,0,0}`, und Java-`double`-Formatierung ergibt `1.0 0.0 …`. Länge 39 [V] `VSB:306-353`, `VSD:26` [P].
- `/BBox` = Formatter-Rechteck aus `{0,0,w+1,h+1}`, wobei die Ecken als `ur=(0,0)` und `ll=(w+1,h+1)` gesetzt werden [V] `VSP:107-109`, `VSB:507-522`.
- `/Matrix` des Holders = `AffineTransform.rotate(rotation + pageRotation)` [V] `VSB:549-567`. `/Rect` wird bei Seitenrotation 90/180/270 zusätzlich verschoben [V] `VSB:404-487`. Die Details gehören zur Positionierung.
- Inner-Inhalt: Der Tabelleninhalt wird mit `PDPageContentStream` auf die Template-Seite gezeichnet, in einen neuen, ungefilterten Stream kopiert und als UTF-8 zurückgeschrieben. Der Inhalt ist reines ASCII (nicht-ASCII-Text erscheint als Hex-String) [V] `VSB:253-286,336-348` [I].
- Bilder: `LosslessFactory.createFromImage` → `/Subtype /Image /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors 3 /BitsPerComponent 8 /Columns w >> /ColorSpace /DeviceRGB`. Bei PDF/A wird der Alphakanal entfernt, also keine `/SMask` [V] `VSB:226-243` [P].
- Schriften: Standard-14 (`HELVETICA`, `COURIER`, `TIMES_ROMAN`) werden nicht eingebettet. `TTF:<datei>` → `PDType0Font.load` (Subset, `/Identity-H`, `CIDFontType2`, `/ToUnicode`, `/CIDToGIDMap`, `FontFile2`, `CIDSet`, Subset-Präfix `AAAxxx+`) [V] `PBF:159-196` [P]. Beim PDF/A-3-Profil wird `/CIDSet` aus den Font-Deskriptoren entfernt [V] `PTC:168-186`, `VSB:666-692`.
- Ressourcennamen: `FRM1`, `Im1`, `F1`/`F2` [P].
- Für `buildPreviewOnly` (Vorschaubild in PDF-Over) wird das Holder-Stream-Kommando nicht geschrieben [V] `PTC:147-151`.

### 4.8 Neue Seite

Bei `p:new` oder wenn der Platz nicht reicht (Positionierung liefert `makeNewPage`), und nicht im Formularfeld-Fall [V] `PPS:227-237`:
`new PDPage(lastPage.getMediaBox())`, `/Resources` = leeres Dictionary, `/Rotate` = Rotation der letzten Seite, angehängt mit `doc.addPage`. `/Pages` wird als geändert markiert und mit neuem `/Count` und erweitertem `/Kids` neu geschrieben [P] P1 (`/Count 3 /Kids [3 0 R 26 0 R 87 0 R]`; Seite `<< /Type /Page /MediaBox […] /Resources 95 0 R /Rotate 0 /Parent 2 0 R /Annots [84 0 R] /Tabs /S >>`).

### 4.9 PDF/UA-Strukturbaum (immer versucht, wenn `/StructTreeRoot` mit Kindern existiert)

[V] `PPS:427-570`, [P] P1:
1. Erstes `PDStructureElement` unter `StructTreeRoot./K` als Elternelement (`docElement`) wählen.
2. Neues `StructElem`: `/S /Form`, `/P docElement`, `/T (Signature Table)`, `/Pg <Seite des Widgets>`, `/K [ << /Type /OBJR /Pg <Seite> /Obj <Widget> >> ]`, `/A << /O /Layout /Placement /Block >>`. Wird an `docElement./K` angehängt.
3. ParentTree-Schlüssel = `/ParentTreeNextKey`, falls ≥ 0. Sonst Maximum der Schlüssel im Number Tree + 1.
   - `/Nums` vorhanden, keine `/Kids`: `[key, structElem]` anhängen.
   - Nur `/Kids`: neues Blatt `<< /Nums [key structElem] /Limits [key key] >>` an `/Kids` anhängen.
   - Weder noch, oder beides: Fehler `error.pdf.sig.pdfua.1`.
4. Widget `/StructParent key`, `StructTreeRoot /ParentTreeNextKey key+1`, Seite `/Tabs /S`. Ist `/TU` leer, wird `/TU` = Feldname gesetzt.
5. Fehler werden nur bei `SIG_PDFUA_FORCE=true` gemeldet, sonst geloggt und ignoriert [V] `PPS:555-568`, `SPS:225-232`. In den PDF-Over-Profilen ist `SIG_PDFUA_FORCE` nicht gesetzt [V] (Suche in `cfgzip/cfg` ohne Treffer). Im Formularfeld-Fall wird das Feld über den **neu gebildeten** Namen gesucht (`<Name><n>`) und daher nicht gefunden. `signatureField` ist dann `null`. Hat das Dokument einen Strukturbaum mit Kindern, endet die Injektion deshalb in einer NullPointerException, die nur geloggt wird. Es entsteht kein StructElem für die Signatur [I] `PPS:342-350,458` (konsistent mit P2: kein StructElem im Increment [P]).

### 4.10 PDF/A

| Aspekt | Verhalten | Fundstelle |
|---|---|---|
| Aktivierung | Profil-Key `sig_obj.<P>.key.SIG_PDFA1B_VALID=true` (veraltet) oder `…key.SIG_PDFA_VALID=true`. Gemeint ist der **Key**, nicht der Value. In PDF-Over: alle `*_PDFA`-Profile | [V] `SPS:205-223,243-254`; `cfgzip/cfg/profiles/SIGNATURBLOCK_SMALL_DE_NOTE_PDFA.properties:9` |
| Erkennung | `pdfaid:part` aus dem XMP-Metadatenstrom des Katalogs (`DomXmpParser`) | [V] `PPS:850-875` |
| Wirkung der Version | Teil = 1 → `isPDFA()=true`. Teil = 3 → `isPDFA3()=true`. **Teil = 2 → beide false**, d. h. für PDF/A-2-Dokumente greift trotz PDF/A-Profil **keine** PDF/A-Sonderbehandlung. Kein XMP → Profilflag gilt (`SIG_PDFA_VALID` gilt dann für beide) | [V] `SPS:205-223,243-254` |
| OutputIntent | Nur bei **sichtbarer** Signatur: Katalog-`/OutputIntents` wird durch **genau einen** neuen Intent **ersetzt**: `/Type /OutputIntent /S /GTS_PDFA1 /Info /OutputCondition /OutputConditionIdentifier (sRGB IEC61966-2.1) /RegistryName (http://www.color.org) /DestOutputProfile <ICC-Stream /N 3, Flate>` aus `pdf-as-4/pdf-as-lib/src/main/resources/icm/sRGB Color Space Profile.icm` | [V] `PPS:260-288` [P] P1, P2 |
| Signaturblock | Alphakanal entfernen. TTF einbetten (Profil muss TTF-Schriften nutzen). PDF/A-3: `CIDSet` entfernen | [V] `VSB:226-234`, `PTC:168-186`; `pdf-as-4/doc/PDFAS4_Profile_Dokumentation.docx` („… müssen alle im Profil verwendeten Schriften TTF Schriftarten sein“) |
| Nicht geändert | XMP-Metadaten, `/Info`, `/Version`, `/Extensions` | [P] (nicht im Increment) |
| Prüfung | Nach dem Speichern PDFBox-Preflight (PDF/A-1b), **nur bei `isPDFA()`**. Validierungsfehler werden **nur geloggt**. Abbruch nur bei Syntaxfehlern, IO- oder Laufzeitfehlern | [V] `PPS:399-401,669-714` |
| Nebeneffekt | Die PDFBox-Fixup-Schriften in `/DR` (`/Helv`, `/ZaDb`) sind nicht eingebettet. [I] Für PDF/A meist unkritisch, weil sie von keinem Content-Stream benutzt werden. Mit veraPDF prüfen |  |

### 4.11 Incremental Update – was genau geschrieben wird

Objekte im Increment, Standardfall „neues Feld, sichtbar“ [V] [BC] [P] P1:

| Objekt | Grund |
|---|---|
| Katalog | `addSignature` markiert ihn als geändert. Enthält bei neuem AcroForm dieses direkt eingebettet, außerdem eventuell neue `/OutputIntents` |
| AcroForm (wenn indirekt) | als geändert markiert |
| `/Pages`, neue Seite | nur bei neuer Seite |
| Zielseite | `/Annots` neu und direkt eingebettet, ggf. `/Tabs /S` |
| Feld/Widget, Signaturdictionary, Holder-/Inner-Form, Bilder, Fonts (+ Deskriptoren, `FontFile2`, `ToUnicode`, `CIDToGIDMap`, `CIDSet`), `DR/Font`-Dictionary, Helvetica/ZapfDingbats-Fontdicts | neu |
| StructTreeRoot, ParentTree (bzw. Blatt), Eltern-StructElem, neues StructElem, OBJR, Attributdict | PDF/UA |
| OutputIntent + ICC-Stream | PDF/A und sichtbar |
| xref-Tabelle bzw. xref-Stream und Trailer | siehe Phase E, Schritt 8 |

Unverändert bleiben: Header `%PDF-1.x`, `/Info` (kein `ModDate`), XMP, alle übrigen Seiten und Objekte [P].

### 4.12 Bestehende Signaturen, mehrfaches Signieren

- Es gibt **keine** Prüfung, ob das Dokument bereits signiert ist. Jede weitere Signatur ist ein weiteres Increment mit neuem Feld `PDF-AS Signatur<n+1>` [V] (kein Prüfcode) [P] P3 (3 Increments, `/M` jeweils neu, `/SigFlags 3` jeweils neu geschrieben).
- Ältere Signaturen bleiben gültig, solange nur die in 4.11 genannten Objekte geändert werden. Das entspricht den erlaubten Änderungen bei „Formular ausfüllen/signieren“ [I].
- Ein schon signiertes **Zielfeld** (Formularfeld-Fall) ergibt `IllegalStateException` [V] `PPS:890-892`.
- In PDF-AS-Tests gibt es die Szenarien `POS_LAST_ALREADY_SIGNED` und `POS_LAST_ALREADY_SIGNED_FORCE_NEW` (Positionierung bei vorhandenen Signaturblöcken) [V] `pdf-as-4/pdf-as-tests/src/test/test-suites/public_pdfbox2/`.

### 4.13 Verschlüsselte PDFs

- Laden nur ohne Passwort, also leeres User-Passwort [V] `PBO:56-58`. Dokumente mit User-Passwort weist PDF-Over vorab ab [V] `…/PositioningState.java:84-85`.
- Ist das Dokument verschlüsselt **oder** fehlt die Owner-Berechtigung, ist Signieren nur erlaubt, wenn `(canModify && canModifyAnnotations) || canFillInForm`. Sonst Fehler `error.pdf.sig.12` „Document is protected“ [V] `PBU:17-39`; `pdf-as-4/pdf-as-common/src/main/resources/resources/messages/common.properties:46`.
- Beim Schreiben verschlüsselt PDFBox neue Strings und Streams mit dem vorhandenen Security Handler (`willEncrypt`). Eine Ausnahme für das Signaturdictionary `/Contents` gibt es im Writer **nicht** [BC] (`COSWriter#visitFromString` ruft `SecurityHandler#encryptString` auf). [I] Damit würde der Platzhalter verschlüsselt geschrieben und danach unverschlüsselt mit dem CMS überschrieben. Bei AES verlängert sich die Platzhalterlänge (IV, Padding), und das Füllmuster nach dem CMS wäre kein reines `0`. Das Verhalten mit RC4/AES-128/AES-256 muss getestet werden. Für einen Nachbau gilt gemäß ISO 32000: Signatur-`/Contents` **nie** verschlüsseln.

### 4.14 DocMDP, Zertifizierungssignaturen, FieldMDP, Lock, Seed Values

- PDF-AS erzeugt **keine** Zertifizierungssignatur (`/Perms /DocMDP`, `/Reference`, `/TransformParams`) [V] (Suche ohne Treffer).
- PDF-AS **prüft auch nicht**, ob das Eingangsdokument DocMDP-zertifiziert ist (P=1/2/3) oder ob `/Lock`/`/SV` am Zielfeld vorhanden sind [V] (Suche ohne Treffer). [I] Bei DocMDP P=1 macht die neue Signatur die Zertifizierung in Acrobat ungültig („Änderungen nicht erlaubt“).
- `/SigFlags 3` (AppendOnly) wird gesetzt [BC].

---

## 5. CMS/CAdES-Struktur

### 5.1 Keystore-Signatur (`PAdESSignerKeystore`, IAIK-CMS)

Kontext: PDF-Over liefert im eigenen Maven-Repo ein `iaik_cms-2.15.jar` mit, dessen Manifest „IAIK-CMS … 5.0 evaluation version“ angibt [BC] (`repo/iaik/iaik_cms/2.15/iaik_cms-2.15.jar`, `META-INF/MANIFEST.MF`). PDF-AS 4.4.2 selbst baut gegen `iaik_cms-5.1.1.jar` [V] `pdf-as-4/pdf-as-lib/build.gradle:77-80`.

ASN.1-Struktur (Code [V] `PKS:144-209,298-367`, nachgemessen [P] P1/P2/P3):

```
ContentInfo ::= SEQUENCE {                                   -- DER, definite length [P]
  contentType  id-signedData (1.2.840.113549.1.7.2),
  content [0] EXPLICIT SignedData {
    version            1                                     [P]
    digestAlgorithms   { sha256 (2.16.840.1.101.3.4.2.1) mit NULL-Parametern }   [P]
    encapContentInfo   { eContentType id-data, eContent ABSENT }   -- SignedData.EXPLICIT = detached [V] PKS:161 [P]
    certificates [0]   { Signerzertifikat }                  -- nur EIN Zertifikat, keine Kette [V] PKS:162 [P]
    crls               ABSENT                                [P]
    signerInfos { SignerInfo {
      version             1                                  [P]
      sid                 IssuerAndSerialNumber              [V] PKS:154 [P]
      digestAlgorithm     sha256 (NULL-Parameter)            [P]
      signedAttrs [0] (DER-sortiert, Reihenfolge laut Probe):
        content-type       (1.2.840.113549.1.9.3)   = id-data              [V] PKS:308-312
        mime-type          (0.4.0.1733.2.1)         = UTF8String "application/pdf"   [V] PKS:298-306
        signing-time       (1.2.840.113549.1.9.5)   = UTCTime (Tag 0x17), new Date()   [V] PKS:337-341 [P]
        message-digest     (1.2.840.113549.1.9.4)   = Hash der ByteRange-Daten   -- automatisch von IAIK [P]
        signing-certificate-v2 (1.2.840.113549.1.9.16.2.47)
              = SEQ { SEQ { ESSCertIDv2 { certHash OCTET(32), issuerSerial } } }
                -- hashAlgorithm weggelassen (= Default SHA-256), issuerSerial vorhanden [V] PKS:314-335 [P]
        (bei Digest SHA-1 stattdessen signing-certificate (1.2.840.113549.1.9.16.2.12) mit ESSCertID)
      signatureAlgorithm  sha256WithRSAEncryption (1.2.840.113549.1.1.11, NULL)
                          bzw. ecdsa-with-SHA256 (1.2.840.10045.4.3.2) **mit NULL-Parametern**   [P]
      signature           RSA: Modullänge (P1: 384 Byte) / ECDSA: DER-SEQ{r,s} (P2/P3: 70 Byte)
      unsignedAttrs       ABSENT                             [P]
}}}}
```

Varianten:
- Mit `sig_obj.PAdESCompatibility=true` (in PDF-Over **nicht** gesetzt, nur auskommentiert dokumentiert) gibt es nur content-type und signing-certificate(-v2) plus das automatische message-digest. **Kein** mime-type, **kein** signing-time [V] `PKS:166-180,358-367`; `cfgzip/cfg/advancedconfig.properties:25-29`.
- Algorithmuswahl nach Schlüsseltyp [V] `CU:34-75`:

| Public Key | Signaturalgorithmus | Digest |
|---|---|---|
| RSA (alle Längen) | sha256WithRSAEncryption (PKCS#1 v1.5, **kein PSS**) | SHA-256 |
| DSA | dsaWithSHA256 | SHA-256 |
| EC, Feldgröße ≥ 512 (P-521, brainpoolP512r1) | ecdsa-with-SHA512 | SHA-512 |
| EC, Feldgröße ≥ 256 (P-256, **P-384**, brainpoolP256/384) | ecdsa-with-SHA256 | SHA-256 |
| EC, Feldgröße < 256 | ecdsa-with-SHA1 | SHA-1 (→ signing-certificate v1) |
| sonst | `NoSuchAlgorithmException` → `error.pdf.sig.01` | – |

- Nach dem Erzeugen ruft `PKS` `SignatureUtils.verifySignature(signature, input)` als Selbstprüfung auf [V] `PKS:192-193`.
- Keystore-Laden: zuerst mit dem Standard-Provider (IAIK), dann Fallback auf `SunJSSE` (Probleme mit manchen PKCS#12). Der Typ kommt aus der Konfiguration (`PKCS12`, `JKS`, …) [V] `PKS:94-112,259-296`. Fehlercodes: 11002 (Keystore öffnen), `error.pdf.sig.13–18` (Key oder Zertifikat fehlt, Passwort null) [V] `PKS:219-257`.

### 5.2 SL-Signatur (lokale BKU, A-Trust Handy-Signatur / ID Austria über A-Trust)

**PDF-AS baut hier kein CMS.** Es nimmt das von der BKU gelieferte `CMSSignature` **unverändert** und bettet es ein [V] `SCW:92-125`, `PSW:69-71`. Was im CMS steht, bestimmt die BKU oder der A-Trust-Server.

Was PDF-AS vom gelieferten CMS verlangt (Prüfung in `SignatureUtils.verifySignature`) [V] `SU:26-137`:
1. DER-parsebares `SignedData`. Externer Inhalt = ByteRange-Konkatenation (`setContent(input)`).
2. **Genau ein** SignerInfo (0 oder mehr als 1 → 11008 „Signature created by the BKU is not valid“).
3. `verify(0)` gelingt. Das Signerzertifikat muss dafür in `certificates` enthalten sein, `message-digest` muss zum Hash der ByteRange-Daten passen, und die Signatur muss stimmen.
4. Signed Attribute `signing-certificate` **oder** `signing-certificate-v2` muss vorhanden sein und das Signerzertifikat referenzieren (`isSignerCertificate`).
5. SHA-1-Fingerprint des Signerzertifikats = Fingerprint des vorher per InfoboxRead geholten Zertifikats, sonst „Certificates missmatch!“ [V] `SCW:119-123`.
6. Optional (`report.invalidSign`) wird ein ungültiges CMS Base64-kodiert in die Metadaten gelegt [V] `SCW:104-114`, `ICC:99`.

Was die BKU laut Security-Layer-Spezifikation erzeugen muss [E] (SL 1.2 Core, Abschnitt CreateCMSSignatureRequest):
- ein CAdES-konformes CMS in `sl:CMSSignature` (Base64);
- signierte Attribute: **ContentHints** (mit MIME-Type als contentDescription, contentType = id-data), **ESS signing-certificate-v2** (signing-certificate bei SHA-1) und **SigningTime**;
- für `ExcludedByteRange`: Offsets nullbasiert, **From und To inklusive**. Der Bereich wird für die Anzeige mit 0x00 gefüllt und gehört nicht zum Signaturinput.

[I] Erwartung an A-Trust: ECDSA P-256 (ecdsa-with-SHA256) oder RSA, SHA-256, signing-time vorhanden, Zertifikat(e) in `certificates`. Das muss an einer echten A-Trust-Signatur verifiziert werden (siehe Offene Fragen). Lokal liegt keine solche Probe vor. Alle Proben P1–P3 sind Keystore-Signaturen (erkennbar am ETSI-mime-type-Attribut, das nur `PKS` setzt).

`PAdESCompatibility`-Flag: XML-Attribut `PAdESCompatibility="true"` am `CreateCMSSignatureRequest`. Gesetzt wird es nur bei `sig_obj.PAdESCompatibility=true` oder `sig_obj.CheckPAdESCompatibility=true` mit Whitelist-Treffer (Server-Header) [V] `BSL:141-158`, `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/schema/CreateCMSSignatureRequestType.java:88-89`. In PDF-Over ist es **nicht** gesetzt [V]. MOCCA unterstützt das Flag seit 1.3.20 [E]. Die genaue Wirkung (vermutlich keine signing-time, also PAdES-B-B-konform) ist nicht dokumentiert [I].

### 5.3 PAdES-Level und Konformität

| Kriterium (ETSI EN 319 142-1, PAdES-B-B) | PDF-AS-Keystore | SL/A-Trust | Bewertung |
|---|---|---|---|
| SubFilter `ETSI.CAdES.detached` | ja | ja | ok |
| content-type = id-data, message-digest | ja | ja [E] | ok |
| signing-certificate-v2 | ja | ja [E] | ok |
| signing-time **nicht** vorhanden (Zeit über `/M`) | **verletzt** (vorhanden) | **verletzt** [E] (laut SL-Spec Pflicht) | [I] Validatoren (DSS) melden das als Abweichung vom Baseline-Profil. Das muss getestet werden |
| mime-type / ContentHints | zusätzlich (ETSI mime-type) | ContentHints [E] | zulässig [I] |
| ECDSA-AlgorithmIdentifier ohne Parameter (RFC 5758) | **NULL-Parameter** [P] | unbekannt | [I] Wird in der Praxis toleriert. Ein Nachbau sollte die Parameter weglassen |
| Zeitstempel (B-T), DSS/VRI (B-LT), DocTimeStamp (B-LTA) | nein | nein | nicht vorhanden [V] |
| CMSAlgorithmProtection | nein [P] | unbekannt | optional |
| `/Extensions /ESIC` im Katalog | nein [P] | – | optional |

Fazit: PDF-AS erzeugt **PAdES-B-B-artige** Signaturen (historisch „PAdES-BES“), die in der Praxis als PAdES-Basissignatur anerkannt werden (RTR-Prüfdienst, Adobe). Streng nach Baseline-Profil sind sie wegen `signing-time` **nicht** konform [I].

---

## 6. Externes Signieren – Schnittstellen

### 6.1 Vertrag `IPlainSigner`

```
X509Certificate getCertificate(SignParameter)                              // vor jeder PDF-Änderung
byte[]          sign(byte[] input, int[] byteRange, SignParameter, RequestedSignature)
                //  input     = ByteRange-Konkatenation (NICHT das ganze PDF, NICHT ein Hash)
                //  byteRange = [0, a, b, c] (absolute Offsets im fertigen PDF)
                //  Rückgabe  = DER-kodiertes ContentInfo(SignedData)
String          getPDFFilter() / getPDFSubFilter()
```
[V] `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/api/sign/IPlainSigner.java:36-64`.

### 6.2 SL 1.2 (lokale BKU und A-Trust)

**Schritt 1 – Zertifikat** [V] `BSL:71-83`, `SCW:66-90`:
`InfoboxReadRequest` mit `InfoboxIdentifier=Certificates` und `AssocArrayParameters/ReadValue/@Key=SecureSignatureKeypair`. Aus der Antwort wird das Paar mit Key `SecureSignatureKeypair` gelesen: `Base64Content` = DER-Zertifikat.

**Schritt 2 – Signatur** [V] `BSL:85-162`:

```xml
<!-- Namespace http://www.buergerkarte.at/namespaces/securitylayer/1.2#; JAXB, UTF-8, formatiert (Präfix vom Marshaller gewählt [I]) -->
<CreateCMSSignatureRequest Structure="detached" [PAdESCompatibility="true"]>
  <KeyboxIdentifier>SecureSignatureKeypair</KeyboxIdentifier>
  <DataObject>
    <MetaInfo><MimeType>application/pdf</MimeType></MetaInfo>
    <Content Reference="formdata:fileupload"/>          <!-- upload-Modus -->
    <!-- oder: <Content><Base64Content>…ganzes PDF…</Base64Content></Content>  (b64-Modus) -->
    <ExcludedByteRange><From>a</From><To>b-1</To></ExcludedByteRange>
  </DataObject>
</CreateCMSSignatureRequest>
```

- **Übertragene Daten = das vollständige PDF** in Endlänge (Original + Increment), bei dem die Bytes von `<` bis `>` des `/Contents`-Werts durch **0x00** ersetzt sind (`blackOutSignature`) [V] `PU:72-97`, `BSL:97,109-115`, `A4C:115-117`.
- `ExcludedByteRange`: `From = br[0]+br[1]` (Offset `<`), `To = br[2]-1` (Offset `>`), beide inklusive [V] `PU:53-70`, `BSL:99,121-129` [E].
- Transport: lokale BKU `POST http://127.0.0.1:3495/http-security-layer-request`, Formularfeld `XMLRequest`. Im upload-Modus `multipart/form-data` mit `XMLRequest` (UTF-8) und Datei `fileupload` [V] `LBC:81-99`. A-Trust: `POST https://service.a-trust.at/mobile/https-security-layer-request/default.aspx` als multipart mit `fileupload` (`application/pdf`, Dateiname `sign.pdf`) und `XMLRequest`, danach der interaktive Web-Ablauf [V] `MBC:199-213`; `pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java:108`.
- Antwort: `CreateCMSSignatureResponse/CMSSignature` (Base64-DER) oder `ErrorResponse` → `SLPdfAsException(code, info)`. Die Codes 6000/6001 bedeuten Abbruch durch den Benutzer [V] `A4C:144-158`, `A4S:162-172`.
- **Der Hash wird von der BKU berechnet.** PDF-AS berechnet hier keinen Hash. A-Trust bekommt damit das komplette Dokument. Es gibt ein serverseitiges Größenlimit (HTTP 413 → „Provided document exceeds size limit“) [V] `MBC:127`; `pdf-over-gui/src/main/resources/at/asit/pdfover/gui/messages.properties` (`atrusterror.http_413`).

### 6.3 Keystore

Eingabe: ByteRange-Konkatenation. Ausgabe: CMS nach 5.1. Der private Schlüssel bleibt lokal. Die Zeit im `signing-time` kommt von der lokalen Uhr [V] `PKS:144-209`.

### 6.4 Asynchrone API (von PDF-Over nicht genutzt, aber als Vorlage für einen zweiphasigen Nachbau relevant)

[V] `PAI:317-479`, `SDE:72-76`, `PPS:745-749`:
1. `startSign`: Status anlegen, Zertifikat anfordern.
2. `process` (Zertifikat vorhanden): `signPDF` mit `SignatureDataExtractor`. Dessen `sign()` merkt sich die ByteRange-Daten und liefert `{0x00}`. Ergebnis: fertiges PDF mit `/Contents <00 000…>`, dazu `signatureData` (ByteRange-Konkatenation) und `byteRange`.
3. `process` (Signatur vorhanden): CMS gegen `signatureData` prüfen, Zertifikat vergleichen (11019). Dann `COSString(cms).toHexString()` (Großbuchstaben) ab `byteRange[1]+1` **in das fertige PDF-Byte-Array schreiben**. Eine Längenprüfung gegen den Platzhalter gibt es dabei **nicht** [V] `PAI:427-447` [I].
4. `finishSign`: Ergebnis ausgeben.

**Empfehlung für den Nachbau [I]:** Genau dieses zweiphasige Modell (PDF mit Platzhalter fertig schreiben → Daten bzw. Hash an das Signaturgerät → CMS einpatchen) ist für alle Plattformen am besten geeignet. Es entkoppelt die (asynchrone, UI-lastige) A-Trust-Interaktion vom PDF-Writer. Vor dem Einpatchen muss die Längenprüfung `2·len(cms) ≤ 2·N` stehen.

---

## 7. Vorbedingungen, Prüfungen, Fehlerfälle, Größenlimits

| Situation | Verhalten | Fundstelle |
|---|---|---|
| Profil existiert nicht | PDFASError 11009 | [V] `PAI:95-102`; `pdf-as-4/pdf-as-common/src/main/resources/resources/messages/error.properties:13` |
| keine Eingabe | 10001 | [V] `PAI:104-106` |
| PDF nicht parsebar | PDFBox-Exception → Fehler. PDFBox 2 lädt **lenient**: kaputte xref wird durch Brute-Force-Objektsuche rekonstruiert [I] | `PBO:57` |
| User-Passwort | Laden scheitert. PDF-Over meldet „password protected“ | [V] `…/PositioningState.java:84-85` |
| Verschlüsselt/eingeschränkt ohne Formular- bzw. Annotationsrecht | `error.pdf.sig.12` „Document is protected“ | [V] `PBU:17-39` |
| Zertifikat abgelaufen oder noch nicht gültig | 11021 | [V] `PAI:272-286`; `error.properties:25` |
| 0 Seiten | `IllegalStateException("Cannot sign an empty document")` | [BC] |
| Zielfeld schon signiert | `IllegalStateException` | [V] `PPS:890-892` |
| Bereits signiertes Dokument | **erlaubt**, neues Increment | [V] [P] P3 |
| DocMDP-zertifiziert, gesperrte Felder, Seed Values | **keine Prüfung** | [V] |
| PDF/UA-Strukturbaum unvollständig | nur mit `SIG_PDFUA_FORCE` ein Fehler (`error.pdf.sig.pdfua.1`) | [V] `PPS:523-529,555-568` |
| PDF/A-Preflight negativ | nur Log | [V] `PPS:692-701` |
| CMS größer als reserviert (Standard 4096 Byte) | IOException „Can't write signature, not enough space …“ → `error.pdf.sig.06` „Failed to finish signature process“ | [BC]; [V] `PPS:407-409` |
| BKU-CMS ungültig, mehrere SignerInfos, signing-certificate fehlt | 11008 | [V] `SU:35-45,58-105` |
| BKU hat mit anderem Zertifikat signiert | „Certificates missmatch!“ (`PdfAsSignatureException`) | [V] `SCW:119-123` |
| Benutzer bricht ab (SL 6000/6001) | `UserCancelledException` in PDF-Over | [V] `A4S:162-172`, `A4C:87-89,140-141` |
| A-Trust: Dokument zu groß | HTTP 413 → Fehlermeldung | [V] `MBC:127` |
| Dateigröße allgemein | `br[2]+br[3]` muss kleiner als 2·10⁹ sein (Platzhalter-Erkennung, `int[]` in PDF-AS). Praktisch begrenzt der Speicher, weil das Dokument mehrfach vollständig im RAM liegt (Original, Increment-Puffer, `signedDocument`, Blackout-Kopie für den Upload) | [BC] `pdfbox:COSWriter#detectPossibleSignature`; [V] `PSW:63`, `PU:72-97` [I] |
| ByteRange-String passt nicht in 35 Zeichen | IOException (praktisch unmöglich, 4 Zahlen mit je ≤ 10 Stellen) | [BC] |

---

## 8. Nutzt PDF-Over eine Verifikation?

- **Nein**, es gibt keinen Aufruf von `PdfAs.verify()` in PDF-Over (Suche in `pdf-over-gui`, `pdf-over-commons`, `pdf-over-signer` ohne Treffer) [V]. Die Prüf-URL im Profil (`verifyURL`, Text im Signaturblock und in `/Reason`) ist reiner Text [V] `cfgzip/cfg/profiles/INVISIBLE.properties:15`.
- Implizit beim Signieren [V]:
  1. CMS-Selbstprüfung (Integrität, signing-certificate-Attribut, genau ein SignerInfo) bei SL **und** Keystore (`SU`, `SCW:102-103`, `PKS:192-193`);
  2. Zertifikatsvergleich InfoboxRead ↔ CMS (nur SL);
  3. PDF/A-1b-Preflight (nur Log).
- Es gibt **keine** Zertifikatsketten-, Sperr- oder QES-Prüfung. PDF-AS kann so etwas nur über MOA-SP (`pdf-as-moa`), das PDF-Over nicht einbindet [V] `pdf-as-4/doc/PDFAS4_Dokumentation.docx` (Einleitung: „… keine Zertifikatsprüfung durchgeführt …“).
- Die vorhandene PDF-AS-Verifikation (`PDFBOXVerifier`) durchläuft nur `/Fields` der obersten Ebene mit `/FT /Sig`. Sie schneidet die ByteRange aus und prüft über `PAdESVerifier` (Filter `Adobe.PPKLite`/`ETSI.CAdES.detached`) die Integrität [V] `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/verify/pdfbox2/PDFBOXVerifier.java`, `pdf-as-4/signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/PAdESVerifier.java:51-72`, `cfgzip/cfg/advancedconfig.properties:101`. Sie eignet sich als Referenz-Orakel für Tests (9.3).

---

## 9. Teststrategie für den Nachbau

### 9.1 Testvektoren (Eingaben)

| Kategorie | Inhalt | Quelle |
|---|---|---|
| Vorhandene PDF-AS-Proben | P1, P2, P3 (Referenz für Struktur und CMS). Außerdem unsignierte Gegenstücke: `manySignFields.pdf`, `new_qr_2-2.pdf`, `1Sign_manyQR.pdf`, `pdf-as-4/pdf-as-pdfbox-2/src/test/resources/data/platzhalter_en_de_test.pdf`, `pdf-as-4/pdf-as-web/src/test/resources/data/placeholder_sigfield_and_qr.pdf`, `enc_own.pdf` (verschlüsselt, Owner-Passwort) | [V] Dateien vorhanden |
| Rotation | `simple_rotated_{0,90,180,270}.pdf`, `rotate_{090,180,270}.pdf` | [V] `pdf-as-4/pdf-as-cli/src/test/resources/` |
| PDF-AS-Testsuiten | `POS_*`, `PDFA_*` je mit `config.properties`, `example.pdf`, `example_ref.png` (Referenzbild der Seite) | [V] `pdf-as-4/pdf-as-tests/src/test/test-suites/public_pdfbox2/` |
| Eigenes Korpus (anzulegen) [I] | (a) PDF 1.3/1.4 mit klassischer xref; (b) PDF 1.5+ nur xref-Stream + ObjStm; (c) Hybrid mit `/XRefStm`; (d) linearisiert; (e) mit vorhandenem AcroForm und `/NeedAppearances true`; (f) leeres Signaturfeld (direkt und mit `/Kids`); (g) bereits 1×/2× signiert, auch von Fremdsoftware (Adobe, DSS); (h) DocMDP P=1/2; (i) RC4-, AES-128- und AES-256-verschlüsselt mit Owner-Passwort, mit und ohne Formularrecht; (j) PDF/A-1b, -2b, -3b; (k) PDF/UA mit `/Nums` bzw. `/Kids`-ParentTree; (l) kaputte xref-Offsets; (m) große Dateien (50/200 MB, Speicherverhalten mobil); (n) Seiten mit CropBox ≠ MediaBox; (o) 0 Seiten | – |
| Schlüssel (anzulegen) [I] | Testschlüssel RSA-2048/3072/4096, EC P-256/P-384/P-521, brainpoolP256r1 in PKCS#12 (und JKS, wenn JKS unterstützt werden soll). Eigene Test-CA, keine echten Identitäten | – |
| SL-Simulator [I] | Mock-BKU, die einen `CreateCMSSignatureRequest` entgegennimmt, `ExcludedByteRange` auswertet und ein CMS mit konfigurierbaren Attributen liefert. Fehlerfälle: falsches Zertifikat, falscher Digest, 2 SignerInfos, kein signing-certificate, zu großes CMS, `ErrorResponse` 6001 | – |

### 9.2 Strukturelle Golden-File-Vergleiche

Nicht deterministisch sind `/M`, `/ID[1]`, `signing-time`, ECDSA-Signaturwerte, das BKU-CMS und Font-Subset-Präfixe [I]. Deshalb wird **normalisiert** verglichen statt byte-genau:

1. **Präfix-Invariante:** `output[0:len(input)] == input` (byte-genau). Das Increment beginnt mit `\r\n` [BC].
2. **Signaturdictionary:** Keys und Werte wie in 4.1. `/Contents`-Hexlänge = 2·N. `/ByteRange` nach 4.2. `br[2]+br[3] == len(output)`. `output[br[1]] == '<'`, `output[br[2]-1] == '>'`. Das Hex endet mit `0`-Padding.
3. **Objektmenge im Increment:** Diff gegen PDF-AS mit demselben Eingabe-PDF (z. B. per pdf-as-cli `-m sign -c ks …` als Referenzlauf in einer separaten Java-Umgebung). Vergleichsbasis: Typ und Rolle der Objekte, nicht die Nummern.
4. **Feld/Widget/AcroForm:** Keys nach 4.3–4.5. `/F`, `/SigFlags`, `/DA`, `/DR`-Einträge. `/T`-Nummerierung bei Mehrfachsignatur.
5. **CMS:** ASN.1-Dump (z. B. `openssl asn1parse -inform DER`, `dumpasn1`) mit Maskierung von Signaturwert, signing-time und message-digest. Danach Vergleich der Attribut-OIDs, Reihenfolge (DER-Sortierung), Algorithmus-OIDs **und** der Parameterkodierung (NULL vs. fehlend), Anzahl der Zertifikate, Versionen.
6. **message-digest-Orakel:** `SHA-256(output[0:br[1]] ‖ output[br[2]:])` muss dem `message-digest`-Attribut entsprechen.
7. **Visuell:** Seite mit Signaturblock rendern (pdfium/poppler/MuPDF) und gegen `example_ref.png` der PDF-AS-Testsuiten bzw. eigene PDF-AS-Referenzrenderings vergleichen (Toleranz für Antialiasing) [I].

### 9.3 Externe Validatoren

| Werkzeug | Zweck | Erwartung für PDF-AS-gleiche Ausgabe |
|---|---|---|
| **EU DSS Demo-Webapp** (`https://ec.europa.eu/digital-building-blocks/DSS/webapp-demo/validation`) bzw. DSS lokal | ETSI-Validierung, Format-Erkennung (PAdES-BASELINE-B?), ByteRange- und Struktur-Checks, Feld-Überlappungen | Integrität ok, Signaturformat prüfen. [I] Wegen `signing-time` ggf. Warnung oder kein „BASELINE-B“. Genau das klären und dokumentieren |
| **Adobe Acrobat Reader** | Viewer-Akzeptanz, „Dokument seit Signatur nicht verändert“, Änderungs-Historie, Darstellung des Signaturfelds | grüner bzw. gelber Haken (je nach Trust), keine „Änderung“-Warnung bei Mehrfachsignatur |
| **pdf-as verify** (`pdf-as-cli -m verify -vl intOnly`) | Kompatibilität mit dem österreichischen Referenzstack | `PAdESVerifier`: Integrität ok [V] `pdf-as-4/pdf-as-cli/src/main/java/at/gv/egiz/pdfas/cli/Main.java:67-130` |
| **pyHanko** (`pyhanko sign validate --pretty-print <pdf>`) | strenger PDF- und CMS-Parser, Erkennung erlaubter Änderungen im Increment (Difference Analysis) | „INTACT“. Die Änderungsanalyse sollte die PDF-AS-Änderungen als „form filling/signing“ einstufen [I] |
| **poppler `pdfsig`** (`pdfsig -dump <pdf>`) | NSS-basierte Prüfung, Linux-Viewer | „Signature is Valid“. Prüft auch ECDSA mit NULL-Parametern [I] |
| **RTR-Signaturprüfung** (`https://www.signaturpruefung.gv.at`) | österreichische Behördenprüfung (QES, Amtssignatur) | gültig, Signaturformat PAdES |
| **qpdf `--check`**, **veraPDF** | Syntax, xref-Konsistenz, PDF/A-Erhalt (veraPDF auch für PDF/UA) | keine neuen Fehler gegenüber dem Eingangsdokument |
| **openssl cms -verify -binary -content <(ByteRange-Daten)** | einfaches Krypto-Orakel in der CI | „Verification successful“ (mit `-noverify` ohne Trust) |

### 9.4 Negativ- und Robustheitstests [I]

- CMS genau N, N+1 und 2·N Byte groß (Platzhalterüberlauf muss sauber abbrechen).
- Manipulation nach dem Signieren: ein Byte in jedem ByteRange-Teil ändern → alle Validatoren melden Ungültigkeit.
- `/ByteRange` beschädigen oder `/Contents` verschieben → pyHanko und DSS müssen das erkennen.
- Abbruch in der Mitte (A-Trust-Timeout) → es darf keine halbfertige Ausgabedatei entstehen.
- Gleichzeitige Signaturen (Mehrfachauswahl in PDF-Over) → kein globaler Zustand (PDF-AS synchronisiert global über `PdfAs4Helper.class`, [V] `A4S:61,120`).

### 9.5 Was „bit-kompatibel“ realistisch heißt [I]

- **Erreichbar:** gleiche PDF-Objektstruktur, gleiche Keys/Werte (bis auf Zeit und IDs), gleiches `/Contents`-Layout (Großbuchstaben-Hex, `0`-Padding, N = 4096), gleiche ByteRange-Formatierung mit Leerzeichen-Padding, gleiche CMS-Attributmenge und -kodierung (inklusive NULL-Parameter, falls gewünscht).
- **Nicht erreichbar oder nicht sinnvoll:** Objektnummern-Zuteilung und Schlüsselreihenfolge exakt wie PDFBox (möglich, aber aufwendig und ohne Nutzen), identische Font-Subsets und Bildkompression (Flate-Implementierungen unterscheiden sich), `/ID`.
- **Bewusst abweichen sollte man [I]** bei: Nicht-Verschlüsselung von `/Contents` in verschlüsselten PDFs, Längenprüfung vor dem Einpatchen, Weglassen der ECDSA-NULL-Parameter, optional PAdES-B-B ohne signing-time (nur Keystore, SL-Seite bestimmt die BKU), korrekter PDF/A-2-Behandlung, Behandlung von Widgets in `/Kids`.

---

## 10. Hinweise für die Neuimplementierung [I]

Benötigte Bausteine (sprachunabhängig):
1. **PDF-Leser** mit xref-Tabelle, xref-Stream, Hybrid, Object Streams, Verschlüsselung (RC4/AES, mindestens lesend mit leerem User-Passwort) und Reparatur kaputter xref.
2. **Incremental-Writer**: Objekte neu schreiben, xref im passenden Format, Trailer mit `/Prev`, `/ID` aktualisieren, optional `/XRefStm` bei Hybrid-Dateien.
3. **Platzhalter-Mechanik**: Offsets beim Serialisieren mitschreiben, ByteRange patchen, Daten bzw. Hash bereitstellen, CMS mit Längenprüfung einpatchen. Die Daten nur streamen, damit große Dateien mobil machbar bleiben.
4. **CMS/CAdES-Builder** (Keystore): SignedData v1, IssuerAndSerialNumber, DER-sortierte Signed Attributes, ESSCertIDv2, RSA PKCS#1 v1.5 / ECDSA. Dazu ein **CMS-Prüfer** für die Selbstprüfung des BKU-CMS (6.2, Punkte 1–5).
5. **AcroForm/Widget/AP-Erzeugung** nach 4.3–4.7, inklusive TrueType-Subsetting (Type0/CIDFontType2, ToUnicode), PNG-Predictor-Bildern und optionalem PDF/UA-Strukturbaum.
6. **SL-1.2-Client** (XML, multipart, Blackout-Upload). Die A-Trust-Web-Interaktion ist ein eigenes Thema.

### 10.1 Plattformspezifische Randbedingungen der Engine [I]

| Thema | Desktop (Win/macOS/Linux) | iOS | Android |
|---|---|---|---|
| Lokale BKU (`http://127.0.0.1:3495`, SL über HTTP) | möglich (wie PDF-Over) | praktisch nicht verfügbar (keine lokale BKU, ATS blockiert Klartext-HTTP standardmäßig) | praktisch nicht verfügbar (Cleartext-HTTP muss explizit erlaubt werden, keine BKU-App bekannt) |
| A-Trust/ID Austria (SL über HTTPS, Upload des ganzen PDFs) | möglich | möglich; Datenvolumen und Größenlimit (HTTP 413) beachten | wie iOS |
| Keystore PKCS#12 | möglich | möglich (Datei-Import über Dokumentenauswahl) | möglich |
| Keystore JKS | nur mit eigenem JKS-Parser (Java-Format) | wie Desktop | wie Desktop |
| Speicher (Dokument liegt bei PDF-AS mehrfach vollständig im RAM) | unkritisch | kritisch bei großen PDFs → Streaming bzw. Dateipuffer | kritisch, wie iOS |
| PDF/A-Preflight (PDFBox preflight, veraPDF: beide Java) | nur über externes Tool oder weglassen (PDF-AS loggt ohnehin nur) | weglassen | weglassen |
| TrueType-Subsetting, Flate, PNG-Predictor, Alphakanal-Entfernung (PDF-AS: Java AWT/PDFBox) | Bibliothek nötig | Bibliothek nötig (keine AWT) | Bibliothek nötig (keine AWT) |
| IAIK-CMS/-JCE | nicht verwendbar (Java, proprietär) → eigener CMS-Builder | dto. | dto. |

---

## 11. Offene Fragen

1. Wie sieht ein **echtes A-Trust- bzw. ID-Austria-CMS** aus (Attribute, Algorithmus, Zertifikatskette, Größe)? Reichen 4096 Byte immer? Dafür braucht es eine reale, mit PDF-Over erzeugte Probe.
2. Was bewirkt `PAdESCompatibility="true"` bei A-Trust und den lokalen BKUs genau (signing-time und ContentHints weg?)? Soll der Klon das Flag setzen?
3. Soll der Klon PDF-AS-Eigenheiten nachbilden (signing-time, ETSI-mime-type, NULL-Parameter bei ECDSA, „verkehrte“ BBox, AcroForm-Fixup mit `/Helv`/`/ZaDb`/`/DA`, Ersetzen der OutputIntents) oder sauberes PAdES-B-B erzeugen? Wie wirkt sich das auf RTR-Prüfung und Amtssignatur-Erwartungen aus?
4. Wie verhält sich PDF-AS bzw. PDFBox mit **verschlüsselten** PDFs (wird `/Contents` mitverschlüsselt?) – Test nötig, bevor man das Verhalten übernimmt oder ablehnt.
5. Hybrid-xref: Laut Bytecode 2.0.34 wird `/XRefStm` gesetzt, die Probe P1 (älterer Stand) zeigt das nicht. Was erwarten Adobe und DSS?
6. Soll der Klon DocMDP-zertifizierte oder gesperrte Dokumente **ablehnen** oder warnen? PDF-AS signiert sie ohne Prüfung.
7. Sollen PAdES-B-T/LT/LTA (Zeitstempel, DSS) als Erweiterung angeboten werden? PDF-AS hat dafür keine Vorlage.
8. Soll das persistente `signature_field_name`-Verhalten von PDF-Over (Folgewirkung auf spätere Feldnamen) übernommen werden?
9. Soll JKS als Keystore-Format unterstützt werden (Java-spezifisch) oder nur PKCS#12?
10. Lizenzlage: PDF-Over liefert laut Manifest eine IAIK-CMS-„evaluation version“ mit. Für den Klon irrelevant, weil IAIK ersetzt wird, aber gut zu wissen.
11. Widgets in `/Kids` (Formularfeld-Fall) und `/NeedAppearances true`: Landen die Änderungen von PDFBox vollständig im Increment?

---

## Anhang A – Wichtigste Fundstellen (vollständige Pfade)

- `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/PADESPDFBOXSigner.java`
- `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/PdfboxSignerWrapper.java`
- `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/signing/pdfbox2/SignatureDataExtractor.java`
- `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/utils/PdfBoxUtils.java`
- `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/stamping/pdfbox2/{PDFAsVisualSignatureBuilder,PDFAsVisualSignatureProperties,PDFAsVisualSignatureDesigner,PDFAsTemplateCreator,TableDrawUtils,PDFBoxFont}.java`
- `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/PdfAsImpl.java`
- `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/util/{BaseSLConnector,ISignatureConnectorSLWrapper}.java`
- `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/util/{SignatureUtils,CertificateUtils}.java`
- `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/utils/PDFUtils.java`
- `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/settings/SignatureProfileSettings.java`
- `pdf-as-4/signature-standards/sigs-pades/src/main/java/at/gv/egiz/pdfas/sigs/pades/{PAdESSigner,PAdESSignerKeystore,PAdESConstants,PAdESVerifier}.java`
- `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/{PdfAs4Signer,PdfAs4BKUSLConnector,PdfAs4SignatureParameter,PdfAs4SigningState,PdfAs4Helper}.java`
- `pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/{MobileBKUConnector,LocalBKUConnector}.java`
- `cfgzip/cfg/config.properties`, `cfgzip/cfg/advancedconfig.properties`, `cfgzip/cfg/profiles/*.properties`
- PDFBox 2.0.34 (Bytecode): `PDDocument#addSignature`, `#prepareVisibleSignature`, `#prepareNonVisibleSignature`, `#assignSignatureRectangle`, `#assignAppearanceDictionary`, `#assignAcroFormDefaultResource`, `#saveIncremental`; `COSWriter#write(PDDocument,SignatureInterface)`, `#visitFromDocument`, `#visitFromDictionary`, `#detectPossibleSignature`, `#doWriteXRefInc`, `#doWriteSignature`, `#getDataToSign`, `#writeExternalSignature`; `PDSignatureField(PDAcroForm)`; `PDAcroForm#setSignaturesExist/#setAppendOnly`; `AcroFormDefaultFixup`, `AcroFormDefaultsProcessor`, `AcroFormGenerateAppearancesProcessor`; `SignatureOptions.DEFAULT_SIGNATURE_SIZE = 9472`
- Extern [E]: Security Layer 1.2 Core-Spezifikation (CreateCMSSignatureRequest, ExcludedByteRange), `https://sl.eid.egiz.gv.at/konzept/securitylayer/spezifikation/20140114-en/core/Core.en.html`. MOCCA-Release-Notes 1.3.20 (PAdESCompatibility), `https://interoperable-europe.ec.europa.eu/collection/e-government-innovation-center-egiz/solution/mocca/release-2`
