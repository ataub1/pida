# PDF-Signaturen: Technik und Prüfung

## Wie steckt eine Signatur in einem PDF?

- Die Signatur wird als **inkrementelle Änderung** an das PDF angehängt; der ursprüngliche Inhalt bleibt Byte für
  Byte erhalten. Die Signatur deckt das ganze Dokument ab, außer der Lücke, in der sie selbst steht (ByteRange).
- Das Format folgt dem europäischen Standard **PAdES** (PDF Advanced Electronic Signatures, ETSI EN 319 142); intern
  steckt eine CMS-Signatur (CAdES).
- **Mehrere Signaturen:** Jede weitere Signatur wird wieder angehängt; frühere Signaturen bleiben gültig, solange
  danach nur zulässige Änderungen erfolgen.
- **Sichtbar oder unsichtbar:** Das Signaturbild (Signaturblock) ist nur die Darstellung; die eigentliche Signatur
  steckt in den Daten. Auch eine unsichtbare Signatur ist vollwertig.
- **Signaturfelder:** Formulare können leere Signaturfelder enthalten; PIDA kann dort hinein signieren.
- **Zertifizierte Dokumente:** Ein Dokument kann vom Ersteller „zertifiziert“ sein und Änderungen einschränken (bis
  hin zu „keine Änderungen erlaubt“). Dann ist Signieren nur eingeschränkt oder gar nicht möglich.
- **Geschützte Dokumente:** Ein PDF mit Kennwort zum Öffnen kann PIDA nicht signieren.

## Signaturniveaus (PAdES-Baseline)

- **B-B:** Grundsignatur.
- **B-T:** zusätzlich ein Zeitstempel eines Zeitstempeldienstes.
- **B-LT / B-LTA:** zusätzlich Prüfdaten (Zertifikate, Sperrinformationen) bzw. Archivzeitstempel für die
  Langzeitprüfung.
- PIDA signiert (wie PDF-AS) auf dem Niveau B-B.

## Signatur prüfen

- **Selbstprüfung in PIDA:** Nach dem Signieren prüft PIDA, ob das Dokument unverändert und die Signatur
  kryptografisch intakt ist. Vertrauenskette und Sperrstatus prüft PIDA nicht.
- **Vollständige Prüfung:** www.signaturpruefung.gv.at (Signaturprüfdienst der RTR) prüft Signatur, Zertifikat,
  Vertrauenskette und Sperrstatus.
- **Adobe Acrobat/Reader zeigt „Gültigkeit unbekannt“ oder eine Warnung:** Häufig vertraut Adobe dem
  Zertifikatsaussteller nicht, weil die europäischen Vertrauenslisten (EUTL) in den Einstellungen nicht aktiviert
  sind. Die Signatur selbst ist dadurch nicht ungültig; maßgeblich ist die Prüfung z. B. über signaturpruefung.gv.at.
- **„Dokument wurde nach dem Signieren verändert“:** Jede Änderung nach dem Signieren (auch Formular ausfüllen oder
  Kommentar in einem nicht dafür vorgesehenen Bereich) kann eine Signatur ungültig machen. Erst fertig bearbeiten,
  dann signieren.

## PDF/A

PDF/A ist ein ISO-Standard für die Langzeitarchivierung von PDF-Dokumenten. PIDA erkennt PDF/A-Dokumente und
signiert sie so, dass sie PDF/A-konform bleiben.

## Dokument für andere vorbereiten (QR-Platzhalter)

Wer ein Dokument erstellt, das andere unterschreiben, kann mit einem **QR-Platzhalter** festlegen, wo die
Signatur hinkommt. PIDA, PDF-Over und PDF-AS (auch die barrierefreie Signaturseite PDF-AS-WAI) erkennen ihn und
setzen den Signaturblock an seine Stelle.

1. **„QR-Platzhalter erstellen …“** wählen – auf der Startseite unter der Ablagefläche oder in der Befehlsliste.
2. **Nummer** festlegen. Für mehrere Unterschriften bekommt jeder Platzhalter eine eigene Nummer, z. B. 0001 und
   0002. Teilen Sie den Unterzeichnenden ihre Nummer mit.
3. **Signaturbild** und **Sprache** wählen. „Wie bei der unterzeichnenden Person eingestellt“ übernimmt deren
   eigenes Signaturbild.
4. Als **PNG** oder **SVG** speichern und im Textprogramm an der gewünschten Stelle einfügen – so breit, wie der
   Signaturblock werden soll. Gespeichert ist er 8,5 cm breit, das passt zum Standard-Signaturblock.
5. Das Dokument als PDF speichern.

Beim Signieren findet PIDA den Platzhalter, wenn unter Einstellungen › Dokument die Platzhaltersuche für
QR-Code-Platzhalter eingeschaltet ist. Ohne Platzhalter geht es auch mit Millimeter-Angaben: Geben Sie den
Unterzeichnenden Seite und Abstand von links und von unten der linken oberen Ecke an („Platzierung der Signatur“ ›
„Position in Millimetern“).
