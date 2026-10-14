# 03 – Signatur-Ökosystem Österreich / EU (für den PDF-Over-Klon)

Stand der Recherche: **2026-10-14**. Referenz-Code: PDF-Over `main` @ `c07c5ec` (4.4.10-SNAPSHOT), PDF-AS 4.4.2.
Pfadangaben ohne Präfix beziehen sich auf `ref/PDF-Over/`, Pfade mit Präfix `pdf-as-4/` auf `ref/pdf-as-4/`.
Alle Web-Abrufe und Messungen am 2026-10-14. Inhalte fremder Seiten wurden als Daten behandelt, nicht als Anweisungen.

## Legende für den Verifikationsgrad

| Tag | Bedeutung |
|---|---|
| **[C]** | Im lokalen Referenzcode verifiziert (PDF-Over oder PDF-AS, Pfad:Zeile angegeben) |
| **[G]** | In öffentlichem Fremd-Quellcode verifiziert (z. B. A-Trust-GitHub-Beispiele, Datei:Zeile angegeben) |
| **[W]** | Offizielle Quelle (A-Trust, id-austria.gv.at, RTR, EGIZ/A-SIT, EU, Gesetzestext), URL angegeben |
| **[S]** | Sekundärquelle (Presse, Herstellerblog, Forum, Suchmaschinen-Zusammenfassung) – plausibel, nicht offiziell |
| **[M]** | Eigene Messung am 2026-10-14 (harmlose GET/HEAD/OPTIONS-Requests ohne Zugangsdaten, siehe Anhang A) |
| **[I]** | Eigene Interpretation / Schlussfolgerung / Vermutung – **nicht** belegt |

Bewusst **nicht** übernommen: öffentlich dokumentierte Test-Zugangsdaten (A-Trust-Testsysteme), Zertifikats-Fingerprints, personenbezogene Kontaktdaten.

---

## 0. Kurzfazit (die zehn Aussagen, die die Architektur prägen)

1. **PDF-Over spricht für ID Austria das Security-Layer-1.2-HTTPS-Binding von A-Trust an** (`https://service.a-trust.at/mobile/https-security-layer-request/default.aspx`, `pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java:108`) und **parst die A-Trust-HTML-Seiten selbst** (Benutzername/Passwort, SMS-TAN, QR, App-Warten, FIDO2) [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/mobile/ATrustParser.java:149-215`).
2. **A-Trust verbietet genau das für Drittanwendungen**: Telefonnummer/Benutzername, Signaturpasswort und TAN müssen auf A-Trust-Seiten eingegeben werden; Parsen oder automatisches Befüllen per Programm ist laut A-Trust-Doku unzulässig und in der Nutzungsvereinbarung geregelt. Lokale Anwendungen sollen einen lokalen Browser bzw. ein Browser-Control anzeigen [W] ([docs.a-trust.at/apis/SecurityLayer](https://docs.a-trust.at/apis/SecurityLayer/), [GitHub A-Trust/Handy-Signatur](https://github.com/A-Trust/Handy-Signatur)). **Ein Klon darf den PDF-Over-Weg (natives Scraping) ohne ausdrückliche Vereinbarung mit A-Trust nicht 1:1 übernehmen** [I]. Warum PDF-Over (A-SIT) das darf, ist öffentlich nicht dokumentiert (offene Frage Q1).
3. **Es gibt eine offizielle, moderne Alternative: die CSC-API v1.4 („Hash-Signing“) von A-Trust** – OAuth2-Authorization-Code-Flow im Browser plus REST/JSON, **nur der Hash verlässt das Gerät** [W] ([docs.a-trust.at/apis/CSC](https://docs.a-trust.at/apis/CSC), [Presse 01.12.2023](https://a-trust.at/en/about_us/news/20231201_en_post.html)). Sie erfordert eine **Partnerschaft/Vertrag** (client_id, client_secret, subscriber_id) [G] (`HashSignExampleClient/GUIHashSigner.cs:28-32` im Repo [A-Trust/CSC_HashSignClient](https://github.com/A-Trust/CSC_HashSignClient)).
4. **Rein browserbasiert ohne eigenes Backend geht es nicht**: Das Produktiv-Endpoint von A-Trust sendet **keine CORS-Header** [M]; das Testsystem `hs-abnahme.a-trust.at` sendet `access-control-allow-origin: *` [M] – Test und Produktion verhalten sich also unterschiedlich. Die SL-Antwort landet entweder in der Browser-Verbindung (für eine Web-App cross-origin unlesbar) oder per `DataURL` serverseitig bei einem Empfänger, der öffentlich erreichbar sein muss [W] ([SL-Bindings §3.2.2/§3.2.3](https://sl.eid.egiz.gv.at/konzept/securitylayer/spezifikation/20140114-en/bindings/Bindings.en.html)).
5. **Mobile: Für Signaturen gibt es kein dokumentiertes App2App über Universal/App Links.** Die App „ID Austria“ (`at.gv.oe.app`) beansprucht ausschließlich `idp.id-austria.gv.at/auth/idp/profile/oidc/*` (Login), die App „A-Trust Signatur“ nur `www.a-trust.at/app/*`; `service.a-trust.at` hat keine Association-Dateien [M]. Die Bestätigung läuft per **Push in die App** [W] ([id-austria.gv.at Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse)).
6. **FIDO2 ist der plattformkritische Faktor**: In WKWebView (iOS) und Android-WebView funktioniert WebAuthn nur für Domains, mit denen die App verknüpft ist – also **nicht** für `service.a-trust.at` [S]/[W] ([Apple-Forum](https://developer.apple.com/forums/thread/723273), [Android Credential Manager WebView](https://developer.android.com/identity/sign-in/credential-manager-webview)). Systembrowser-Komponenten (ASWebAuthenticationSession, SFSafariViewController, Custom Tabs) unterstützen WebAuthn [S] ([passkeys.dev](https://passkeys.dev/docs/reference/ios)).
7. **Bürgerkarte/lokale BKU ist ein Auslaufmodell**: MOCCA wird seit Ende 2021 nicht mehr weiterentwickelt (letzte Version 1.4.3 vom 09.09.2020) [W] ([technology.a-sit.at](https://technology.a-sit.at/?p=3914)); A-Trust-Signaturkarten werden aber weiter verkauft und dienen v. a. als **zweiter Faktor für ID Austria** [W] ([A-Trust Signaturkarte](https://a-trust.at/en/products/id_and_signature_cards/signature_card/)). Für den Klon: optionales Desktop-Plugin, niedrige Priorität [I].
8. **Platzhalter und Prüfung lassen sich entkoppeln**: Der QR-Platzhalter ist ein simpler String `PDF-AS-POS;id=…;profile=…` [C] (`pdf-as-4/pdf-as-web/src/main/java/at/gv/egiz/pdfas/web/servlets/PlaceholderGeneratorServlet.java:69-90`) – lokal erzeugbar. Die RTR-Signaturprüfung hat eine SOAP-Schnittstelle, die aber **authentifizierungspflichtig** ist (HTTP 401) [M]; die Web-App ist per CSP nur in `rtr.at` einbettbar [M].
9. **EU-Ausblick verlangt eine Backend-Abstraktion**: eIDAS 2.0 (VO (EU) 2024/1183) verpflichtet Wallets zu QES „by default and free of charge“ (Art. 5a(5)(g)), Mitgliedstaaten dürfen auf nicht-berufliche Zwecke einschränken (ErwG 20) [S] (zitiert im [CSC-Whitepaper 2024](https://cloudsignatureconsortium.org/wp-content/uploads/2024/10/CSC-White-Paper-The-EU-digital-identity-wallet-a-new-tool-for-remote-signing-with-qualified-electronic-signatures.pdf)); Österreich entwickelt ID Austria schrittweise zur EUDI-Wallet weiter [S] ([Parlament AB 5451](https://www.parlament.gv.at/dokument/XXVIII/AB/5451/imfname_1769922.pdf)); A-Trust hat im SL-iframe bereits Parameter `showwallet`/`usewallet` für einen EUDI-Wallet-Signaturfluss [W] ([docs.a-trust.at transport](https://docs.a-trust.at/apis/SecurityLayer/transport)).
10. **Rechtlich braucht die Client-Software keine Zertifizierung** – zertifiziert wird nur die qualifizierte Signaturerstellungseinheit (QSCD), bei Fernsignatur also das HSM bei A-Trust [I] (abgeleitet aus eIDAS Art. 3(12)/Art. 30 und dem Inhaltsverzeichnis des SVG, siehe §9). Die „vertrauenswürdige Anzeige“ übernimmt im SL-Fluss A-Trust (Link auf das PDF) [W]/[C]; im CSC-Hash-Fluss rückt die Darstellungsverantwortung zur Client-Software [I]. Datenschutz: im SL-Fluss geht **das ganze PDF** an A-Trust (Löschung spätestens nach 24 h laut Datenschutzerklärung „Signaturdienst“) [C]/[W].

---

## 1. Ziel, Umfang, Methode

- Ziel: belastbare Faktenbasis für die Wahl der **Signatur-Backends** eines nicht-Java-Klons von PDF-Over (Desktop Windows/macOS/Linux, iOS, Android, optional Web).
- Umfang: A-Trust/ID-Austria-Schnittstellen, Authentifizierungsfaktoren, Bürgerkarte/lokale BKU, Mobile-Integration, Web-Integration (CORS), Platzhalter-Service, Signaturprüfung, EU-Ausblick, rechtlich-organisatorischer Rahmen.
- Methode: Web-Recherche (WebSearch/WebFetch), Lesen öffentlicher Spezifikationen und A-Trust-Beispielcodes auf GitHub, Abgleich mit dem lokalen PDF-Over/PDF-AS-Code, harmlose HTTP-Messungen (Anhang A). **Keine** Formulare abgeschickt, **keine** Zugangsdaten verwendet.
- Abgrenzung: Die Details der PDF-AS-Signaturerzeugung (PAdES-Aufbau, Signaturblock, Profile) sind Gegenstand anderer Analyse-Dokumente; hier nur soweit sie die Backend-Schnittstelle betreffen.

---

## 2. Ausgangslage: Was PDF-Over heute an Backends nutzt (Code-Befund)

### 2.1 Backend-Arten

| Enum `BKUs` | Bedeutung | Endpunkt / Mechanismus | Fundstelle |
|---|---|---|---|
| `LOCAL` | Lokale Bürgerkartenumgebung (Smartcard) | `http://127.0.0.1:3495/http-security-layer-request` (SL 1.2 HTTP-Binding) | `pdf-over-commons/src/main/java/at/asit/pdfover/commons/BKUs.java:24`, `Constants.java:105` [C] |
| `MOBILE` | ID Austria (früher Handy-Signatur) über A-Trust | `https://service.a-trust.at/mobile/https-security-layer-request/default.aspx` (SL 1.2 HTTPS-Binding); Test: `https://hs-abnahme.a-trust.at/...` | `BKUs.java:27`, `Constants.java:108-109` [C] |
| `KS` | Software-Keystore (PKCS#12/JKS) | lokal, ohne BKU | `BKUs.java:30` [C] |

Hinweis [I]: Ein Software-Keystore ist **keine QSCD** – damit lassen sich fortgeschrittene, aber keine qualifizierten Signaturen erzeugen (eIDAS Art. 3(12) verlangt eine QSCD, siehe §9.1).

### 2.2 Ablauf Richtung A-Trust (MOBILE)

1. PDF-AS erzeugt zuerst einen `InfoboxReadRequest` (Zertifikat lesen), dann einen `CreateCMSSignatureRequest` (SL 1.2, XML) [C] (`pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4BKUSLConnector.java:69`, `:113-119`).
2. Für MOBILE wird der „Upload“-Modus erzwungen (`useBase64Request = false`) [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/MobileBKUState.java:524`; `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4Signer.java:125-128`). Der XML-Request referenziert die Signaturdaten als `formdata:fileupload` [C] (`pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl/util/BaseSLConnector.java:112`); hochgeladen wird **das PDF mit ausgeschwärztem Signaturplatzhalter** (`blackOutSignature`) [C] (`BaseSLConnector.java:97`, `PdfAs4BKUSLConnector.java:116-117`).
3. Erster HTTP-Request: `multipart/form-data` mit `fileupload` (application/pdf, Dateiname `sign.pdf`) und `XMLRequest`; ohne Datei: `application/x-www-form-urlencoded` mit `XMLRequest` [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/MobileBKUConnector.java:201-211`). **Kein `DataURL`, kein `RedirectURL`** [C] – die SL-Antwort kommt am Ende als `*/xml` direkt in der HTTP-Antwort zurück (`ATrustParser.java:365-366`, `MobileBKUConnector.java:165`).
4. Dazwischen: Redirects (301/302, `Refresh`-Header, `<meta http-equiv=refresh>`) werden selbst verfolgt, mit Endlosschleifenschutz (50 Requests in < 2 s) [C] (`MobileBKUConnector.java:74`, `:103-105`, `:132`, `:155`).
5. HTML-Seiten werden mit jsoup analysiert und als „Blöcke“ erkannt (CSS-Selektoren): `#handynummer`/`#signaturpasswort` (Login), `#input_tan`/`#vergleichswert` (SMS-TAN), `#qrimage` + `#jsLongPoll script` (QR), `#smartphoneAnimation, #biometricimage, .ida-signtype #SignType` (App-Bestätigung), `#fidoBlock`/`#credentialOptions`/`#credentialResult` (FIDO2), Links `#SmsButton`, `#FidoButton`, `ShowSigobj.aspx` (Signaturdaten anzeigen) [C] (`ATrustParser.java:149-151`, `:167-169`, `:182-186`, `:198-201`, `:213-215`, `:326-328`).
6. Warten auf App/QR: Long-Polling auf eine aus dem Seiten-JavaScript extrahierte URL mit `Authorization: Bearer <sid>`; JSON-Felder `Fin`/`Wait`/`Error`; Timeout 5 min [C] (`ATrustParser.java:53-71`, `MobileBKUConnector.java:255-268`, `:305`).
7. FIDO2: PDF-Over führt die WebAuthn-Assertion **nativ** aus (Bibliothek `at.a-sit:webauthn-client-java:0.0.1`) und setzt dabei die Origin `https://service.a-trust.at` [C] (`pdf-over-gui/pom.xml:72-74`, `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/mobilebku/MobileBKUFido2Composite.java:61`); das Ergebnis wird als JSON in ein A-Trust-Formularfeld geschrieben und abgeschickt (`MobileBKUConnector.java:437-445`). Unter Linux/macOS verweist die GUI auf libfido2 [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/Fido2ConfigurationComposite.java:40`, `pdf-over-gui/src/main/resources/at/asit/pdfover/gui/messages_de.properties:121`).
8. Zugangsdaten: Benutzername wird persistent gespeichert, das Passwort nur als „Overlay“ bis zum Neustart [C] (`MobileBKUState.java:230-235`, `messages_de.properties:216`). Zusätzlich existiert ein CLI-Argument für das ID-Austria-Passwort [C] (`messages_de.properties:66`).
9. HTTP-Client: Apache HttpClient 5 mit System-Properties, **ohne eigenen User-Agent** im Mobile-Pfad [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/utils/HttpClientUtils.java:7-9`); der lokale BKU-Pfad setzt `http.useragent = APP_NAME_VERSION` [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/BKUHelper.java:65-66`).

### 2.3 Lokale BKU

- Erreichbarkeit wird per TCP-Connect auf `127.0.0.1:3495` geprüft (alle 5 s, bei Erfolg alle 30 s) [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/LocalBKUConnector.java:45-55`).
- Vor der Signatur ein `NullOperationRequest`; enthält der `Server`-Header `trustDeskbasic` oder `asignSecurityLayer`, wird Base64-Inline statt Upload verwendet [C] (`pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/LocalBKUState.java:104-118`). Daraus folgt [I]: PDF-Over kennt drei lokale BKUs – trustDesk basic (IT Solution), a.sign Security Layer (A-Trust) und (implizit, Upload-Modus) MOCCA.
- Signatur-Request als multipart mit `XMLRequest` und `fileupload` [C] (`LocalBKUConnector.java:84-98`).

### 2.4 Datenschutzhinweis im Original

PDF-Over erklärt selbst: lokale Verarbeitung, bei Bürgerkarte lokal signiert, **bei ID Austria wird das PDF an A-Trust gesendet** [C] (`messages_de.properties:113`, `Readme.md:20-26`). Das ist exakt durch Schritt 2 (Upload-Modus) technisch begründet [C].

---

## 3. ID Austria und die A-Trust-Schnittstellen

### 3.1 Begriffe und Rollen

| Begriff | Bedeutung | Beleg |
|---|---|---|
| ID Austria | Staatliche eID mit integrierter qualifizierter Fernsignatur; Weiterentwicklung/Ablöse von Handy-Signatur und Bürgerkarte | [W] [id-austria.gv.at generelle Info](https://www.id-austria.gv.at/hilfe/hilfe-zu-ida/generelle-info) |
| Handy-Signatur | Vorgänger; am 05.12.2023 abgelöst | [W]/[S] [A-SIT Bescheinigung (Suchtreffer)](https://a-sit.at/downloads/2156), [BKA](https://www.bundeskanzleramt.gv.at/agenda/digitalisierung/digitales-oesterreich/id-austria.html) |
| Vollfunktion / Basisfunktion | Basisfunktion (aus Handy-Signatur-Umstieg ohne behördliche Registrierung) ist eingeschränkt, nicht verlängerbar, erlaubt aber noch SMS-TAN | [W] [Authentifizierungsfaktoren](https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren) |
| VDA (A-Trust) | Führt Identifikation/Authentifizierung durch und erzeugt die qualifizierte Signatur (auch als Authentifizierungsnachweis) | [W] [Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse) |
| IdP (Shibboleth) | ID-Austria-Login-Plattform für Service Provider (SAML2, OIDC) – **nicht** für Dokumentsignaturen | [W] [Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse) |
| xIDENTITY (früher EU-Identity) | A-Trust-QES für Personen ohne österreichischen Wohnsitz | [W] [A-Trust Presse 10.11.2025](https://www.a-trust.at/downloads/documents/Pressemeldungen/PM_A-Trust_Sec_f._alle_Groessen.pdf) |

Verbreitung: A-Trust nennt > 4 Mio. aktive QES-Nutzer:innen (z. B. ID Austria) [W] (Presse 10.11.2025); die APA meldete > 3,9 Mio. ID-Austria-Registrierungen und > 500 integrierte Anwendungen (Juni 2025) [S] ([ORF](https://orf.at/stories/3397644/)).

### 3.2 Security Layer 1.2 – HTTPS-Binding („Mobile BKU“, von PDF-Over genutzt)

**Was A-Trust offiziell unterstützt** [W] ([Application Interface](https://docs.a-trust.at/apis/SecurityLayer/application), [Transport](https://docs.a-trust.at/apis/SecurityLayer/transport)):

- Befehle: Signaturerstellung allgemein, **CMS** (`CreateCMSSignatureRequest`), **XMLDSIG**, InfoboxRead **nur Certificates-Infobox**, NullOperation.
- Transport: **nur das HTTPS-Binding**; `StylesheetURL` und `PushInfobox` werden **nicht** unterstützt.
- Zusätzliche Darstellungsparameter für die iframe-Einbindung: `appletwidth`, `appletheight`, `width`, `height`, `backgroundcolor`, `redirecttarget` (Ziel-Frame bei `RedirectURL`, z. B. `_parent`), `telephonenumber` (Vorbelegung), **`showwallet`** (Button für EUDI-Wallet, Beispiel „Valera“), **`usewallet`** (springt direkt in den EUDI-Wallet-Signaturfluss). Getestete iframe-Größen: 300×300, 350×350 (empfohlen), 450×450 px.
- Hinweis: Die Transport-Seite schreibt den Host als „serivce.a-trust.at“ (Tippfehler in der Doku) [W].

**Pflichten/Verbote für Drittanwendungen** [W] ([SecurityLayer](https://docs.a-trust.at/apis/SecurityLayer/), [A-Trust/Handy-Signatur README](https://github.com/A-Trust/Handy-Signatur)):

- Telefonnummer/Benutzername, Signaturpasswort und TAN sind **immer auf A-Trust-Seiten** einzugeben (Präfixe `https://service.a-trust.at/mobile/` bzw. `https://www.a-trust.at/mobile/`).
- Parsen der Seite bzw. automatisches Ausfüllen per Programm/Skript ist **nicht erlaubt**; Verweis auf die Nutzungsvereinbarung der Fernsignatur.
- Lokale Anwendungen müssen einen **lokalen Webbrowser oder ein Browser-Control** verwenden; Referenz-Demo: [A-Trust/HandySignaturClientDemo](https://github.com/A-Trust/HandySignaturClientDemo) (C#, MIT, letzter Push 2021) [W]/[M].
- Für Batch-Signaturen (mehrere Dokumente mit einer Auslösung) verweist A-Trust auf die **Signatur-Box** [W] (README A-Trust/Handy-Signatur).

**Konflikt mit PDF-Over** [C]+[W]: PDF-Over füllt Benutzername/Passwort/TAN in eigenen SWT-Dialogen ein und schickt die A-Trust-Formulare selbst ab (`MobileBKUConnector.java:367-385`). Ob A-SIT dafür eine Sondervereinbarung hat, ist öffentlich nicht belegt (Q1). Die Konsequenz für den Klon [I]: **Standardpfad = A-Trust-Seite in einer (eingebetteten oder System-)Browser-Komponente anzeigen**; native Login-/TAN-Masken nur mit schriftlicher Zustimmung von A-Trust.

**Protokollsemantik (SL 1.2, HTTP/HTTPS-Binding)** [W] ([Bindings](https://sl.eid.egiz.gv.at/konzept/securitylayer/spezifikation/20140114-en/bindings/Bindings.en.html)):

| Parameter | Wirkung | Relevanz für den Klon |
|---|---|---|
| `XMLRequest` (Pflicht) | SL-Befehl als XML | identisch zu PDF-Over |
| `DataURL` | CCE (hier: A-Trust-Server) **POSTet die Antwort an diese URL** (multipart, Felder `ResponseType`, `XMLResponse`, `BinaryResponse`); Antwort des Servers steuert den weiteren Ablauf (`<ok/>`, neuer Befehl, 307, …) | erfordert öffentlich erreichbaren Server (A-Trust → Server) [I] |
| `RedirectURL` | Browser wird (bei server-basierter CCE nach der Benutzerinteraktion [I]) zu dieser URL umgeleitet | nur sinnvoll mit `DataURL` |
| weder noch | Antwort (`text/xml`) geht in die **Browser-Verbindung** zurück | so arbeitet PDF-Over [C]; im Browser-Control abfangbar [I] |
| Felder mit Suffix `_` / `__` | werden als Formfeld/Header an `DataURL` weitergereicht | Korrelation (Session-ID) mit Backend [I] |

**Registrierung**: Für das SL-Endpoint ist **technisch keine Client-Registrierung** nötig (öffentliches Formular-POST) [I]. Organisatorisch gelten aber die Nutzungsbedingungen (s. o.) und ggf. Regeln zur gewerblichen Nutzung (§3.6).

### 3.3 Security Layer 2.0 (JSON/JOSE)

- SL 2.0 wird im ID-Austria-System **zwischen IdP und VDA** verwendet: Der IdP schickt ein SL-2.0-Kommando an den VDA, um Identifikation/Authentifizierung zu starten [W] ([Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse)).
- PDF-AS enthält einen SL-2.0-Connector mit JOSE (JWS/JWE): Content-Types `application/sl2.0;command` / `application/sl2.0;result`, Befehle u. a. `redirect`, `call`, `error`, `qualifiedeID`, `getCertificate`, `createCAdES`, `createBindingKey`, `storeBindingCert`, Authentifizierungsfaktoren `idAndPassword`, `jwsTokenAuth`, `qrCodeFactor`; Header `SL2ClientType` mit Wert `nativeApp`, Parameter `IPCRedirect` [C] (`pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/sl20/utils/SL20Constants.java:14-22`, `:72-123`).
- In PDF-AS-Web ist SL 2.0 auskommentiert und braucht einen **eigenen Keystore zum Signieren/Verschlüsseln** der SL-2.0-Nachrichten [C] (`pdf-as-4/pdf-as-web/src/main/configuration/pdf-as-web.properties:99-113`).
- **Bewertung** [I]: SL 2.0 ist ein Protokoll für vertraglich angebundene, vertrauenswürdige Gegenstellen (Schlüsselaustausch mit dem VDA). Eine öffentliche Spezifikation für Drittanwendungen wurde nicht gefunden; für den Klon **kein Kandidat**, außer A-Trust/A-SIT bieten es ausdrücklich an (Q3).

### 3.4 CSC-API v1.4 („Hash-Signing“) – der offizielle REST-Weg

Belegt [W] ([docs.a-trust.at/apis/CSC](https://docs.a-trust.at/apis/CSC), [Presse 01.12.2023](https://a-trust.at/en/about_us/news/20231201_en_post.html), [Partner](https://a-trust.at/en/partners)):

- Implementiert die **Cloud Signature Consortium API Version 1.4**: Credential-Discovery, Autorisierung, Remote-Signaturerstellung; QES über die A-Trust-Fernsignatur.
- Testsystem: `https://testbox.a-trust.at/hashsigning/2/csc/v1/` (nicht rechtsgültig; Test-Zugangsdaten öffentlich dokumentiert, hier nicht übernommen).
- Nur der Hash geht an A-Trust; das Dokument verlässt die Partnersoftware nicht.
- Zugang nur als **Partner** (Vertrieb); Kosten nicht veröffentlicht.

Ablauf laut A-Trust-Beispielclient [G] ([A-Trust/CSC_HashSignClient](https://github.com/A-Trust/CSC_HashSignClient), C#, .NET 4.8):

| Schritt | Aufruf | Fundstelle |
|---|---|---|
| 0 | `account_token` = HS256-JWT mit `sub`=subscriberId, `azp`=clientId, Schlüssel = SHA-256(clientSecret) | `classes/TokenGenerator.cs:19-35` |
| 1 | Browser: `oauth2/authorize?response_type=code&scope=service&client_id&redirect_uri&state&lang&account_token` | `HashSignHelper.cs:59-70` |
| 2 | `oauth2/token` (authorization_code, **client_secret**) → access_token | `HashSignHelper.cs:96-110` |
| 3 | `credentials/list`, `credentials/info` (Zertifikatskette) | `HashSignHelper.cs:43`, `:147-160` |
| 4 | Client baut **CMS-SignedAttributes für PAdES** selbst, Hash = SHA-256(SignedAttributes) | `GUIHashSigner.cs:237-253` |
| 5 | Browser: `oauth2/authorize?scope=credential&credentialID&numSignatures&hash=…` (mehrere Hashes → **Batch mit einer Freigabe**) | `HashSignHelper.cs:212-224` |
| 6 | `oauth2/token` → **SAD** | `HashSignHelper.cs:178-190` |
| 7 | `signatures/signHash` mit `credentialID`, `SAD`, `hash[]` → Signaturwerte | `HashSignHelper.cs:196-201` |
| 8 | Client baut CMS `SignedData` und bettet sie ins PDF ein | `CmsSigBuilder.cs` (Datei vorhanden) |

Konsequenzen [I]:
- **Architekturunterschied zu SL 1.2**: Bei SL liefert A-Trust ein fertiges CMS-`SignedData`; bei CSC liefert A-Trust nur den rohen Signaturwert – **der Klon muss SignedAttributes (inkl. ESS signing-certificate-v2) und SignedData selbst bauen** und das Zertifikat vorher abholen.
- **client_secret** im Token-Request ⇒ ein öffentlicher Client (Desktop/Mobile/Web) darf das Secret nicht enthalten ⇒ kleines **Backend (Token-Proxy/BFF)** nötig, oder A-Trust erlaubt Public Clients mit PKCE (Q4).
- Zwei Browser-Interaktionen (Service- und Credential-Autorisierung) – ob jeweils volle ID-Austria-Anmeldung nötig ist, ist offen (Q5).
- CSC-Version: A-Trust = v1.4; der Markt und die EU-Wallet-Durchführungsrechtsakte orientieren sich an der CSC-API (laut CSC-Whitepaper; dort „V 2.0“ als Mehrheitsstandard genannt) [S] – ein generischer CSC-Client sollte v1 und v2 abstrahieren.
- Beim Testendpoint keine CORS-Header (OPTIONS → 405) [M] ⇒ auch hier kein Direktzugriff aus einer Web-App.

### 3.5 Signatur-Box / Hash2Sign (Enterprise)

- Signature-Server-API (REST, OpenAPI „A-Trust Signature Box API 2.0“): Batch eröffnen (`/v2/signaturebatches`), Dokumente hinzufügen, `/v2/signaturebatches/{ticket}/mobileSignature` liefert eine **Redirect-URL zur Fernsignatur (SecurityLayer-iframe-Parameter)**, alternativ Siegel; Authentifizierung per Header `x-api-key` [W] ([Swagger signaturbox.yaml](https://docs.a-trust.at/swagger/signaturbox.yaml), [Signature Server API](https://docs.a-trust.at/apis/SignaturServer)).
- Hash2Sign: Software bereitet Dokumente vor, nur Hashwerte verlassen die IT-Umgebung; REST-Schnittstelle; Batch, qualifizierter Zeitstempel, LTV, Firmensiegel [W] ([Hash2Sign](https://a-trust.at/en/products/signature_solutions_for_companies/hash2sign_software/), Presse 10.11.2025).
- Place2Sign: Web-App zum Platzieren von Signaturfeldern, liefert Koordinaten an die Signature-Server-API [W] ([Place2Sign](https://docs.a-trust.at/apis/Place2Sign)).
- Bewertung [I]: Für Unternehmen/Behörden als optionales Backend interessant (Batch!), für Privatnutzer:innen eines Desktop-Tools nicht passend (API-Key, Vertrag).

### 3.6 PDF Sign, A-Trust-Konto, „Paket gewerbliche Nutzung“ (Nutzungsrecht!)

- ID Austria und xIDENTITY sind für **Privatpersonen** kostenlos; mit dem „Paket gewerbliche Nutzung“ können kleine Unternehmen über PDF Sign oder das A-Trust-Konto „unter voller Compliance“ signieren, inkl. SLA [W] (Presse 10.11.2025; [Commercial use](https://a-trust.at/en/products/qualified_e-signature_services/commercial_use/)). Für die Anbindung an die Signaturschnittstelle ist laut Produktseite eine **Dienstleistervereinbarung** nötig [W].
- A-Trust PDF Sign (Web-Tool): kostenlos nur für Privatpersonen, < 20 MB, kein Batch, Dokumente spätestens 1 h nach letztem Zugriff gelöscht, kein qualifizierter Zeitstempel [W] ([a-trust.at/pdfsign](https://www.a-trust.at/pdfsign/)).
- Bewertung [I]: Ob die gewerbliche Nutzung der kostenlosen ID-Austria-Signatur über **Dritt-Software** (wie PDF-Over oder einen Klon) vertraglich zulässig ist, ist **nicht eindeutig belegt** (die Seiten ordnen „kostenlos“ Privatpersonen zu, regeln Dritt-Clients aber nicht) – Q6. Der Klon sollte Business-Nutzer:innen auf die Bedingungen hinweisen und keine Zusage machen.

### 3.7 ID-Austria-Login (OIDC/SAML) – nur für Authentifizierung

- OIDC: nur `response_type=code`, Authorize-Endpoint `https://idp.id-austria.gv.at/auth/idp/profile/oidc/authorize`, **Client-Secret verpflichtend**, PKCE technisch unterstützt [W] ([Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse), [OIDC für Mobile Apps](https://www.id-austria.gv.at/de/developer/anbinden/openid-connect-f%C3%BCr-mobile-apps)).
- Service Provider müssen sich registrieren (Service Owner + Service Provider, IDA-SPR); private SPs sind möglich [W] ([Developer](https://www.id-austria.gv.at/de/developer)).
- Für den Klon nur relevant, falls ein eigenes Backend Nutzerkonten braucht (z. B. für Batch-Jobs) [I] – eine **Dokumentsignatur liefert dieser Login nicht**.

### 3.8 Vergleich der Integrationspfade

| # | Pfad | QES mit ID Austria | Was geht an Dritte | Vertrag nötig | Batch | FIDO2 | Desktop | Mobile | Web (ohne Backend) | Einschätzung [I] |
|---|---|---|---|---|---|---|---|---|---|---|
| P1 | SL 1.2 HTTPS + **A-Trust-Seite im Browser-Control** | ja [W] | ganzes PDF an A-Trust [C] | nein (Nutzungsbedingungen) [I] | nein (1 Dok./Auslösung) [W] | nur wo Web-Komponente WebAuthn für fremde RP kann (§5) | ja | ja, mit Einschränkungen | nein (CORS/DataURL) [M] | **Standardpfad v1** |
| P2 | SL 1.2 HTTPS + **natives Scraping** (PDF-Over-Weg) | ja | ganzes PDF | **Zustimmung A-Trust nötig** [W] | nein | ja (nativ, wie PDF-Over) | ja | ja | nein | nur nach schriftlicher Freigabe |
| P3 | **CSC v1.4 Hash-Signing** | ja (A-Trust-QES) [W] | nur Hash [W] | **ja (Partner)** [W] | ja (`numSignatures`) [G] | im Systembrowser ja [I] | ja | **gut** (OAuth im Systembrowser) | nein (CORS, client_secret) | **Zielpfad v2**, abhängig vom Vertrag |
| P4 | Signatur-Box / Hash2Sign REST | ja | PDF bzw. Hash an Box | ja (API-Key) [W] | ja [W] | über A-Trust-Seite | über Backend | über Backend | über Backend | nur Enterprise-Variante |
| P5 | PDF-AS-Web (EGIZ-Instanz) | ja | PDF an EGIZ + A-Trust | Fair Use, kein SLA [W] | „Bulk-Light“ (SOAP) [C-Doku] | über A-Trust-Seite | – | – | ja (Formular/Redirect) | nur Fallback/Demo |
| P6 | SL 2.0 | ja | – | ja (Schlüssel/Trust) [I] | ? | ? | ? | ? | ? | nicht verfügbar |
| P7 | Lokale BKU (SL 1.2 HTTP, localhost) | Karte (QES) | nichts | nein | nein | – | ja | nein | nein | Legacy-Plugin |
| P8 | PKCS#12/PKCS#11 lokal | nur mit QSCD-Karte über PKCS#11 [I] | nichts | nein | ja | – | ja | eingeschränkt | nein | für Tests/fortgeschrittene Signatur |
| P9 | EUDI-Wallet (via A-Trust `usewallet` oder OpenID4VP + CSC) | künftig | abhängig vom Modell | ? | ? | – | ? | ja | ? | Erweiterungspunkt |

---

## 4. Authentifizierungsfaktoren und ihre Einbindung

| Faktor | Status 2026 | Wie PDF-Over ihn behandelt | Einbindung im Klon [I] | Belege |
|---|---|---|---|---|
| Benutzername + Signaturpasswort | immer erforderlich (erster Faktor) | eigener Dialog, Formular selbst befüllt [C] (`MobileBKUConnector.java:367-373`) | auf A-Trust-Seite eingeben lassen (P1) | [W] [docs.a-trust.at](https://docs.a-trust.at/apis/SecurityLayer/) |
| App „ID Austria“ (Push, Biometrie oder Geräte-PIN) | Standard-Zweitfaktor der Vollfunktion; App seit 20.06.2025 Nachfolger von „Digitales Amt“; iOS ≥ 16, Android ≥ 12; Root-Geräte nicht unterstützt | „Warten auf App“-Block + Long-Poll [C] (`ATrustParser.java:192-202`) | A-Trust-Seite zeigt Warten/Vergleichswert; App öffnet sich per Push; offene Signaturen auch manuell abrufbar | [W] [App ID Austria](https://www.id-austria.gv.at/de/verwenden/app-id-austria), [S] [ORF](https://orf.at/stories/3397644/), [USP](https://www.usp.gv.at/hilfe-und-support/anmeldung/id-austria-app-neu.html) |
| App „A-Trust Signatur“ | „derzeit“ alternative Authentifizierungs-App; seit 07/2023 **kein QR-Code- und kein Push-TAN-Signieren** mehr, dafür Geräte-PIN/Biometrie | – | wie oben | [W] [Authentifizierungsfaktoren](https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren), [A-Trust News 27.07.2023](https://a-trust.at/en/about_us/news/20230727_en_post%20copy.html) |
| QR-Code (App scannt) | PDF-Over unterstützt „App-QR“ (`#qrimage`) [C] (`ATrustParser.java:174-189`, `Readme.md:13`); A-Trust erklärte 2023 QR-Signieren in der A-Trust-App für beendet | QR-Dialog + Long-Poll [C] | Status unklar (Q7) – im Browser-Control ohnehin A-Trust-Sache | [C], [W] |
| SMS-TAN | **nicht** für Vollfunktion; nur Basisfunktion (eingeschränkt, nicht verlängerbar) | TAN-Dialog, Vergleichswert [C] (`ATrustParser.java:156-171`) | auf A-Trust-Seite | [W] [Authentifizierungsfaktoren](https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren) |
| FIDO2-Sicherheitsschlüssel | mind. **FIDO2 Level 2**, WebAuthn; nur von A-Trust gelistete Token (GoTrustID, Swissbit, Thales, TrustKey, Yubico ≥ FW 5.7); USB-A/USB-C; Windows mit Chrome/Firefox empfohlen; nicht teilbar; Reset nur telefonisch beim VDA | nativ (libfido2 / Windows-API vermutet), Origin `https://service.a-trust.at` [C] | Desktop: Systembrowser oder Browser-Control mit WebAuthn; Mobile: nur Systembrowser-Komponente (§5.3) | [W] [Authentifizierungsfaktoren](https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren), [A-Trust Komponentenliste 1.7.28 §6](https://www.a-trust.at/docs/verfahren), [C] `Readme.md:13` |
| Signaturkarte (A-Trust) als Zweitfaktor | weiterhin erhältlich (Karte 38,81 €, Registrierung 15,52 €, Nutzung 20,18 €/Jahr inkl. USt.), Kartenleser nötig, „Signaturkarte hinzufügen“ in der ID-Austria-Verwaltung | – (PDF-Over kennt Karte nur als lokale BKU) | über A-Trust-Seite (Browser) [I] – Mechanismus der Kartenansprache im Browser unklar (Q8) | [W] [Signaturkarte](https://a-trust.at/en/products/id_and_signature_cards/signature_card/), [Authentifizierungsfaktoren](https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren) |
| Biometrie | kein eigener Faktor, sondern Entsperrung der App; biometrische Daten verlassen das Gerät nicht | – | – | [W] [generelle Info](https://www.id-austria.gv.at/hilfe/hilfe-zu-ida/generelle-info) |

Hinweis: Die FIDO-Info-URL in PDF-Over (`messages_de.properties:123`, `https://www.oesterreich.gv.at/id-austria/haeufige-fragen/allgemeines-zu-id-austria#fido`) leitet inzwischen per 301 auf `id-austria.gv.at/hilfe/hilfe-zu-ida/generelle-info` um, wo FIDO nicht mehr erwähnt wird [M]. Der Klon sollte auf die Seite „Authentifizierungsfaktoren“ verlinken [I].

Sorgfaltspflichten: A-Trust empfiehlt, Browserfunktionen zum Speichern von Aktivierungsdaten (Autovervollständigung, Passwortspeicher) zu deaktivieren [W] ([Komponentenliste 1.7.28 §3.1](https://www.a-trust.at/docs/verfahren)). ⇒ Der Klon darf das Signaturpasswort **nicht persistent speichern** und im Browser-Control Autofill/Passwortspeicher deaktivieren [I] (PDF-Over hält es nur im Speicher, §2.2 Nr. 8).

---

## 5. Bürgerkarte und lokale Bürgerkartenumgebung (Status 2026)

| Komponente | Status | Beleg |
|---|---|---|
| MOCCA (EGIZ, Open Source) | aktive Weiterentwicklung **Ende 2021 eingestellt**; letzte Version 1.4.3 (09.09.2020) für Windows/Linux/macOS | [W] [technology.a-sit.at](https://technology.a-sit.at/?p=3914) |
| a.sign Client (A-Trust) | jüngste gefundene Doku: Installation Guide und Benutzerhandbuch **Version 3.1 vom 16.11.2023**; Windows-orientiert (CSP, PKCS#11); kein End-of-Life-Hinweis gefunden | [S] (Suchtreffer auf [a-trust.at Installation Guide](https://a-trust.at/downloads/client/a.sign%20Client%20Installation%20Guide.pdf)) |
| A-Trust „Liste der empfohlenen Komponenten und Verfahren“ 1.7.28 (24.10.2024) | listet weiterhin a.sign client 1.3, a.sign Bürgerkartensoftware 1.x, trustDesk ≥ 4.0, SecSigner 5.0, **MOCCA** als Signaturapplikationen; Signaturkarten CardOS V5.3 QES und ACOS-IDv2.0 | [W] [a-trust.at/docs/verfahren](https://www.a-trust.at/docs/verfahren) |
| A-Trust-Signaturkarte | wird verkauft; primär Zweitfaktor für ID Austria, laut Produktseite nur Browser + Kartenleser nötig | [W] [Signaturkarte](https://a-trust.at/en/products/id_and_signature_cards/signature_card/) |
| e-card mit Bürgerkartenfunktion | **kein aktueller Beleg** gefunden, ob Aktivierung noch möglich ist; Zertifikate maximal 5 Jahre gültig | [S] (Parlamentsanfragen, ältere Quellen) |
| PDF-AS-WAI (pdf.egiz.gv.at) | bietet neben „ID Austria / Mobile Phone Signature“ weiterhin **„Signature Card“** an | [M] |
| Signaturkarten als ID-Austria-Zweitfaktor | nur A-Trust-Karten (Stand 2023) | [S] ([oesterreich.gv.at PDF 2023](https://www.oesterreich.gv.at/dam/jcr:d178e5e1-1f21-4470-a731-563e86df6e08/20230828-IDA-eDA-Multiple-Konten-TagX.pdf)) |

Bewertung [I]:
- Die lokale BKU ist für Neuinstallationen 2026 kaum noch relevant (MOCCA tot, a.sign Client Windows-zentriert, Karten primär als ID-Austria-Faktor).
- Bestehende Kartenbesitzer:innen und Behörden-Arbeitsplätze mit trustDesk/a.sign können sie noch nutzen. **Empfehlung:** Backend „LocalBKU (SL 1.2 HTTP, 127.0.0.1:3495)“ als **optionales Desktop-Plugin** mit niedriger Priorität; Protokoll ist trivial (§2.3), Tests aber schwer (Hardware nötig).
- Alternative für Karten ohne BKU: direkter PKCS#11-Zugriff – bei A-Trust-Karten über den a.sign-Client-PKCS#11-Treiber (Windows) [S]; Verfügbarkeit für macOS/Linux unklar (Q8).
- Auf iOS/Android ist keine lokale BKU zu erwarten [I].

---

## 6. Mobile: Wie eine iOS/Android-App ID-Austria-Signaturen auslösen kann

### 6.1 Fakten zu App-Verknüpfungen

| Domain | Datei | Inhalt | Beleg |
|---|---|---|---|
| `idp.id-austria.gv.at` | `/.well-known/apple-app-site-association` | App-ID `…at.gv.oe.app`, Komponente `/auth/idp/profile/oidc/*` („authentication“) | [M] |
| `idp.id-austria.gv.at` | `/.well-known/assetlinks.json` | Package `at.gv.oe.app`, `handle_all_urls` | [M] |
| `www.a-trust.at` | `/.well-known/apple-app-site-association` | App-ID `…at.a-trust.Handy-Signatur`, Pfad `/app/*` | [M] |
| `www.a-trust.at` | `/.well-known/assetlinks.json` | Package `at.atrust.tanapp` | [M] |
| `service.a-trust.at` | AASA / assetlinks | **404** (keine Verknüpfung) | [M] |

Weitere Belege [W] ([OIDC für Mobile Apps](https://www.id-austria.gv.at/de/developer/anbinden/openid-connect-f%C3%BCr-mobile-apps), [Testen auf mobilen Geräten](https://www.id-austria.gv.at/de/developer/testen/testen-auf-mobilen-ger%C3%A4ten)):
- App2App für den **Login**: SP-App öffnet die OIDC-Authorize-URL; ist die App „ID Austria“ installiert und für die https-URL registriert (RFC 8252 §7.2), öffnet sie sich; Rückgabe des Codes per https-Redirect (Universal/App Link der SP-App); **nur http(s)-Redirect-URIs zulässig**; Erkennung der App über Custom-Schemes `at.gv.id-austria://` bzw. Package `at.gv.oe.app` (`LSApplicationQueriesSchemes` bzw. `<queries>`); ohne App: Custom Tab; Beispiel-App `a-sit-plus/eid-service-app`.
- Für **Signaturen über A-Trust** ist kein entsprechender Mechanismus dokumentiert; die App-Bestätigung erfolgt über Push des VDA oder direkt, wenn die Nutzerin bereits in der App ist [W] ([Prozesse](https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse)).

Schlussfolgerung [I]: Auf demselben Smartphone läuft eine Signatur so ab: Klon-App zeigt A-Trust-Seite → Login → Push-Benachrichtigung → Nutzerin wechselt in die App „ID Austria“, bestätigt (Biometrie/PIN) → wechselt manuell zurück → A-Trust-Seite (Long-Poll) schließt ab. Ein automatisches Zurückspringen ist nicht belegt (Q9).

### 6.2 Optionen für die Web-Komponente

| Komponente | A-Trust-konform („Browser-Control“) | WebAuthn/FIDO2 für `service.a-trust.at` | SL-Antwort abgreifbar | POST mit PDF möglich | Bewertung [I] |
|---|---|---|---|---|---|
| iOS **WKWebView** | ja [I] | **nein**, nur für Associated Domains der App [S] ([Apple-Forum](https://developer.apple.com/forums/thread/723273), [passkeys.dev](https://passkeys.dev/docs/reference/ios)) | ja (Navigation/Response im Delegate, Inhalt per JS lesen) [I] | ja (`URLRequest` mit Body oder lokale Auto-Submit-Seite) [I] | **Standard für App/SMS-TAN** |
| iOS **ASWebAuthenticationSession / SFSafariViewController** | ja [I] | **ja** (alle Safari-Web-Features) [S] (passkeys.dev) | **nein** (kein Zugriff auf Seiteninhalt; nur Callback-URL) [I] | nur über eine Start-URL (GET) → Auto-Submit-Seite auf eigenem Server nötig [I] | **für FIDO2**, braucht `RedirectURL`+`DataURL`-Backend |
| Android **WebView** | ja [I] | nur mit `WebSettingsCompat.setWebAuthenticationSupport(…FOR_APP)` (androidx.webkit ≥ 1.12) **und** Digital Asset Links zur eigenen Domain [W] ([Android Docs](https://developer.android.com/identity/sign-in/credential-manager-webview)) ⇒ für A-Trust **nein** [I] | ja (`WebViewClient`, JS-Bridge) [I] | `postUrl()` nur form-urlencoded [I – API-Wissen, nicht verifiziert] ⇒ Base64-Inline-Request oder Auto-Submit-Seite | **Standard für App/SMS-TAN** |
| Android **Custom Tabs / Auth Tab** | ja [I] | ja (Chrome-Engine) [I] | nein (nur Redirect zurück in die App) [I] | nur über Start-URL [I] | **für FIDO2**, wie iOS |

Folgerungen [I]:
1. **Zwei Modi** sind nötig: (a) eingebettetes Browser-Control (kein Backend nötig, kein FIDO2 auf Mobile), (b) Systembrowser + kleines Backend (`DataURL`-Empfänger + Auto-Submit-Startseite) für FIDO2. Alternativ FIDO2 auf Mobile in v1 weglassen (PDF-Over hat FIDO2 nur als Option, `messages_de.properties:115-125`).
2. Der **CSC-Pfad (P3)** passt natürlich zu Mobile: OAuth2 im Systembrowser, Redirect per Universal/App Link, kein Abgreifen von Seiteninhalten nötig – aber Vertrag und Token-Backend (client_secret).
3. Nativ-FIDO2 wie in PDF-Over (eigene CTAP2-Implementierung über NFC/USB mit gesetzter Origin) wäre technisch denkbar (z. B. Hersteller-SDKs), setzt aber Variante P2 (Scraping) voraus und ist damit ohne A-Trust-Zustimmung nicht zulässig [I].
4. iOS: `ASWebAuthenticationSession` unterstützt laut Apple ab iOS 17.4 auch https-Callbacks [I – nicht in dieser Recherche verifiziert]; ansonsten Custom-Scheme-Callback.
5. Desktop-Pendant: Für WebView-basierte Frameworks (Tauri/WebView2/WKWebView/WebKitGTK) gilt dieselbe FIDO2-Problematik auf macOS/Linux (siehe Dokument 01, §5.1) [I]; der Systembrowser + Loopback/DataURL-Weg ist dort die konforme Alternative.

---

## 7. Reine Web-App: CORS, PDF-AS-Web, eigener Proxy

### 7.1 CORS-Messung (2026-10-14, Origin `https://example.org`) [M]

| Endpoint | OPTIONS (Preflight) | GET | Folgerung |
|---|---|---|---|
| `https://service.a-trust.at/mobile/https-security-layer-request/default.aspx` (Produktion) | 200, `allow: OPTIONS, TRACE, GET, HEAD, POST`, **kein** `Access-Control-Allow-Origin` | 302 → `InvalidRequest.aspx`, kein ACAO | `fetch`/XHR-Antworten für fremde Origins unlesbar |
| `https://www.a-trust.at/mobile/https-security-layer-request/default.aspx` | 200, kein ACAO | 302, kein ACAO | dito |
| `https://hs-abnahme.a-trust.at/mobile/https-security-layer-request/default.aspx` (Test) | 200, **`access-control-allow-origin: *`** | 302, `access-control-allow-origin: *` | **Test ≠ Produktion** – Tests mit dem Testsystem sind für CORS nicht aussagekräftig |
| `InvalidRequest.aspx` (Produktion) | – | 200, **kein** `X-Frame-Options`/`frame-ancestors` | iframe-Einbettung technisch möglich (passt zur iframe-Doku) |
| `https://testbox.a-trust.at/hashsigning/2/csc/v1/info` (CSC-Test) | 405, kein ACAO | 200, kein ACAO | CSC nur über Backend |
| `https://apps.egiz.gv.at/pdf-as-web/Sign` | 200, ACAO **spiegelt Origin**, `Allow-Credentials: true` | 200, dito | PDF-AS-Web erlaubt Cross-Origin |
| `https://apps.egiz.gv.at/pdf-as-web/placeholder` | 200, ACAO spiegelt Origin | – | Platzhalter-Bilder per `fetch` ladbar |
| `https://pruefung.signatur.rtr.at/signature-verification/` | **403**, CSP `frame-ancestors https://www.rtr.at` | 302 | weder CORS noch Einbettung |

### 7.2 Konsequenzen für eine Web-App [I]

- **Top-Level-Formular-POST oder Formular-POST in ein iframe** zu A-Trust funktioniert ohne CORS (Navigation, keine Leseoperation). A-Trust dokumentiert die iframe-Einbindung ausdrücklich (§3.2).
- Die SL-Antwort kann die Web-App **nicht** aus dem A-Trust-Frame lesen (Same-Origin-Policy). Lösung: `DataURL` auf ein **eigenes Backend** setzen, das `XMLResponse` (die CMS-Signatur) entgegennimmt; die Web-App holt sie per Polling/WebSocket und bettet sie client-seitig ins PDF ein. Das Backend sieht nur die Signatur (+ Korrelations-ID), das PDF geht ohnehin direkt an A-Trust.
- FIDO2 im A-Trust-iframe: Cross-Origin-WebAuthn verlangt eine Permissions-Policy-Freigabe am iframe (`allow="publickey-credentials-get"`) und Browserunterstützung [I – nicht gemessen]; sicherer ist ein Top-Level-Redirect mit `RedirectURL`.
- Ein **reiner Proxy für A-Trust-Seiten** (eigener Server liefert A-Trust-HTML unter eigener Domain aus) widerspricht der Vorgabe, Zugangsdaten nur auf A-Trust-Seiten einzugeben, und wäre ein Man-in-the-Middle für Signaturpasswörter ⇒ **nicht zulässig** [I], gestützt auf [W] (docs.a-trust.at).
- Zulässig erscheint dagegen ein Backend, das **nur Protokolldaten** (DataURL-Antwort, CSC-Tokens) verarbeitet [I].

### 7.3 PDF-AS-Web als serverseitiges Signieren

- Öffentliche EGIZ/A-SIT-Instanz: Frontend `https://pdf.egiz.gv.at/pdf-as-wai/` (barrierearm), Signatur über „ID Austria / Mobile Phone Signature“ oder „Signature Card“; **Fair-Use**, Daten/Logs werden binnen 5 Tagen gelöscht, **keine Verfügbarkeitsgarantie** [M]/[W] (Seiteninhalt). Backend unter `https://apps.egiz.gv.at/pdf-as-web/` mit `Sign`-Servlet und SOAP `services/wssign?wsdl` (beide 200) [M].
- Schnittstelle (Doku „Anbindung externer Webanwendung an PDF-AS-WEB 4.0“, v1.1 vom 20.12.2023) [C] (`pdf-as-4/doc/AnbindungExterneWebanwendung.pdf`, S. 4-13):
  - Übergabe per User-Agent (Sign-Servlet, GET/POST), Upload per SOAP (`signSingle` → Redirect-URL), Serversignatur per SOAP (`jks`/`moa`), seit PDF-AS 4.3.0 `signMultiple`/`getMultiple` („Bulk-Light“, auch für `mobilebku`).
  - Sign-Servlet-Parameter: `connector` (`bku`, `onlinebku`, `mobilebku`, `moa`, `jks`), `invoke-app-url`, `invoke-app-error-url`, `invoke-app-url-target`, `locale`, `num-bytes`, `sig-type` (z. B. `SIGNATURBLOCK_SMALL_DE`, `AMTSSIGNATURBLOCK_DE`), `pdf-file`/`pdf-url`, `sig-pos-p/-w/-x/-y/-r/-f`, `verify-level` (`full`/`intOnly`), `filename`, `qrcontent`, `placeholder_web_enabled`, `placeholder_web_id`, Präfixe `pp:`, `ov:`, `sbp:`.
  - PDFData-Servlet nur **einmal** abrufbar; Header `Signer-Certificate`, `ValueCheckCode`, `CertificateCheckCode`; optional `origdigest` (SHA-256 hex) zur Integritätsprüfung; Rückgabe an `invoke-app-url` mit `pdfurl`, `pdflength`; Whitelisting der URLs.
  - Verify-Servlet `/Verify` mit `format=json`.
- Bewertung [I]: Technisch die einfachste Web-Lösung (CORS offen, fertige PAdES-Logik), aber (1) Fair-Use ohne SLA, (2) PDF geht an EGIZ **und** A-Trust, (3) Whitelisting der Invoke-URLs liegt beim Betreiber – also **nur als Demo/Fallback** oder mit Vereinbarung (Q10). Selbst gehostetes PDF-AS-Web wäre ein Java-Server (kollidiert nicht mit „Client nicht in Java“, aber mit dem Ziel „keine große Installation“ nur für Self-Hoster).

---

## 8. Platzhalter-Service und Signaturprüfung

### 8.1 QR-Platzhalter (`pdf.egiz.gv.at/pdf-as-wai/qrCodeGeneration`)

- Web-Formular: Kennung (Zahl, Default „0001“), Signaturprofil (`SIGNATURBLOCK_SMALL` + Sprache, `AMTSSIGNATURBLOCK_<LANG>_RECOMMENDED`, „PDF Over“ = **kein** Profil, „Manual input“), Sprache de/en; erzeugt Bild per GET auf `https://apps.egiz.gv.at/pdf-as-web/placeholder?id=…&profile=…&lang=…&b=1&h=60|100` [M] (Seite und `/pdf-as-wai/js/qr-code.js`). Die PDF-Over-Option lässt das Profil leer, damit die PDF-Over-eigenen Signaturblock-Einstellungen gelten [M] (Seitentext).
- Payload-Format [C]: `PDF-AS-POS` + `;id=<n>` (nur Ziffern, max. 5 Stellen, ohne führende Nullen) + `;profile=<PROFIL>` (`pdf-as-4/pdf-as-web/src/main/java/at/gv/egiz/pdfas/web/servlets/PlaceholderGeneratorServlet.java:69-90`; Konstante `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/placeholder/PlaceholderExtractorConstants.java:4`). Der Leser akzeptiert zusätzlich `type=` und `key=` (case-insensitive Schlüssel, Trennzeichen `;` und `=`) [C] (`pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/pdfbox2/placeholder/SignaturePlaceholderExtractor.java:461-487`, `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/impl/placeholder/SignaturePlaceholderData.java:59-65`).
- Bildgröße Default 300×60, Rahmen 1, Basisbild `PLATZHALTER-SIG.png` bzw. `PLACEHOLDER-SIG_EN.png`; Ausgabe JPEG oder PNG [C] (`PlaceholderGeneratorServlet.java:62`, `:107-109`, `:202-219`).
- Folgerung [I]: Der Klon **braucht den Service nicht** – Platzhalter lesen (QR-Decoder) und optional erzeugen ist lokal machbar; ein Link auf den EGIZ-Generator genügt für Nutzer:innen, die ihn gewohnt sind.

### 8.2 Signaturprüfung RTR (signaturpruefung.gv.at)

- `www.signaturpruefung.gv.at` leitet auf die RTR-Seite um, die die Web-App `https://pruefung.signatur.rtr.at/signature-verification/` per iframe einbettet (Version 2.3.5, Build 2026-08-07) [M].
- Formate laut RTR: CAdES, XAdES, PAdES, ASiC, CMS, XMLDSig, PDF („Adobe-Signatur“), PDF-AS (alt), BDC hotSign, trustDesk; Dateien bis 150 MB [W] ([RTR Hinweise](https://www.rtr.at/TKP/was_wir_tun/vertrauensdienste/Signatur/signaturpruefung/Pruefung_Details.de.html)). Ausländische Zertifikate: automatische Prüfung ohne Gewähr [M] (App-Text).
- **WebService-Schnittstelle** (EGIZ/A-SIT, „Webservice Schnittstelle für das Signaturprüfservice“, v1.7 vom 27.02.2025) [W] ([PDF](https://www.rtr.at/TKP/was_wir_tun/vertrauensdienste/Signatur/signaturpruefung/Dokumentation---WebService-Schnittstelle.pdf)):
  - SOAP 1.2 (und 1.1), MOA-SPSS-nahes Schema, Namespace `http://reference.e-government.gv.at/namespace/verificationservice/20120922#`, Pfad `…/signature-verification/services/VerificationService`.
  - Request `VerifyDocumentRequest`: `Version` (1/2/3), `Document` (Base64), `ExternalDocuments`, `FileID`, `TrustProfileID` (nicht unterstützt), `RequestPDFReport`, `Language` (de/en), `RequestSignerCertifcate` (sic), `RequestVerificationTime`.
  - Response `VerifyDocumentResponse` mit signiertem `VerificationReport` (`FileInfo`: FileID, FileType, File-Hash SHA-1; je Signatur `SignatureInfo`: SignerName, SignatureCheck, CertificateCheck, ManifestCheck, Annotations, SignerCertificate, VerificationTime, SignatureType, SignedFile, IssuerCountryCode, ByteRangeCovered, SignatureAlgorithm, signatureCoversFullPDF, TSLInformation), optional signierter PDF-Bericht, XMLDSig-Signatur über den Bericht.
  - Annotationscodes u. a. 8815 qualifiziertes Zertifikat, 8817/8818 Quelle TSL/Zertifikat, 8812/8813 Dokument nach Signatur ergänzt, **8850 visuelle Änderungen, 8851 zusätzliche Seiten, 8852 überdeckende Kommentare/Signaturblöcke** (neu 2025); Fehlercodes 1900-4999.
- Messung [M]: `…/services/VerificationService?wsdl` → **HTTP 401** (Authentifizierung nötig); Web-App-Preflight → 403; CSP `frame-ancestors https://www.rtr.at`.
- Folgerung [I]: Für den Klon (a) Button „Im Browser prüfen“ (öffnet `signaturpruefung.gv.at`), (b) optional Zugang zur SOAP-Schnittstelle bei RTR beantragen (Q11), (c) eigene Basis-Validierung lokal (Integrität, ByteRange, Zertifikatskette gegen EU-Trusted-List) als spätere Ausbaustufe. Die neuen Annotationscodes 8850-8852 sind ein Hinweis, dass Prüfdienste „Shadow-Attack“-artige Veränderungen erkennen – der Klon sollte beim Signieren keine Elemente erzeugen, die solche Warnungen auslösen (z. B. Signaturblöcke über Inhalt) [I].

### 8.3 Weitere Prüfdienste

- A-Trust-Signaturprüfdienst: Dokument spätestens nach 24 h gelöscht, IP/Zeit nach 7 Tagen [W] ([A-Trust Datenschutzerklärung v2.9, Jän. 2026, Abschnitt 5](https://www.a-trust.at/datenschutz)).
- **EU DSS**: Java-Bibliothek (LGPL-2.1; ein Modul EUPL) [S]; Demo-WebApp 6.5 mit REST/SOAP (OpenAPI seit 5.6) inkl. `validateSignature`, aber ausdrücklich **nur Demonstration/Testzwecke**, Dateien gehen an die Infrastruktur der Kommission [W] ([DSS Demo REST](https://ec.europa.eu/digital-building-blocks/DSS/webapp-demo/signature-rest)). Neueste Version 6.5.RC1 (Juli 2026) [S].
- Folgerung [I]: DSS ist als **selbst gehosteter Validierungsdienst** (Java-Server) oder als Referenz für Testfälle nutzbar, nicht als Produktivdienst über die Kommissions-Demo.

---

## 9. Rechtlicher und organisatorischer Rahmen

### 9.1 Was ist für eine QES in Österreich nötig?

- QES = fortgeschrittene Signatur, erstellt mit einer **qualifizierten Signaturerstellungseinheit (QSCD)** und beruhend auf einem **qualifizierten Zertifikat** (eIDAS Art. 3(12)); Rechtswirkung der handschriftlichen Unterschrift gleichgestellt (Art. 25(2)); EU-weite Anerkennung [S] (zitiert im [CSC-Whitepaper](https://cloudsignatureconsortium.org/wp-content/uploads/2024/10/CSC-White-Paper-The-EU-digital-identity-wallet-a-new-tool-for-remote-signing-with-qualified-electronic-signatures.pdf)).
- Österreich: Signatur- und Vertrauensdienstegesetz (**SVG**, BGBl. I Nr. 50/2016, in Kraft seit 01.07.2016, zuletzt idF BGBl. I Nr. 27/2019) [S] ([RIS § 3](https://www.ris.bka.gv.at/eli/bgbl/i/2016/50/P3/NOR40184732)).
  - § 4: QES erfüllt die Schriftlichkeit nach § 886 ABGB; Ausnahmen: letztwillige Verfügungen (nicht elektronisch), familien-/erbrechtliche Erklärungen und Bürgschaften von Verbraucher:innen nur mit Bestätigung von Notar/Rechtsanwalt; Verbraucherschutzklausel [S] ([gesetzefinden.at § 4](https://gesetzefinden.at/bundesrecht/bundesgesetze/svg/para-4)).
  - Inhaltsverzeichnis: § 4 Rechtswirkungen, § 5 Pflichten der Signator:innen, § 6 Aussetzung, § 7 Bestätigungsstelle (prüft **QSCDs** nach Anhang II eIDAS), §§ 8-11 Vertrauensdiensteanbieter (u. a. § 10 Dokumentation), §§ 12-15 Aufsicht [S].
- **Client-Software**: Weder eIDAS (Zertifizierung nach Art. 30 nur für QSCDs) noch das SVG (laut Inhaltsverzeichnis keine Bestimmungen zu Signaturerstellungsanwendungen) sehen eine Zertifizierungspflicht für die Signaturanwendung vor [I]. Bei Fernsignatur ist die QSCD das HSM des VDA; die Freigabe (Signaturpasswort + zweiter Faktor) erfolgt bei A-Trust [I]/[W].
- **Freiwillige Normen** [I – nicht in dieser Recherche abgerufen]: ETSI EN 319 142 (PAdES), ETSI EN 319 102-1 (Erstellung/Validierung), ETSI TS 119 101 (Anforderungen an Signaturerstellungs-/Validierungsanwendungen), ETSI TS 119 432 (Remote-Signatur-Protokolle). Sinnvoll als Qualitätsmaßstab, nicht verpflichtend.
- **A-Trust „Liste der empfohlenen Komponenten und Verfahren“ 1.7.28** [W] ([a-trust.at/docs/verfahren](https://www.a-trust.at/docs/verfahren)): Empfehlungen (keine Zulassung) zu Karten, Kartenlesern mit PIN-Pad, Browsern (Edge, Firefox, Opera, Safari, Chrome), Signaturapplikationen und „vertrauenswürdiger Anzeige“. Für die Fernsignatur gilt: Die Anzeige der Signaturdaten erfolgt im Browser; **bei PDF wird ein Link auf das PDF angeboten** (§5.2.3). Das entspricht dem `ShowSigobj.aspx`-Link, den PDF-Over ausliest (`ATrustParser.java:326`) [C].
- Darstellungsanforderungen für den Klon [I]:
  - SL-Pfad (P1/P2): Die „vertrauenswürdige Anzeige“ liefert A-Trust; der Klon sollte den Link „Signaturdaten anzeigen“ sichtbar lassen und eine eigene Vorschau anbieten.
  - CSC-Pfad (P3): A-Trust sieht nur Hashes ⇒ der Klon ist faktisch die Anzeigekomponente (What-you-see-is-what-you-sign). Empfehlung: unveränderliche Vorschau genau der signierten Bytes, Anzeige von Seitenanzahl/Hash, kein nachträgliches Verändern.
  - Sichtbarer Signaturblock: für **Amtssignatur** gesetzlich verlangt (Bildmarke + Hinweis, Prüfinformationen; E-GovG § 19 Abs. 3) [S] ([RTR Amtssignatur](https://www2.rtr.at/tr/rtr/amtssignatur/Amtssignaturen_sig.pdf), [wien.gv.at](https://www.wien.gv.at/spezial/foerderhandbuch/e-government/amtssignatur/)); Amtssignatur dürfen nur Auftraggeber des öffentlichen Bereichs verwenden. Für private QES ist ein sichtbarer Block **nicht vorgeschrieben**, aber üblich [I].
- Pflichten der Signator:innen (SVG § 5, A-Trust-Belehrung): Signaturpasswort geheim halten, Komponenten trennen, Browser-Passwortspeicher deaktivieren [S]/[W] ⇒ Produktanforderung „kein persistentes Speichern des Signaturpassworts“ [I].

### 9.2 Datenschutz (DSGVO)

| Fluss | Was wird übertragen | Speicherdauer laut Quelle | Beleg |
|---|---|---|---|
| SL 1.2 Mobile (P1/P2) | **ganzes PDF** (Platzhalter ausgeschwärzt) an A-Trust | „Signaturdienst“: zu signierendes/signiertes Dokument spätestens nach **24 h** gelöscht; Dokumentation nach § 10 SVG **30 Jahre** ab Ende der Zertifikatsgültigkeit | [C] §2.2; [W] [Datenschutzerklärung v2.9 Abschn. 8](https://www.a-trust.at/datenschutz) (ob die SL-Schnittstelle unter „Signaturdienst“ fällt: [I]) |
| CSC Hash-Signing (P3) | nur Hash(es) + Zertifikatsabfrage | nicht gesondert angegeben | [W] (Presse 01.12.2023) |
| A-Trust PDF Sign | ganzes PDF | spätestens **1 h** nach letztem Zugriff | [W] [pdfsign](https://www.a-trust.at/pdfsign/) |
| PDF-AS-WAI (EGIZ) | ganzes PDF an EGIZ (+ A-Trust) | temporäre Daten/Logs binnen **5 Tagen** | [M]/[W] (Seitentext) |
| A-Trust-Prüfdienst | signierte Datei | **24 h**, IP/Zeit 7 Tage | [W] Datenschutzerklärung Abschn. 5 |
| RTR-Prüfdienst | signierte Datei | in der Hinweisseite keine Angabe; nur verschlüsselte Übertragung | [W] (RTR Hinweise) |
| EU-DSS-Demo | Datei an EU-Kommission | „not retained“ | [W] (DSS Demo) |

Rollen [I]: Solange der Klon rein lokal arbeitet und direkt mit A-Trust spricht, ist der Klon-Herausgeber kein Auftragsverarbeiter (wie PDF-Over). Sobald ein eigenes Backend (DataURL-Empfänger, CSC-Token-Proxy) eingesetzt wird, verarbeitet der Herausgeber personenbezogene Daten (mindestens Signaturzertifikat mit Namen, IP) ⇒ Datenschutzerklärung, ggf. Verarbeitungsverzeichnis, Löschkonzept, ggf. Rollenvertrag mit A-Trust. Die Datenschutzerklärung des Klons muss – wie PDF-Over – offenlegen, dass das PDF im SL-Pfad an A-Trust geht.

### 9.3 Lizenz- und Marken-Hinweise (kurz)

- PDF-Over steht unter EUPL; im About-Text weist A-SIT darauf hin, dass manche Komponenten eigene Lizenzen haben, teils nur für EUPL-Software frei nutzbar [C] (`messages_de.properties:111`). Für die Wiederverwendung von Ressourcen (Fonts, Logos, Profile aus `cfgzip`) muss das gesondert geprüft werden [I] (anderes Dokument).
- Name/Logo „ID Austria“ und A-Trust-Marken: Nutzungsregeln wurden nicht recherchiert (Q12). Der Klon darf nicht den Eindruck einer offiziellen A-Trust/A-SIT-Anwendung erwecken [I].

---

## 10. EU-Ausblick und Konsequenzen für die Architektur

### 10.1 Rahmen

- eIDAS-Novelle **VO (EU) 2024/1183**, in Kraft seit 20.05.2024 [S] (CSC-Whitepaper; EUR-Lex war am Recherchetag für automatisierte Abrufe gesperrt, daher Zitate aus Sekundärquellen).
  - Art. 5a(4)(e) / 5a(5)(g): Wallet ermöglicht natürlichen Personen QES „by default and free of charge“; ErwG 20: kostenlos für nicht-berufliche Zwecke, Mitgliedstaaten dürfen verhältnismäßige Maßnahmen gegen berufliche Gratisnutzung treffen; „professional“ ist nicht definiert [S] (CSC-Whitepaper S. 7-9).
  - Art. 5a(5)(a)(xi): gemeinsame Protokolle/Schnittstellen zur QES-Erstellung; laut CSC-Whitepaper verweist der Entwurf des Durchführungsrechtsakts zu Wallet-Kernfunktionen (Art. 12(3)) für in Wallets integrierte Signaturerstellungsanwendungen auf die **CSC-API** [S].
  - Wallet-Bereitstellung bis **Ende 2026** (öffentlicher Sektor muss akzeptieren; Privatsektor-Pflichten ab Ende 2027) [S]/[W] ([A-Trust News 03.09.2025](https://a-trust.at/en/about_us/news/20250903_en_post.html), CSC-Whitepaper).
- Österreich [S] ([Parlament AB 5451](https://www.parlament.gv.at/dokument/XXVIII/AB/5451/imfname_1769922.pdf), Suchzusammenfassung):
  - Pflicht, im Lauf von 2026 eine EUDI-Wallet bereitzustellen; schrittweise Einführung mit Pilotprojekten; Interoperabilität bis Ende 2026; BKA bestätigte (ID Austria Zukunftsforum, Okt. 2025) die **Weiterentwicklung der bestehenden ID-Austria-Infrastruktur zur Wallet**; Budget ca. 8 Mio. €; ID Austria wird nicht ersetzt; Nutzung freiwillig. Ein Presseartikel (April 2026) nennt „Anfang 2027“ als Start [S].
  - **Valera**: Open-Source-Wallet von A-SIT (Kotlin Multiplatform), Technologie-Demonstrator/Sandbox, in Large-Scale-Pilots getestet; Repo `a-sit-plus/valera` existiert [S]/[M] ([wallet.a-sit.at](https://wallet.a-sit.at/), [Trinsic](https://docs.trinsic.id/docs/austria-valera-eudi-sandbox)).
- A-Trust [W]:
  - Pilot **POTENTIAL** abgeschlossen; zwei Modelle getestet: **wallet-getriebene QES** (Dokument über RP-EUDIW-Schnittstelle, Wallet zeigt Details, gibt beim QTSP frei) und **QTSP-getriebene QES** (Dokument oder Hash über RP-QTSP-Schnittstelle, Wallet autorisiert); Test-/Referenzumgebung für Partner über Vertrieb ([News 03.09.2025](https://a-trust.at/en/about_us/news/20250903_en_post.html)).
  - Launch-Partner der deutschen Wallet „d-you“ (Start 02.01.2027) [S]; Use-Cases für die deutsche Wallet eingereicht ([News 26.07.2026](https://a-trust.at/en/about_us/news/20260726_en_post.html)).
  - SL-iframe-Parameter `showwallet`/`usewallet` für den EUDI-Wallet-Signaturfluss existieren bereits [W] (§3.2).

### 10.2 Architekturkonsequenz: Backend-Abstraktion [I]

Empfohlene Trennung, abgeleitet aus den Unterschieden P1-P9:

```
SigningPipeline
  1. prepare(pdf, position|placeholder, profile)      -> PreparedDocument (ByteRange, Platzhalter)
  2. backend.capabilities()                            -> {inputKind, outputKind, interaction, needsServer, batch, factors}
  3. backend.sign(PreparedDocument[] | DigestRequest[]) -> CMS SignedData | rohe Signaturwerte + Zertifikat
  4. assemble(...)                                      -> CMS bauen (falls nötig), in PDF einbetten, optional LTV/Timestamp
```

| Capability | Werte | Beispiel |
|---|---|---|
| `inputKind` | `FULL_DOCUMENT` / `DIGEST` | SL-Mobile: FULL_DOCUMENT (A-Trust zeigt PDF an); CSC: DIGEST |
| `outputKind` | `CMS_SIGNED_DATA` / `RAW_SIGNATURE` | SL: CMS; CSC/PKCS#11: RAW ⇒ Client baut SignedAttributes/SignedData |
| `interaction` | `EMBEDDED_WEB` / `SYSTEM_BROWSER` / `NATIVE` / `NONE` | SL-P1: EMBEDDED_WEB; CSC: SYSTEM_BROWSER; Keystore: NONE |
| `needsServer` | `NONE` / `DATAURL_RECEIVER` / `TOKEN_PROXY` | Web-App + SL: DATAURL_RECEIVER; CSC: TOKEN_PROXY |
| `batch` | bool + Max | CSC `numSignatures`; SL: false |
| `certificateFirst` | bool | CSC/PKCS#11 brauchen das Zertifikat vor dem Hash (ESS-Attribut) |

Konkrete Backends: `AtrustSecurityLayerMobile` (P1, Pflicht v1), `AtrustCscHashSigning` (P3, sobald Vertrag), `GenericCscRssp` (v1/v2, andere QTSPs/EUDI-QTSP-Modell), `LocalBkuSecurityLayer` (P7, Desktop optional), `Pkcs11Token` / `Pkcs12Keystore` (P8), später `EudiWalletSigning`. Wichtig: PAdES-Vorbereitung und Signaturblock dürfen **nicht** an ein Backend gekoppelt sein, weil SL einen fertigen CMS-Container liefert, CSC aber nur Rohsignaturen.

---

## 11. Plattformspezifische Einschränkungen (Zusammenfassung)

| Plattform | Einschränkung | Grad |
|---|---|---|
| Alle | Natives Befüllen/Parsen der A-Trust-Seiten nur mit A-Trust-Zustimmung | [W]/[I] |
| Alle | Produktiv-SL-Endpoint ohne CORS; Testsystem mit CORS – nicht verwechseln | [M] |
| Desktop Windows | WebView2/Systembrowser mit WebAuthn; FIDO2 über Windows-WebAuthn-API auch nativ möglich (nur relevant für P2) | [I] (siehe Dok. 01 §5.1) |
| Desktop macOS | WKWebView-WebAuthn nur für Associated Domains ⇒ für FIDO2 Systembrowser; lokale BKU nur über (veralteten) MOCCA | [S]/[I] |
| Desktop Linux | WebKitGTK-WebAuthn unsicher ⇒ Systembrowser; libfido2 nur für P2; lokale BKU nur MOCCA | [I] |
| iOS | Kein App2App für SL-Signaturen; FIDO2 nur in ASWebAuthenticationSession/SFSafariViewController; App „ID Austria“ ab iOS 16 | [M]/[S]/[W] |
| Android | WebView-WebAuthn nur für eigene, per Asset Links verknüpfte Domain; Custom Tabs für FIDO2; `postUrl` nur form-urlencoded; App „ID Austria“ ab Android 12, Root-Geräte ausgeschlossen | [W]/[I] |
| Web | Kein Lesen der A-Trust-Antwort; DataURL-Backend nötig; kein Proxy für A-Trust-Seiten; RTR-Prüfung nicht einbettbar | [M]/[I] |
| Lokale BKU | Desktop-only; MOCCA eingestellt; a.sign Client primär Windows | [W]/[S] |

---

## 12. Offene Fragen (an A-Trust, A-SIT/EGIZ, BKA, RTR)

- **Q1** (A-Trust/A-SIT): Auf welcher Grundlage darf PDF-Over die A-Trust-Seiten parsen und nativ befüllen? Kann ein unabhängiger Open-Source-Klon dieselbe Freigabe erhalten (ggf. mit Identifikation per User-Agent)?
- **Q2** (A-Trust): Ist die Nutzung des SL-1.2-Endpoints durch beliebige Dritt-Clients (Browser-Control, ohne Registrierung) ausdrücklich erlaubt? Gibt es Rate-Limits/Blocklisten?
- **Q3** (A-Trust/A-SIT): Wird SL 2.0 für Dritt-Signaturanwendungen angeboten (Spezifikation, Schlüsselregistrierung, `nativeApp`/`IPCRedirect`)?
- **Q4** (A-Trust): Unterstützt die CSC-API Public Clients (PKCE ohne client_secret) und Redirects auf Universal/App Links bzw. Loopback?
- **Q5** (A-Trust): Wie viele Benutzerinteraktionen braucht der CSC-Fluss (scope `service` und `credential`) mit ID Austria; was zeigt die A-Trust-Seite bei Hash-Signing an?
- **Q6** (A-Trust): Darf die kostenlose ID-Austria-Signatur über Dritt-Software gewerblich genutzt werden, oder ist dafür das „Paket gewerbliche Nutzung“ nötig? Konditionen der CSC-Partnerschaft für ein Open-Source-Projekt?
- **Q7** (A-Trust): Ist der QR-Code-Fluss (PDF-Over `#qrimage`) mit der App „ID Austria“ 2026 noch aktiv?
- **Q8** (A-Trust): Wie wird die Signaturkarte als ID-Austria-Zweitfaktor im Browser angesprochen (Erweiterung, WebUSB, a.sign Client)? PKCS#11-Treiber für macOS/Linux?
- **Q9** (BKA/A-Trust): Gibt es für Signaturen auf demselben Smartphone einen App2App-Rücksprung (analog OIDC)?
- **Q10** (EGIZ/A-SIT): Darf ein Klon `pdf.egiz.gv.at`/`apps.egiz.gv.at/pdf-as-web` (Sign/SOAP/placeholder) programmatisch nutzen; gibt es dafür Whitelisting/SLA?
- **Q11** (RTR): Bedingungen für den Zugang zur SOAP-Prüfschnittstelle (401)?
- **Q12** (BKA/A-Trust): Regeln zur Verwendung von Namen/Logo „ID Austria“ in Dritt-Apps.
- **Q13** (BKA): Zeitplan und Schnittstellen der österreichischen EUDI-Wallet für QES (wallet- vs. QTSP-getrieben, CSC-Version, OpenID4VP).
- **Q14** (A-Trust): Gilt die Löschfrist „spätestens 24 h“ (Datenschutzerklärung, Signaturdienst) auch für Uploads über die SL-Schnittstelle?

---

## Anhang A – Messprotokoll (2026-10-14)

Alle Requests ohne Cookies/Zugangsdaten, mit `curl` (Origin-Header `https://example.org` für CORS-Prüfung). Keine Formulare abgeschickt.

| # | Request | Ergebnis (gekürzt) |
|---|---|---|
| A1 | `OPTIONS https://service.a-trust.at/mobile/https-security-layer-request/default.aspx` (+ `Access-Control-Request-Method: POST`) | 200; `allow: OPTIONS, TRACE, GET, HEAD, POST`; `server: Microsoft-IIS/10.0`; kein ACAO |
| A2 | `GET` dito | 302 `location: /mobile/https-security-layer-request/InvalidRequest.aspx`; kein ACAO |
| A3 | `OPTIONS`/`GET https://hs-abnahme.a-trust.at/mobile/https-security-layer-request/default.aspx` | 200 bzw. 302; `access-control-allow-origin: *` |
| A4 | `OPTIONS`/`GET https://www.a-trust.at/mobile/https-security-layer-request/default.aspx` | 200 bzw. 302; kein ACAO |
| A5 | `GET https://service.a-trust.at/mobile/https-security-layer-request/InvalidRequest.aspx` | 200; kein `X-Frame-Options`, kein CSP `frame-ancestors` |
| A6 | `OPTIONS`/`GET https://testbox.a-trust.at/hashsigning/2/csc/v1/info` | 405 (`allow: GET, POST`) bzw. 200 `text/html`; kein ACAO |
| A7 | `GET https://{service,www}.a-trust.at/.well-known/{apple-app-site-association,assetlinks.json}`, `https://idp.id-austria.gv.at/.well-known/…` | siehe §6.1 (service.a-trust.at: 404) |
| A8 | `GET https://pdf.egiz.gv.at/` → `/pdf-as-wai/`; `GET /pdf-as-wai/qrCodeGeneration`; `GET /pdf-as-wai/js/qr-code.js` | 200; Platzhalter-Endpoint `https://apps.egiz.gv.at/pdf-as-web/placeholder` |
| A9 | `OPTIONS https://apps.egiz.gv.at/pdf-as-web/{Sign,placeholder}` | 200; ACAO = gespiegelte Origin; `Access-Control-Allow-Credentials: true` |
| A10 | `GET https://apps.egiz.gv.at/pdf-as-web/services/wssign?wsdl` | 200 `text/xml` |
| A11 | `GET https://www.signaturpruefung.gv.at/` | 301 → `https://www.rtr.at/TKP/…/signaturpruefung/Pruefung.de.html` (iframe auf `pruefung.signatur.rtr.at`) |
| A12 | `OPTIONS https://pruefung.signatur.rtr.at/signature-verification/` | 403; CSP `frame-ancestors https://www.rtr.at` |
| A13 | `GET https://pruefung.signatur.rtr.at/signature-verification/services/VerificationService?wsdl` | 401 Unauthorized |
| A14 | `GET https://www.oesterreich.gv.at/id-austria/haeufige-fragen/allgemeines-zu-id-austria` | 301 → `https://www.id-austria.gv.at/hilfe/hilfe-zu-ida/generelle-info` |
| A15 | `GET https://api.github.com/users/A-Trust/repos` | 14 öffentliche Repos, u. a. `CSC_HashSignClient` (Push 2025-11-17), `Handy-Signatur` (2025-09-24), `HandySignaturClientDemo` (2021-05-25), `Signaturbox` (2026-09-11) |

## Anhang B – Quellen (Abruf 2026-10-14)

A-Trust
- https://docs.a-trust.at/ · https://docs.a-trust.at/apis/SecurityLayer/ · https://docs.a-trust.at/apis/SecurityLayer/application · https://docs.a-trust.at/apis/SecurityLayer/transport
- https://docs.a-trust.at/apis/CSC · https://docs.a-trust.at/apis/SignaturServer · https://docs.a-trust.at/apis/Place2Sign · https://docs.a-trust.at/apis/TokenAPI · https://docs.a-trust.at/apis/Identschnittstelle · https://docs.a-trust.at/integration-guides/ · https://docs.a-trust.at/swagger/signaturbox.yaml
- https://github.com/A-Trust/Handy-Signatur · https://github.com/A-Trust/HandySignaturClientDemo · https://github.com/A-Trust/CSC_HashSignClient · https://github.com/A-Trust/ShortTermCert
- https://a-trust.at/en/about_us/news/20231201_en_post.html (Hash-Signing) · https://a-trust.at/en/about_us/news/20230727_en_post%20copy.html (App-Umbenennung, QR/Push-TAN entfallen) · https://a-trust.at/en/about_us/news/20250903_en_post.html (POTENTIAL) · https://a-trust.at/en/about_us/news/20260726_en_post.html (deutsche Wallet)
- https://www.a-trust.at/downloads/documents/Pressemeldungen/PM_A-Trust_Sec_f._alle_Groessen.pdf (Presse 10.11.2025: Hash2Sign, gewerbliche Nutzung)
- https://a-trust.at/en/partners · https://a-trust.at/en/companies/ · https://a-trust.at/en/products/signature_solutions_for_companies/hash2sign_software/ · https://a-trust.at/en/products/qualified_e-signature_services/commercial_use/ · https://a-trust.at/en/products/id_and_signature_cards/signature_card/ · https://www.a-trust.at/pdfsign/ · https://www.a-trust.at/meine-id-austria/fido-token/
- https://www.a-trust.at/docs/verfahren (Liste der empfohlenen Komponenten und Verfahren 1.7.28, 24.10.2024)
- https://www.a-trust.at/datenschutz (Datenschutzerklärung v2.9, Jän. 2026)

ID Austria / Bund
- https://www.id-austria.gv.at/hilfe/hilfe-zu-ida/generelle-info · https://www.id-austria.gv.at/de/hilfe/hilfe-zu-ida/authentifizierungsfaktoren · https://www.id-austria.gv.at/de/verwenden/app-id-austria
- https://www.id-austria.gv.at/de/developer · https://www.id-austria.gv.at/de/developer/technische-konzepte/prozesse · https://www.id-austria.gv.at/de/developer/anbinden/openid-connect-f%C3%BCr-mobile-apps · https://www.id-austria.gv.at/de/developer/testen/testen-auf-mobilen-ger%C3%A4ten
- https://www.usp.gv.at/hilfe-und-support/anmeldung/id-austria-app-neu.html · https://orf.at/stories/3397644/ · https://www.bundeskanzleramt.gv.at/agenda/digitalisierung/digitales-oesterreich/id-austria.html
- https://www.parlament.gv.at/dokument/XXVIII/AB/5451/imfname_1769922.pdf (EUDI-Wallet AT, via Suchzusammenfassung)
- https://www.ris.bka.gv.at/eli/bgbl/i/2016/50/P3/NOR40184732 · https://gesetzefinden.at/bundesrecht/bundesgesetze/svg/para-4
- https://www2.rtr.at/tr/rtr/amtssignatur/Amtssignaturen_sig.pdf · https://www.wien.gv.at/spezial/foerderhandbuch/e-government/amtssignatur/

EGIZ / A-SIT
- https://sl.eid.egiz.gv.at/konzept/securitylayer/spezifikation/20140114-en/bindings/Bindings.en.html (SL 1.2 Bindings)
- https://pdf.egiz.gv.at/pdf-as-wai/ · https://pdf.egiz.gv.at/pdf-as-wai/qrCodeGeneration · https://pdf.egiz.gv.at/pdf-as-wai/js/qr-code.js · https://apps.egiz.gv.at/pdf-as-web/
- https://technology.a-sit.at/?p=3914 (MOCCA) · https://wallet.a-sit.at/ · https://github.com/a-sit-plus/valera · https://github.com/a-sit-plus/eid-service-app

Prüfung
- https://www.rtr.at/TKP/was_wir_tun/vertrauensdienste/Signatur/signaturpruefung/Pruefung.de.html · https://www.rtr.at/TKP/was_wir_tun/vertrauensdienste/Signatur/signaturpruefung/Pruefung_Details.de.html · https://www.rtr.at/TKP/was_wir_tun/vertrauensdienste/Signatur/signaturpruefung/Dokumentation---WebService-Schnittstelle.pdf · https://pruefung.signatur.rtr.at/signature-verification/?locale=de
- https://ec.europa.eu/digital-building-blocks/DSS/webapp-demo/signature-rest

EU / Plattformen
- https://cloudsignatureconsortium.org/wp-content/uploads/2024/10/CSC-White-Paper-The-EU-digital-identity-wallet-a-new-tool-for-remote-signing-with-qualified-electronic-signatures.pdf
- https://developer.apple.com/forums/thread/723273 · https://passkeys.dev/docs/reference/ios · https://developer.android.com/identity/sign-in/credential-manager-webview

Sekundär (Kontext)
- https://eideasy.com/handy-signatur-and-id-austria-qualified-electronic-signatures/ (2021, Integrationserfahrungen SL) · https://www.trendingtopics.eu/a-trust-wird-launch-partner-der-deutschen-eudi-wallet-d-you/ · https://docs.trinsic.id/docs/austria-valera-eudi-sandbox
