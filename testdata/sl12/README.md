# Testdaten `pdfover-sl12` (Paket D)

> **NUR FÜR TESTS.** Keine echten Zertifikate, keine Personendaten.

| Pfad | Inhalt | Herkunft |
|---|---|---|
| `jaxb-golden/*.xml` | Ausgabe des JAXB-Marshallers von PDF-AS 4.4.2 (`SLMarschaller`: formatiert, UTF-8) für `InfoboxReadRequest`, `CreateCMSSignatureRequest` (Upload, Base64, `PAdESCompatibility`) und `NullOperationRequest` | erzeugt mit den Original-Schemaklassen `at.gv.egiz.sl.schema.*` (EUPL) und `jaxb-runtime` 2.3.3 (wie in `pdf-as-lib/build.gradle`); Generator: `jaxb-golden/Golden.java.txt` (bildet `BaseSLConnector.createInfoboxReadRequest`/`createCMSRequest` nach) |
| `responses/*.xml` | synthetische SL-Antworten (Präfix `sl:`, Default-Namespace, `ErrorResponse` 6001/1501, fehlendes Paar, fremde Wurzel) | `infobox-read-response-*` enthalten das Testzertifikat `testdata/keystores/test-rsa2048.crt.der` |

Erkenntnis aus den Golden-Dateien: JAXB schreibt den SL-Namespace als **Default-Namespace** (kein
Präfix) und deklariert zusätzlich `xmlns:ns2` für XMLDSig; `Structure` steht vor den
Namespace-Deklarationen. `SlRequest::to_xml` bildet das Byte für Byte nach.

Neu erzeugen (nur lokal, benötigt JDK und die JAXB-Jars aus dem Gradle-Cache):

```sh
javac -d out -cp "$JAXB_CP" reference/pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/schema/*.java
cp testdata/sl12/jaxb-golden/Golden.java.txt Golden.java
javac -d out -cp "$JAXB_CP:out" Golden.java && java -cp "$JAXB_CP:out" Golden testdata/sl12/jaxb-golden
```
