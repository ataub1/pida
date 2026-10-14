# 05 – Konfiguration, Plattformintegration, Build/Distribution und Betrieb von PDF-Over

Stand der Analyse: 2026-10-14
Referenz: PDF-Over `main`, Commit `c07c5ec` (2026-09-03), Version `4.4.10-SNAPSHOT`; PDF-AS 4.4.2; mitgelieferte PDF-AS-Konfiguration (`PDFASConfig.zip`).

## 0. Lesehinweise

### 0.1 Pfadkonventionen für Fundstellen

| Präfix | Bedeutung |
|---|---|
| *(kein Präfix)* | relativ zum PDF-Over-Repository-Root, z. B. `pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java:78` |
| `pdf-as-4/` | relativ zum PDF-AS-4-Repository |
| `cfgzip/` | relativ zur entpackten `pdf-over-gui/src/main/resources/cfg/PDFASConfig.zip` |

Abkürzungen in Fundstellen (nur in Tabellen, um Breite zu sparen):

| Kürzel | Datei |
|---|---|
| `C` | `pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java` |
| `CM` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/config/ConfigurationManager.java` |
| `CD` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/config/ConfigurationDataInMemory.java` |
| `PCS` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/PrepareConfigurationState.java` |
| `SCC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/SimpleConfigurationComposite.java` |
| `ACC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/AdvancedConfigurationComposite.java` |
| `KCC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/KeystoreConfigurationComposite.java` |
| `FCC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/Fido2ConfigurationComposite.java` |
| `AC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/configuration/AboutComposite.java` |
| `CC` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/ConfigurationComposite.java` |
| `cli/` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/cliarguments/` |
| `gui/` | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/` |
| `res/` | `pdf-over-gui/src/main/resources/` |

### 0.2 Verlässlichkeitskennzeichnung

- **[V]** im Code/in Dateien verifiziert (mit Fundstelle).
- **[I]** Interpretation/Vermutung (aus Code abgeleitet, aber nicht ausgeführt oder nicht eindeutig).
- **[E]** extern recherchiert oder gemessen (Web; Datum 2026-10-14), nicht im Repository verifizierbar.

Es wurde nichts gebaut oder ausgeführt. In der Doku stehen keine Passwörter, Zugangsdaten, Zertifikats-Fingerprints, Team-IDs oder Personendaten aus dem Repository.

---

## 1. Konfigurationsdateien

### 1.1 Speicherort je Betriebssystem

PDF-Over nutzt auf **allen** Desktop-Plattformen ein verstecktes Verzeichnis im Home-Verzeichnis, also **nicht** die plattformüblichen Orte (`%APPDATA%`, `~/Library/Application Support`, `$XDG_CONFIG_HOME`).

| Zweck | Pfad (Java) | Windows | macOS | Linux | Fundstelle |
|---|---|---|---|---|---|
| Konfigurationsverzeichnis | `${user.home}/.pdf-over` | `C:\Users\<user>\.pdf-over` | `/Users/<user>/.pdf-over` | `/home/<user>/.pdf-over` | [V] `C:78` |
| PDF-AS-Arbeitsverzeichnis (identisch) | `${user.home}/.pdf-over` | wie oben | wie oben | wie oben | [V] `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4Helper.java:64-66` |
| Backup beim Factory-Reset | `${user.home}/.pdf-over.old` | analog | analog | analog | [V] `C:81`, `CM:69-105` |
| Optionaler Logging-Override | `${user.home}/.pdf-over-logging.xml` | analog | analog | analog | [V] `res/logback.xml:29` |

`user.home` ist unter Windows bei Java 21 `%USERPROFILE%` [I]. Das Verzeichnis wird in `Main.setup()` angelegt, wenn es fehlt (`gui/Main.java:40-44`) [V].

### 1.2 Verzeichnislayout von `~/.pdf-over` (nach Erststart und Nutzung)

| Pfad | Inhalt / Erzeuger | Fundstelle |
|---|---|---|
| `PDF-Over.config` | Hauptkonfiguration (Java-Properties), siehe 1.3–1.5 | [V] `C:84`, `PCS:108-117`, `CM:280-383` |
| `.version` | Klartext: PDF-Over-Version, die die PDF-AS-Konfiguration zuletzt entpackt hat (erste nicht leere Zeile ohne `#`) | [V] `C:87`, `PCS:133-147`, `PCS:175-203` |
| `cfg/` | PDF-AS-Konfiguration: `config.properties`, `advancedconfig.properties`, `log4j.properties`, `logback.xml`, `pdf-as-web.properties`, `profiles/*.properties` (20 Profile) | [V] `cfgzip/cfg/**` |
| `fonts/` | `DejaVuSansCondensed.ttf`, `DejaVuSansCondensed-Bold.ttf`, `DejaVuSansMono.ttf`, `arial.ttf` | [V] `cfgzip/fonts/` |
| `images/` | `signatur-logo_de.png`, `signatur-logo_en.png`, `amtsignatur-logo_demo.png` | [V] `cfgzip/images/` |
| `licenses/` | Lizenztexte von Drittkomponenten (Apache, PDFBox, DejaVu/Bitstream, log4j, …) | [V] `cfgzip/licenses/` |
| `cfg_backup.zip`, `cfg_backup1.zip`, … | Sicherung des alten `cfg/` beim PDF-AS-Config-Update | [V] `PCS:210-240`, `C:94` |
| `logs/pdf-over.log`, `logs/pdf-over.log.1` … `.10` | Logdateien (Rolling) | [V] `res/logback.xml:10-23` |
| `tmp/` | Temp-Verzeichnis des Ausgabe-Schritts | [V] `gui/workflow/states/OutputState.java:57-65` |
| `.emblem.png`, `.emblem.properties` | Cache der Bildmarke (auf max. 480×600 px verkleinert; Properties `IMG` = Originalpfad, `HSH` = MD5 der Originaldatei) | [V] `pdf-over-signer/src/main/java/at/asit/pdfover/signer/Emblem.java:46-54,84-87,123-166` |

Die PDF-AS-Bibliothek liest `cfg/config.properties` relativ zum Arbeitsverzeichnis (`IProfileConstants.CFG_DIR = "cfg"`, `CFG_FILE = "config.properties"`) [V] `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/settings/IProfileConstants.java:85-86`, `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/settings/Settings.java:64-78`.

### 1.3 Dateiformat von `PDF-Over.config`

- Format: `java.util.Properties`. Gelesen wird mit `Properties.load(InputStream)`, geschrieben mit `Properties.store(OutputStream, "Configuration file was generated!")` [V] `CM:112-114`, `CM:378-380`.
- Daraus folgt für einen Klon [I] (Java-Standardverhalten, nicht im Repo getestet):
  - **Zeichenkodierung ISO-8859-1.** Zeichen außerhalb von 0x20–0x7E werden beim Schreiben als `\uXXXX` escaped. Das betrifft z. B. Umlaute im Signaturhinweis `SIGNATURE_NOTE`, dessen deutscher Standardtext Umlaute enthält.
  - Escapes in Schlüsseln/Werten: `\\`, `\=`, `\:`, `\#`, `\!`, `\t`, `\n`, `\r`, `\f`, führende Leerzeichen als `\ `. Windows-Pfade stehen deshalb z. B. als `C\:\\Users\\...` in der Datei.
  - Kopfzeilen: `#Configuration file was generated!` und eine Datumszeile (`#<Date.toString()>`).
  - Die Reihenfolge der Schlüssel ist nicht definiert (Hash-basiert).
  - Beim Lesen gelten Kommentare mit `#`/`!`, die Trenner `=`, `:` oder Whitespace und Fortsetzungszeilen mit `\` am Zeilenende.
- Die Datei wird beim Speichern **komplett neu geschrieben** (`props.clear()`, nur bekannte Keys) [V] `CM:284-285`. Unbekannte oder veraltete Keys gehen beim nächsten Speichern verloren. Eine Ausnahme ist `CRASH`, das nie geschrieben wird.
- Alternativer Dateiname per CLI `-c <name>`. Der Name wird **relativ zu `~/.pdf-over`** aufgelöst (`CONFIG_DIRECTORY + separator + name`), sowohl beim Laden als auch beim Speichern [V] `cli/ConfigFileArgument.java:38-57`, `CM:114`, `CM:281-282`. Fehlt die Datei, löst `loadFromDisk()` eine `IOException` aus. Diese wird zur `InitializationException`, und der Nutzer bekommt den Factory-Reset-Dialog angeboten [V] `PCS:92-98`, `PCS:313-327`. Das ist fragwürdiges Verhalten. `ensurePdfOverConfigExists()` legt nur die Standarddatei an [V] `PCS:108-117`.
- `THEME` wird **immer** aus der Standarddatei `PDF-Over.config` gelesen, auch wenn `-c` gesetzt ist, und zwar im statischen Initialisierer [V] `C:237-251`.

### 1.4 Lade-/Speicher-Logik: Persistent vs. Overlay

- `ConfigurationManager` hält zwei Instanzen von `ConfigurationDataInMemory`: `configuration` (persistent, aus der Datei) und `configurationOverlay` (aus CLI-Argumenten, nur zur Laufzeit) [V] `CM:52-67`.
- Getter ohne Suffix liefern in der Regel Overlay vor Persistent (`fallThroughOnNull`) [V] `CM:385`. Sonderfälle:
  - `getDefaultBKU()`: Overlay gilt nur, wenn es ungleich `NONE` ist [V] `CM:405-410`.
  - `getProxyPort()`: Overlay gilt nur, wenn es ungleich `-1` ist [V] `CM:569-574`.
  - `getAutoPositionSignature()`: Overlay ODER Persistent [V] `CM:424-426`.
  - Keystore-Passwörter: Zuerst das Overlay. Das persistente Passwort wird nur verwendet, wenn `KEYSTORE_PASS_STORE_TYPE=disk` [V] `CM:808-836`.
  - Mobile-Passwort: Es gibt **nur** das Overlay, es wird nie persistiert [V] `CM:464-467`.
- `saveToDisk()` schreibt ausschließlich persistente Werte. CLI-Overlays werden nie gespeichert [V] `CM:280-383`.
- Reihenfolge beim Start [V] `PCS:243-311`:
  1. `~/.pdf-over` und `PDF-Over.config` fehlen: anlegen und `PDFASConfig.zip` entpacken (`createConfiguration`).
  2. Andernfalls `.version` prüfen und das PDF-AS-Config-Update ausführen, falls nötig (siehe 1.8).
  3. CLI-Argumente nur für `-c` auswerten (eigener Handler).
  4. Konfigurationsdatei laden.
  5. Alle CLI-Argumente auswerten (Overlays).
  6. Ist `UPDATE_CHECK` aktiv, den Update-Check starten (asynchron).
  7. BKU und Auto-Positionierung in den `Status` übernehmen, dann weiter zu `OpenState`.
- **Wann wird gespeichert?** [V]
  - beim Klick auf „Speichern“ im Einstellungsdialog (`CC:338-355`),
  - beim **Schließen des Hauptfensters** über das Shell-Close-Event, inkl. Fenstergröße (`gui/MainWindow.java:223-235`).
  - [I] `StateMachine.exit()` ruft nur `shell.dispose()` auf (`gui/workflow/StateMachine.java:220-225`). Das löst kein `SWT.Close`/`shellClosed` aus. Beim Beenden über das macOS-Menü „Beenden“ (Quit-Listener ruft `exit()` auf, `gui/MainWindow.java:332-334`) und bei `-s` (Skip-Finish, `gui/workflow/states/OutputState.java:99-100`) werden deshalb Fenstergröße, `LAST_SOURCE_DIRECTORY` und gemerkte Mobilnummer vermutlich **nicht** gespeichert.
- Werte, die außerhalb des Einstellungsdialogs persistent gesetzt werden [V]:
  - `LAST_SOURCE_DIRECTORY` beim Öffnen eines Dokuments: `gui/workflow/states/OpenState.java:123`
  - `MOBILE_NUMBER` und `MOBILE_REMEMBER_PASSWORD` im ID-Austria-Login: `gui/workflow/states/MobileBKUState.java:230-262`
  - `KEYSTORE_STOREPASS`/`KEYSTORE_KEYPASS` nach erfolgreicher Eingabe, wenn `disk` gewählt ist und die Keystore-Datei der persistenten entspricht: `gui/workflow/states/KSState.java:118-129,168-176`
  - `MAINWINDOW_SIZE`: `gui/MainWindow.java:227`.

### 1.5 Kompatibilitätstabelle aller Keys in `PDF-Over.config`

Legende: „nur wenn …“ heißt, dass der Key beim Speichern nur unter dieser Bedingung geschrieben wird, sonst fehlt er. Booleans: `TRUE="true"`, `FALSE="false"` [V] `C:121-124`.

| Key | Bedeutung | Typ / Werte | Default, wenn Key fehlt | Lese-Regel | Schreib-Regel | UI-Ort | CLI-Overlay | Fundstellen |
|---|---|---|---|---|---|---|---|---|
| `BKU` | Voreingestelltes Signaturmittel | Enum `LOCAL`, `MOBILE`, `KS`, `NONE` | `NONE` | `BKUs.valueOf`; ungültig führt zu `NONE` mit Log-Error | immer (`name()`) | Erweitert → BKU-Auswahl (Combo) | `-b LOCAL\|MOBILE\|KS` | `C:129`; `pdf-over-commons/.../BKUs.java:22-34`; `CM:197-210,287`; `ACC:172-185` |
| `SIGNATURE_POSITION` | Automatische Positionierung | `auto` oder leer. Der Kommentar nennt `x=..;y=..;p=..` (Altformat), der Code wertet aber nur `auto` aus | aus | `trim().equals("auto")` | immer: `auto` oder `""` | Erweitert → Signatur → „Automatische Positionierung“ | `-a` | `C:131-139`; `CM:234-235,333-336`; `ACC:147-150` |
| `USE_MARKER` | Nach QR-Code-Platzhaltern suchen | bool | false | `equalsIgnoreCase("true")`; gesetzt erzwingt `USE_FIELDS=false` | nur wenn true | Erweitert → Platzhalter → Radio 1 | – | `C:142`; `CM:160-162,319-320,879-882`; `ACC:162-165` |
| `USE_FIELDS` | Nach Signaturfeld-Platzhaltern suchen | bool | false | wie oben; wird **nach** `USE_MARKER` gelesen und gewinnt, wenn beide true | nur wenn true | Erweitert → Platzhalter → Radio 2 | – | `C:145`; `CM:164-166,322-324,870-873` |
| `USE_PLACEHOLDER_SEARCH` | Platzhaltersuche generell aktiv | bool | false | `equalsIgnoreCase` | nur wenn true | Erweitert → Platzhalter → Checkbox | – | `C:148`; `CM:168-170,326-328`; `ACC:157-160` |
| `MOBILE_NUMBER` | Vorbelegter ID-Austria-Benutzername oder Telefonnummer | String; normalisiert (`0664…` wird zu `+43664…`, `00xx` zu `+xx`, Testnummern `10301…` bleiben; sonst unverändert, da ggf. Benutzername) | – | leer führt zu null; `normalizeMobileNumber` | nur wenn nicht null | Einfach → ID Austria → „Benutzername“ | `-n` | `C:154`; `CM:133,300`; `CD:72-80`; `gui/bku/mobile/MobileBKUValidator.java:37-78`; `SCC:119-151` |
| `MOBILE_REMEMBER_PASSWORD` | Benutzername merken und Passwort für die Sitzung merken | bool, **case-sensitiv** `"true"` | false | `"true".equals(...)` | nur wenn true | **nicht** im Einstellungsdialog, sondern Checkbox im ID-Austria-Login | – | `C:155`; `CM:134,301-302`; `gui/workflow/states/MobileBKUState.java:251-262` |
| `SIGNATURE_NOTE` | Hinweistext im Signaturblock | String (Latin-1 + `\u`-Escapes) | profil- und sprachabhängiger Standardtext (`simple_config.Note_Default_Standard` bzw. `..._OfficialSignature`, sonst `""`) | Key vorhanden: Wert, leer wird zu null (dann **kein** Hinweis, Profil ohne `_NOTE`). Key fehlt: Standardtext | **immer** (null wird zu `""`) | Einfach → Signaturhinweis | – | `C:158`; `CM:176-179,299,652-662`; `pdf-over-commons/.../Profile.java:31-40`; `SCC:190-236` |
| `SIGNLOCALE` | Sprache des Signaturblocks | `Locale.toString()`, praktisch `de` oder `en` | System-Locale, auf `de`/`en` abgebildet, sonst `de` | Abgleich gegen `Locale.getAvailableLocales()`; unbekannt wird ignoriert | immer | Einfach → „Sprache des Signaturblocks“ | – | `C:161`; `CM:154-158,314-317`; `gui/utils/LocaleSerializer.java:32-56`; `pdf-over-commons/.../Messages.java:48-55` |
| `SIGNATURE_PDFA_COMPAT` | PDF/A-kompatible Signatur (Profil-Suffix `_PDFA`) | bool | false | `equalsIgnoreCase` | nur wenn true | Erweitert → Signatur → „PDF/A-Kompatibilität“ | – | `C:164`; `CM:181-183,330-331`; `ACC:152-155` |
| `SIGNATURE_PROFILE` | Signaturprofil | `SIGNATURBLOCK_SMALL`, `AMTSSIGNATURBLOCK`, `BASE_LOGO`, `INVISIBLE` | `SIGNATURBLOCK_SMALL` | `Profile.getProfile`; unbekannt wird null, also Default | immer | Einfach → „Signatur-Profil“ | `-v <beliebig>` erzwingt `INVISIBLE` per statischem Flag, nicht persistent | `C:58`; `CM:172-174,373,892-898`; `pdf-over-commons/.../Profile.java:5-29`; `cli/InvisibleProfile.java:43-51` |
| `EMBLEM` | Pfad zur Bildmarke (Logo) | Dateipfad | keiner (Standard-Logo aus dem PDF-AS-Profil) | Datei muss existieren und von SWT ladbar sein, sonst null (Log-Error) | nur wenn nicht null | Einfach → Vorschau → „Logo auswählen/zurücksetzen“, Drag & Drop | `-e` | `C:187`; `CM:126,296,477-492`; `CD:48-69`; `SCC:283-381` |
| `LOGO_ONLY_SIZE` | Zielgröße der Bildmarke im Profil `BASE_LOGO` | double, **Millimeter** | `23` (Text `23.0`) | `Double.parseDouble`; Fehler werden ignoriert | immer (`Double.toString`, z. B. `23.0`) | Einfach → „Größe der Bildmarke“ (Slider 10–200 mm, Anzeige in cm; nur bei `BASE_LOGO` sichtbar) | – | `C:99-102,190`; `CM:127-131,297`; `SCC:238-272`; bei Positionierung auf 120 begrenzt: `gui/workflow/states/PositioningState.java:116` |
| `LOCALE` | Sprache der Oberfläche | `de` oder `en` | System-Locale, auf `de`/`en` abgebildet | wie `SIGNLOCALE`; setzt zusätzlich `Locale.setDefault` | immer | Erweitert → „Interface-Sprache“ | – | `C:75,193`; `CM:148-152,309-312,664-672`; `ACC:250-262` |
| `UPDATE_CHECK` | Beim Start nach Updates suchen | bool | **true** | jeder Wert außer `false` (case-insensitiv) ergibt true | nur wenn false | **Über** → Checkbox (nicht „Erweitert“, wie es die Webseite angibt [E]) | – | `C:196`; `CM:264-266,367-368`; `CD:143`; `AC:67-75` |
| `PROXY_HOST` | Proxy-Hostname/IP | String | keiner | leer wird zu null | nur wenn nicht null | Erweitert → Proxy → Host | `-proxy` | `C:199`; `CM:136,289`; `ACC:264-296` |
| `PROXY_PORT` | Proxy-Port | int 1–65535 | `-1` (keiner) | `Integer.parseInt` **ohne** Catch: Nicht-Zahl führt zum Abbruch der Initialisierung mit Factory-Reset-Dialog [V]; außerhalb des Bereichs nur Warnung | nur wenn ≠ -1 | Erweitert → Proxy → Port | `-proxyport` | `C:202`; `CM:185-194,290-292`; `CD:91-103`; `ACC:298-325,484-497` |
| `PROXY_USER` | Proxy-Benutzer | String | keiner | leer wird zu null | nur wenn nicht null | **keine UI** (auskommentiert) | `-proxyuser` | `C:205`; `CM:137,293`; `ACC:689-700` |
| `PROXY_PASS` | Proxy-Passwort (**Klartext**) | String | keiner | leer wird zu null | nur wenn nicht null | **keine UI** | `-proxypass` | `C:208`; `CM:138,294` |
| `OUTPUT_FOLDER` | Standard-Ausgabeordner; leer heißt Dialog „Speichern unter“; ein relativer Pfad (z. B. `.`) wird relativ zum Ordner des Eingabedokuments aufgelöst | String (Pfad) | keiner | leer wird zu null | nur wenn nicht null | Erweitert → Ausgabe → „Standard-Ausgabeordner“ + Durchsuchen | `-o` (Ordner oder Datei) | `C:211`; `CM:140,303`; `ACC:192-231,622-634`; `gui/composites/OutputComposite.java:308-366` |
| `SAVE_FILE_POSTFIX` | Suffix des Ausgabedateinamens | String | `_signed` | fehlt: Default | immer | Erweitert → Ausgabe → „Ausgabedatei Postfix“ (leer setzt `_signed`) | – | `C:59,213`; `CM:142-146,304`; `ACC:233-248`; `gui/composites/OutputComposite.java:423-428` |
| `MAINWINDOW_SIZE` | Fenstergröße | `width,height` (int) | `600,780` | Parse-Fehler: Default | immer | – (wird beim Schließen gemerkt) | – | `C:216,254-257`; `CM:212-231,306-307`; `gui/MainWindow.java:202,227` |
| `KEYSTORE_ENABLED` | Keystore-Signatur aktiviert (blendet Tab und BKU-Option `KS` ein) | bool (`Boolean`, null = false) | false | `equalsIgnoreCase` | nur wenn true | Erweitert → BKU-Auswahl → „Keystore-Signatur aktivieren“ | implizit durch `-ks` | `C:167`; `CM:238-240,341-342`; `ACC:187-190,431-449`; `cli/KeystoreFileArgument.java:54-55` |
| `KEYSTORE_FILE` | Pfad zur Keystore-Datei | String | `""` | leer wird zu `""` | nur wenn nicht leer | Keystore → „Keystore-Datei“ | `-ks` | `C:170`; `CM:241,343,720-742`; `KCC:120-125,174-202` |
| `KEYSTORE_TYPE` | Keystore-Typ (Java `KeyStore.getInstance`) | UI: `PKCS12` oder `JCEKS` (Label „Java-Keystore“). CLI: beliebig (Hilfe nennt `PKCS12\|JKS\|JCEKS`) | `""` | – | nur wenn nicht leer | Keystore → „Keystore-Typ“; wird aus der Dateiendung abgeleitet (`.p12/.pkcs12/.pfx` ergibt PKCS12, `.ks/.jks` ergibt JCEKS) | `-kstype` | `C:173`; `CM:242,344`; `KCC:335-366,543-547`; `gui/keystore/KeystoreUtils.java:13-27` |
| `KEYSTORE_ALIAS` | Schlüssel-Alias | String | `""` | – | nur wenn nicht leer | Keystore → „Schlüssel-Alias“ (editierbare Combo, wird über „Keystore laden“ befüllt) | `-ksalias` | `C:176`; `CM:243,345`; `KCC:144-147,256-269,299-330` |
| `KEYSTORE_PASS_STORE_TYPE` | Speicherung der Keystore-Passwörter | `none` \| `memory` \| `disk` | **Auto-Migration:** ist `KEYSTORE_STOREPASS` oder `KEYSTORE_KEYPASS` nicht leer, dann `disk`, sonst `memory` | unbekannter Wert ergibt `none` (null) | immer | Keystore → „Keystore-Passwörter speichern?“ (Nicht speichern / Temporär / Permanent) | – | `C:178`; `CM:246-261,347-353`; `KCC:368-389,549-564` |
| `KEYSTORE_STOREPASS` | Keystore-Passwort (**Klartext**) | String | – | – | **nur wenn `disk`** (null wird zu `""`) | Keystore → „Keystore-Passwort“ (nur bei `disk` sichtbar) | `-kspass` | `C:181`; `CM:244,355-360`; `KCC:162-164` |
| `KEYSTORE_KEYPASS` | Schlüssel-Passwort (**Klartext**) | String | – | – | **nur wenn `disk`** | Keystore → „Schlüssel-Passwort“ | `-kskeypass` | `C:184`; `CM:245,361-364`; `KCC:170-172` |
| `FIDO2_DEFAULT` | FIDO2 bei ID Austria standardmäßig wählen | bool | false | `"true".equalsIgnoreCase` | nur wenn true | FIDO2 → „FIDO2 standardmäßig auswählen“ (nur wenn WebAuthN verfügbar) | – | `C:221`; `CM:268,370-371`; `FCC:51-59`; `gui/bku/MobileBKUConnector.java:56` |
| `LAST_SOURCE_DIRECTORY` | Zuletzt geöffneter Ordner im Datei-Dialog | String | keiner | – | nur wenn nicht null | – (automatisch) | – | `C:226`; `CM:270,375`; `gui/workflow/states/OpenState.java:123`; `gui/composites/DataSourceSelectComposite.java:62` |
| `THEME` | Farbschema | `DEFAULT` \| `GEMPLUSH` (Eingabe case-insensitiv) | `DEFAULT` | statisch beim Klassenladen, nur aus `PDF-Over.config` | nur wenn ≠ `DEFAULT` | keine UI (Easter Egg) | – | `C:219,230-251,262-298`; `CM:338-339` |
| `CRASH` | Debug: absichtlicher Absturz | `startup` \| `config` | – | `startup` wirft beim Laden eine Exception, `config` beim Öffnen der Einstellungen | **nie** | – | – | `CM:116-124`; `CC:293-294` |

**Definierte, aber ungenutzte Konstanten** (in alten Konfigurationen evtl. vorhanden; ein Klon kann sie ignorieren) [V]: `SIGNATURE_PLACEHOLDER_TRANSPARENCY` (`C:151`, UI-Texte `advanced_config.SigPHTransparency*` existieren noch in `res/at/asit/pdfover/gui/messages_de.properties:38-41`), `DOWNLOAD_URL` (`C:224`), `SIGNED_SUFFIX` (`C:97`), `MOBILE_BKU_URL_TEST` (`C:109`), `RES_CERT_LIST` (`C:370`). Weil das Repository als Shallow Clone mit nur einem Commit vorliegt, ließ sich nicht prüfen, welche Keys es in älteren Versionen gab.

**Reine CLI-Overlays ohne Persistenz** [V]: `-p` (ID-Austria-Passwort, `cli/PasswordArgument.java:50`), `-s` (Skip-Finish, `cli/SkipFinishArgument.java:43`), `-i` bzw. nackte Dateipfade (Eingabedokumente, mehrere werden zur Warteschlange), `-h/-?/--help`. Ein Argument, das kein bekanntes Flag ist, wird als Eingabedokument interpretiert [V] `cli/ArgumentHandler.java:91-104`, `cli/InputDocumentArgument.java:48-61`. Das ist der Mechanismus hinter „Öffnen mit“ und „Senden an“. Die englischen Hilfetexte zu `-kspass`/`-kskeypass` sind vertauscht [V] `res/at/asit/pdfover/gui/messages.properties:63-64`, `res/ReadMe.txt:21-26`. Der deutsche Text ist korrekt (`messages_de.properties:61-62`). Bei `-v` wird der Wert ignoriert: auch `-v true` macht die Signatur unsichtbar [V] `cli/InvisibleProfile.java:46-50`.

### 1.6 Ableitung der PDF-AS-Profil-ID aus der Konfiguration

Relevant, wenn ein Klon die mitgelieferten PDF-AS-Profile als Datenquelle weiterverwendet [V] `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4SignatureParameter.java:156-193`:

| `SIGNATURE_PROFILE` | Ergebnis-Profil-ID |
|---|---|
| `INVISIBLE` oder CLI `-v` | `INVISIBLE` |
| `BASE_LOGO` | `BASE_LOGO` |
| `AMTSSIGNATURBLOCK` | `AMTSSIGNATURBLOCK_DE_RECOMMENDED` / `AMTSSIGNATURBLOCK_EN_RECOMMENDED` (Note und PDF/A werden ignoriert) |
| `SIGNATURBLOCK_SMALL` | `SIGNATURBLOCK_SMALL_{DE\|EN}` + `_NOTE` (wenn Note ≠ null) + `_PDFA` (wenn PDF/A aktiv) |

Die Sprache kommt aus `SIGNLOCALE` (`en` ergibt `_EN`, alles andere `_DE`). Der Signaturhinweis wird zur Laufzeit als `sig_obj.<Profil>.value.SIG_NOTE` in die PDF-AS-Konfiguration geschrieben [V] `pdf-over-signer/src/main/java/at/asit/pdfover/signer/pdfas/PdfAs4Signer.java:62-68`.

### 1.7 PDF-AS-Konfiguration: Entpacken und Aktualisieren

**Inhalt von `res/cfg/PDFASConfig.zip`** [V]: 1 067 408 Bytes komprimiert, 1 949 242 Bytes entpackt, 66 Einträge (inkl. Verzeichnisse), Wurzelordner `cfg/`, `fonts/`, `images/`, `licenses/`. 20 Profildateien in `cfgzip/cfg/profiles/`. `cfgzip/cfg/config.properties` bindet alle Profile über `include.01 = profiles/*.properties` ein (`:73`). Sie setzt `sig_obj.type.default=SIGNATURBLOCK_SMALL_DE_NOTE` (`:103`), `bku.sign.url=http://127.0.0.1:3495/http-security-layer-request` (`:25`) und eine veraltete `mobile.sign.url` auf die Handy-Signatur (`:33`). PDF-Over nutzt für ID Austria nicht diese URL, sondern `Constants.MOBILE_BKU_URL` (`C:108`, verwendet in `gui/bku/MobileBKUConnector.java:201`) [V]. Die Kodierung ist uneinheitlich: `advancedconfig.properties` ist ISO-8859-1 mit CRLF, `AMTSSIGNATURBLOCK_BASE*.properties` sind UTF-8, die Nicht-ASCII-Zeichen stehen dort nur in Kommentaren [V]. `fonts/arial.ttf` wird von keinem Profil referenziert [V] (grep über `cfgzip/cfg`).

**Erststart** (`~/.pdf-over` oder `PDF-Over.config` fehlt) [V] `PCS:149-168,250-252`:
1. `mkdir ~/.pdf-over`
2. leere `PDF-Over.config` anlegen
3. ZIP aus dem Classpath nach `~/.pdf-over` entpacken (`gui/utils/Zipper.java:104-135`; mit Zip-Slip-Schutz `sanitizePath` `:89-96`)
4. `.version` mit `Constants.APP_VERSION` schreiben.

Schlägt ein Schritt fehl, wird `configDir.delete()` versucht. Das funktioniert nur bei leerem Verzeichnis [I].

**Update bei bestehender Konfiguration** [V] `PCS:253-260`, `PCS:210-240`:
- Bedingung: `.version` fehlt, ist unlesbar, oder `version < MIN_PDF_AS_CONFIG_VERSION` (`"4.4.6"`, `C:91`), verglichen per `VersionComparator`.
- Ablauf:
  1. `~/.pdf-over/cfg` in eine Temp-Datei `cfg_backup*.zip` zippen, **mit Löschen der Originaldateien** (`Zipper.zip(..., doDelete=true)`, `gui/utils/Zipper.java:58-87`).
  2. Komplettes ZIP neu entpacken. Dabei werden auch `fonts/`, `images/` und `licenses/` überschrieben.
  3. Backup als `~/.pdf-over/cfg_backup.zip` ablegen, bei Kollision `cfg_backup1.zip`, `cfg_backup2.zip`, …
  4. `.version` aktualisieren.
- Folge [I]: Manuelle Anpassungen in `cfg/` gehen bei einem Config-Update verloren und liegen nur noch im Backup-ZIP. Ein Merge findet nicht statt. Die Schwelle `4.4.6` ist fest eincodiert. Für ein neues Profil-Update muss also die Konstante im Code erhöht werden.

**Laufzeit-Schreibzugriff auf die PDF-AS-Config** [V] `gui/workflow/states/OpenState.java:66,239-252`: Wählt der Nutzer im Platzhalter-Dialog ein Signaturfeld, schreibt PDF-Over `signature_field_name=<Feld>` (`C:69`) **dauerhaft** in `~/.pdf-over/cfg/advancedconfig.properties`. Das läuft über Apache Commons Configuration (`PropertiesConfigurationLayout`), gelesen und geschrieben wird mit der Plattform-Standardkodierung (`InputStreamReader`/`FileWriter`). [I] Unter Java 21 ist das UTF-8, die ISO-8859-1-Umlaute in Kommentaren der Datei werden dabei vermutlich beschädigt.

### 1.8 Factory-Reset

[V] `CM:69-105`, ausgelöst über den Ja/Nein-Fehlerdialog bei Initialisierungsfehlern (`PCS:313-327`) oder Fehlern der Einstellungsseite (`gui/workflow/states/ConfigurationUIState.java:69-86`):
1. Logback stoppen.
2. `~/.pdf-over.old` löschen.
3. `~/.pdf-over` nach `~/.pdf-over.old` verschieben.
4. Schlägt das fehl: `forceDelete`, danach `forceDeleteOnExit`.

Beim nächsten Start gilt das als Erststart. Bug [V]: In `messages_de.properties:141-142` ist `error.Initialization` doppelt definiert, `error.ConfigInitialization` fehlt. Der zweite Eintrag gewinnt, deshalb lautet die deutsche Startfehlermeldung „Der Einstellungsbildschirm konnte nicht geladen werden …“. Für den Einstellungsfehler wird über den Fallback auf `messages.properties:143` der englische Text angezeigt [I].

### 1.9 Migrationsalgorithmus für einen Klon (Empfehlung)

1. Legacy-Datei suchen: `~/.pdf-over/PDF-Over.config` (Desktop). Optional zusätzlich alternative Namen, die der Nutzer mit `-c` verwendet hat.
2. Mit einem Java-Properties-kompatiblen Parser lesen (ISO-8859-1 plus `\uXXXX`, Escapes, Fortsetzungszeilen). Als Fallback: Ergibt die Datei als UTF-8 gültige Mehrbyte-Sequenzen, wurde sie wahrscheinlich von Hand in UTF-8 editiert [I].
3. Jeden Key nach Tabelle 1.5 abbilden und dabei die **exakten Lese-Regeln** übernehmen: Default-Note bei fehlendem `SIGNATURE_NOTE`, Auto-Migration von `KEYSTORE_PASS_STORE_TYPE`, `UPDATE_CHECK` nur bei `false` aus, `MOBILE_REMEMBER_PASSWORD` case-sensitiv, `USE_FIELDS` vor `USE_MARKER`.
4. Klartext-Secrets (`KEYSTORE_STOREPASS`, `KEYSTORE_KEYPASS`, `PROXY_PASS`) in den OS-Schlüsselbund übernehmen und dem Nutzer anbieten, sie aus der Legacy-Datei zu löschen. Die Legacy-Datei nicht stillschweigend verändern.
5. Pfade (`EMBLEM`, `KEYSTORE_FILE`, `OUTPUT_FOLDER`, `LAST_SOURCE_DIRECTORY`) auf Existenz prüfen. PDF-Over verwirft eine ungültige `EMBLEM` stillschweigend (`CD:48-69`).
6. `~/.pdf-over/cfg/advancedconfig.properties` auf `signature_field_name` prüfen. Das ist ein von PDF-Over persistierter Zustand (1.7).
7. Eigene, versionierte Konfiguration (z. B. JSON/TOML mit `schemaVersion`) am plattformüblichen Ort anlegen. Siehe Kapitel 7.

---

## 2. Einstellungen-UI

### 2.1 Aufbau und Verhalten

- Zustand `ConfigurationUIState`. Er blendet die Hauptleiste aus und deaktiviert den Config-Button [V] `gui/workflow/states/ConfigurationUIState.java:114-119`.
- Die Tabs liegen in einem `TabFolder` in dieser Reihenfolge: **Einfach**, **Erweitert**, *(Keystore, nur wenn aktiviert, an Index 2)*, **FIDO2**, **Über PDF-Over** [V] `CC:166-214,258-285`, Labels `CC:388-398`.
- Alle Tabs teilen sich **ein** `ConfigurationDataInMemory` als Arbeitskopie [V] `CC:115`. Lebenszyklus pro Tab [V] `pdf-over-gui/.../composites/configuration/ConfigurationCompositeBase.java:51-76`: `initConfiguration(provider)` (aus persistenten Werten), dann `loadConfiguration()` (UI befüllen), `validateSettings(resumeIndex)`, `storeConfiguration(provider)`.
- **Speichern** [V] `CC:306-356`:
  1. Alle Tabs validieren. Ein `ResumableException` öffnet einen Dialog Abbrechen/Wiederholen/Ignorieren. „Ignorieren“ setzt die Validierung beim Resume-Index fort.
  2. Alle Tabs speichern.
  3. `saveToDisk()`, bei IO-Fehler Wiederholen/Abbrechen.
  4. Danach `reloadResources()` (Sprachwechsel ohne Neustart), Rückkehr zum vorherigen Zustand oder bei `OutputState` zu `OpenState` (`gui/workflow/states/ConfigurationUIState.java:88-96`).
- **Abbrechen** verwirft die Arbeitskopie [V] `CC:230-240`.
- Der Fenstertitel zeigt im Dialog `PDF-Over v<Version> [<java.vendor> Java <java.version>]` [V] `CC:246`.
- Alle Labels kommen aus `res/at/asit/pdfover/gui/messages{,_de}.properties`; `messages_en.properties` ist ein Symlink auf `messages.properties` [V].

### 2.2 Tab „Einfach“ (`SimpleConfigurationComposite`)

| Gruppe (Label-Key) | Element | Wirkung / Key | Fundstelle |
|---|---|---|---|
| ID Austria (`simple_config.MobileBKU_Title`) | Textfeld „Benutzername:“ mit Platzhalter `simple_config.ExampleNumber` und ErrorMarker | `MOBILE_NUMBER`; wird bei Fokusverlust/Enter normalisiert | `SCC:119-151,482-494,552-561` |
| Signatur-Profil | Read-only-Combo: Standard Signaturblock / Amtssignatur / Nur Bildmarke / Unsichtbar | `SIGNATURE_PROFILE`; blendet Sprache/Hinweis/Logo-Größe/Vorschau ein oder aus; ersetzt den Standard-Hinweistext, wenn er unverändert war | `SCC:153-169,523-542` |
| Sprache des Signaturblocks | Combo Deutsch/Englisch | `SIGNLOCALE`; ersetzt den Standard-Hinweistext, wenn er unverändert war | `SCC:171-188,508-519` |
| Signaturhinweis | Textfeld + Button „Standard wiederherstellen“ | `SIGNATURE_NOTE` | `SCC:190-236` |
| Größe der Bildmarke (nur `BASE_LOGO`) | Slider 10–200 (mm), Anzeige `x.x cm`, Labels 1 cm / 20 cm | `LOGO_ONLY_SIZE` | `SCC:238-272` |
| Vorschau (nur sichtbare Profile) | Buttons „Logo auswählen“ (Filter `*.jpg;*.png;*.gif`), „Logo zurücksetzen“; Canvas mit Live-Vorschau des Signaturblocks über PDF-AS; Drag & Drop einer Bilddatei (Drop wechselt auf Profil `BASE_LOGO`) | `EMBLEM` | `SCC:274-381,396-468` |

Validierung: Nummer normalisieren, Hinweis übernehmen [V] `SCC:647-659`.

### 2.3 Tab „Erweitert“ (`AdvancedConfigurationComposite`)

| Gruppe | Element | Key | Fundstelle |
|---|---|---|---|
| Interface-Sprache | Combo Deutsch/Englisch | `LOCALE` | `ACC:117-120,250-262` |
| Signatur | Checkbox „Automatische Positionierung“ | `SIGNATURE_POSITION` | `ACC:147-150` |
| | Checkbox „PDF/A-Kompatibilität“ (Tooltip: Original muss bereits PDF/A sein) | `SIGNATURE_PDFA_COMPAT` | `ACC:152-155` |
| Platzhalter | Checkbox „Platzhalter Suche aktivieren“, aktiviert die beiden Radios | `USE_PLACEHOLDER_SEARCH` | `ACC:157-160,419-424` |
| | Radio „Nach QR-Code Signaturplatzhaltern suchen“ | `USE_MARKER` | `ACC:162-165` |
| | Radio „Nach Signaturfelder-Platzhaltern suchen“ | `USE_FIELDS` | `ACC:167-170` |
| BKU-Auswahl | Combo: Lokale Bürgerkarte / ID Austria / Keystore / Keine Voreinstellung | `BKU` | `ACC:172-185`; Labels `messages_de.properties:3-6` |
| | Checkbox „Keystore-Signatur aktivieren“: fügt den Combo-Eintrag `KS` und den Keystore-Tab hinzu oder entfernt sie; beim Deaktivieren wird eine KS-Auswahl auf `NONE` gesetzt | `KEYSTORE_ENABLED` | `ACC:187-190,431-449`; `CC:258-285` |
| Ausgabe Einstellungen | „Standard-Ausgabeordner:“ + „Durchsuchen“ (DirectoryDialog); Tooltip: leer = deaktiviert, `.` = Ordner des Originals | `OUTPUT_FOLDER` | `ACC:192-231`; `messages_de.properties:22` |
| | „Ausgabedatei Postfix:“ (leer ergibt `_signed`) | `SAVE_FILE_POSTFIX` | `ACC:233-248` |
| Proxy | „Host:“ mit ErrorMarker | `PROXY_HOST` | `ACC:264-296` |
| | „Port:“ mit ErrorMarker (NumberFormat/Range) | `PROXY_PORT` | `ACC:298-325,470-497` |
| | Benutzer/Passwort: **auskommentiert**, nicht in der UI | `PROXY_USER`/`PROXY_PASS` | `ACC:689-700` |

Validierung [V] `ACC:619-648`: Ausgabeordner muss existieren und ein Verzeichnis sein (`OutputfolderDoesntExistException` ist resumable), danach Proxy-Host und Proxy-Port. Beim Entfernen von Proxy-Werten werden die JVM-System-Properties gelöscht [V] `ACC:575-610`.

### 2.4 Tab „Keystore“ (`KeystoreConfigurationComposite`, nur bei `KEYSTORE_ENABLED`)

| Element | Verhalten | Key | Fundstelle |
|---|---|---|---|
| Keystore-Datei + Durchsuchen | Filter `*.p12;*.pkcs12;*.pfx;*.ks;*.jks`; der Typ wird aus der Endung abgeleitet | `KEYSTORE_FILE`, `KEYSTORE_TYPE` | `KCC:120-125,174-202,335-352` |
| Keystore-Typ (Combo) + „Keystore laden“ | PKCS12 / „Java-Keystore“ (= `JCEKS`). Laden fragt bei Bedarf das Keystore-Passwort ab und füllt die Alias-Liste | `KEYSTORE_TYPE` | `KCC:131-138,223-254,299-330,543-547` |
| Schlüssel-Alias (editierbare Combo) | Freitext oder Auswahl | `KEYSTORE_ALIAS` | `KCC:144-147,256-269` |
| „Keystore-Passwörter speichern?“ | Nicht speichern (Eingabe bei jeder Signatur) / Temporär (bis Programmende) / Permanent (auf Disk, **Klartext**, Tooltip warnt) | `KEYSTORE_PASS_STORE_TYPE` | `KCC:149-156,368-389,549-564`; `messages_de.properties:187-194` |
| Keystore-Passwort, Schlüssel-Passwort | Nur bei „Permanent“ sichtbar | `KEYSTORE_STOREPASS`, `KEYSTORE_KEYPASS` | `KCC:158-172,379-388` |

Validierung [V] `KCC:493-539`: Datei existiert. Ist ein Passwort hinterlegt, wird der Keystore geladen, ohne einen Prompt zu provozieren. Danach wird geprüft, ob der Alias existiert und ein Key-Entry ist, und ob das Key-Passwort passt. Bei „Temporär“ werden die Passwörter als Overlay gesetzt (`KCC:479-483`).

### 2.5 Tab „FIDO2“ (`Fido2ConfigurationComposite`)

- Infotext mit Link auf `config.fido2.InfoURL` (oesterreich.gv.at) [V] `FCC:32-35`, `messages_de.properties:120,123`.
- Ist `WebAuthN.isAvailable()` true, erscheint die Gruppe „FIDO2-Einstellungen“ mit der Checkbox „FIDO2 standardmäßig auswählen“ (`FIDO2_DEFAULT`). Andernfalls zeigt Linux/macOS einen Hinweis auf libfido2 mit Link `https://developers.yubico.com/libfido2/#_installation`, Windows den fett gesetzten Text „Ihr System unterstützt keine Authentifizierung mittels FIDO2-Token“ [V] `FCC:37-75`. Die Plattformerkennung läuft über `com.sun.jna.Platform` (`FCC:10`).

### 2.6 Tab „Über PDF-Over“ (`AboutComposite`)

| Element | Verhalten | Fundstelle |
|---|---|---|
| Lizenz-/Supporttext (Link) | EUPL-Hinweis, Link auf die Lizenzbedingungen von A-SIT, Support-Mailadresse; Hinweis, dass Komponenten eigene Lizenzen haben, die die freie Nutzung teils auf EUPL-Software beschränken | `AC:41-44`; `messages_de.properties:111` |
| „Datenschutzinformation“ (fett) + Datenschutztext mit Link auf die A-Trust-Datenschutzerklärung | siehe 3.4 | `AC:46-54`; `messages_de.properties:112-114` |
| Checkbox „Automatisch auf Updates überprüfen“ | `UPDATE_CHECK`; beim Aktivieren sofortiger Check | `AC:67-75` |
| Status-Link Update-Check | Zustände `NOT_CHECKED/CHECKING/OUTDATED/UP_TO_DATE/FAILED`. Klick bei OUTDATED öffnet die Download-Seite, sonst neuer Check | `AC:56-65,96-98`; `messages_de.properties:264-268` |
| Button „Logs anzeigen“ | öffnet `~/.pdf-over/logs` über `java.awt.Desktop.open` | `AC:77-90` |

---

## 3. Proxy, Logging, Update-Check, Datenschutz, TLS

### 3.1 Proxy

- Quellen: `PROXY_*` aus der Datei, alternativ CLI-Overlays (1.5).
- Wirksam werden die Werte **erst beim Signieren**: In `PrepareSigningState` setzt PDF-Over die JVM-System-Properties `http(s).proxyHost`, `http(s).proxyPort`, `http(s).proxyUser`, `http(s).proxyPassword` und registriert einen globalen `java.net.Authenticator`. Dieser beantwortet nur Proxy-Requests, deren Host und Port exakt passen [V] `gui/workflow/states/PrepareSigningState.java:75-125`.
- HTTP-Clients:
  - Apache HttpClient 5 mit `useSystemProperties()` für ID Austria und den Update-Check [V] `gui/utils/HttpClientUtils.java:7-9`, `gui/bku/MobileBKUConnector.java:74,248`, `gui/utils/UpdateCheckManager.java:51`.
  - Legacy commons-httpclient 3.1 für die lokale BKU über `BKUHelper.getHttpClient()`. Hier ist `useProxy=false`, die lokale BKU wird also immer ohne Proxy erreicht (`gui/bku/BKUHelper.java:62-97`, `gui/bku/LocalBKUConnector.java:83`) [V].
- [I] Der Update-Check beim Start (`PCS:303-304`) läuft **vor** `PrepareSigningState` und berücksichtigt deshalb den in PDF-Over konfigurierten Proxy nicht. Es gelten nur JVM-Defaults und `-D…`-Properties. In Proxy-Umgebungen schlägt er daher vermutlich fehl (Status `FAILED`).
- Es gibt keine Unterstützung für PAC/WPAD und keine Option „System-Proxy verwenden“ [V] (keine entsprechende Konfiguration im Code).

### 3.2 Logging

- SLF4J 2.0.9 mit Logback-classic 1.5.18 [V] `pdf-over-commons/pom.xml:13-18`, `pdf-over-gui/pom.xml:23-27`.
- Konfiguration `res/logback.xml` [V]:
  - Appender STDOUT, Muster `%-5p | %t @ %c{40} | %m%n`, UTF-8 (`:4-9`).
  - Appender LOGFILE: `${user.home}/.pdf-over/logs/pdf-over.log`, `FixedWindowRollingPolicy` bis Index 10, `SizeBasedTriggeringPolicy` 100 KB. Insgesamt maximal ca. 1,1 MB Logs (`:10-23`).
  - Logger `at.asit.pdfover` auf INFO, Root auf WARN, also PDF-AS (`at.gv.egiz`) nur ab WARN (`:24-28`).
  - Optionales Include `${user.home}/.pdf-over-logging.xml` zum Erhöhen des Levels (`:29`).
- `~/.pdf-over/cfg/logback.xml` und `log4j.properties` stammen aus dem PDF-AS-ZIP. [I] Sie liegen nicht auf dem Classpath und haben in PDF-Over vermutlich keine Wirkung.
- Datenschutzrelevanz der Logs [V]: Beim Start wird Version/OS/Java auf INFO geloggt (`gui/Main.java:37-39`), Konfigurationspfade auf INFO (`CM:272,382`). Auf DEBUG kommen u. a. der Proxy-Benutzername (`gui/workflow/states/PrepareSigningState.java:95`) und Dateipfade (`KCC:336`) hinzu.

### 3.3 Update-Check

| Aspekt | Verhalten | Fundstelle |
|---|---|---|
| Endpoint | `GET https://updates.a-sit.at/pdf-over/Release.txt` | [V] `C:115` |
| Format | Kompletter Response-Body, `trim()`, ergibt Versionsstring. Gemessen am 2026-10-14: Body `4.4.9` (5 Bytes, kein Zeilenumbruch), `Content-Type: text/plain`, HSTS aktiv | [V] `gui/utils/UpdateCheckManager.java:53`; [E] |
| Prüfung | `lessThan(APP_VERSION, latest)` ergibt `OUTDATED`, sonst `UP_TO_DATE`. **Kein** HTTP-Statuscode-Check, keine Signatur, keine Prüfsumme. [I] Eine 404-HTML-Seite würde als Version „0“ geparst und zu `UP_TO_DATE` führen | [V] `gui/utils/UpdateCheckManager.java:50-74` |
| Benachrichtigung | Info-Dialog „Neue Version verfügbar!“ / „Version %s ist verfügbar. Jetzt Download-Seite öffnen?“. OK öffnet `https://technology.a-sit.at/en/pdf-over/` im Browser. Pro Prozess höchstens einmal je neuerer Version (`latestVersionNotified`) | [V] `gui/utils/UpdateCheckManager.java:57-66`; `C:118`; `messages_de.properties:260-261` |
| Auto-Update | **Keines.** Es gibt nur einen Hinweis, Download und Installation macht der Nutzer manuell | [V] |
| Auslöser | Programmstart (wenn `UPDATE_CHECK`), Aktivieren der Checkbox, Klick auf den Status-Link | [V] `PCS:303-304`; `AC:60-75` |
| Threading | Eigener Thread, Status-Callbacks an die UI | [V] `gui/utils/UpdateCheckManager.java:25-47,76-104` |
| Bug | `checkNow` setzt `needsCheck=true` vor dem Thread-Start. Die Schleife sieht beim ersten Durchlauf `needsCheck==true` und prüft deshalb ein zweites Mal: **jeder Check macht zwei HTTP-Requests**. Ein erneuter Aufruf während eines laufenden Checks wird verworfen (`return` vor `needsCheck=true`) | [V] `gui/utils/UpdateCheckManager.java:80-100` |
| Datenschutz | Der Server sieht IP und User-Agent. [I] Der User-Agent ist der Apache-HttpClient-5-Default, PDF-Over setzt keinen eigenen | [I] |

**Versionsvergleich `VersionComparator`** [V] `gui/utils/VersionComparator.java:26-65`:
1. Ein Suffix `-SNAPSHOT` am Ende wird entfernt und als Flag gemerkt.
2. Der Rest wird an `.` gesplittet. Jede Komponente ist `Integer.parseInt`, bei einem Fehler **0**. Damit wird `10-rc1` zu 0.
3. Verglichen wird komponentenweise bis zur Länge der längeren Version. Fehlende Komponenten zählen als 0 (`4.0` == `4.0.0`).
4. Bei Gleichheit gilt: SNAPSHOT < Release.

Testvektoren aus `pdf-over-gui/src/test/java/at/asit/pdfover/gui/utils/VersionComparatorTest.java:10-17`: `4.0 == 4.0.0`; `4.4.4-SNAPSHOT < 4.4.4`; `4.4.4-SNAPSHOT > 4.4.3`; `4.4.3-SNAPSHOT < 4.4.3.1`; `4.4.3.1 < 4.4.4-SNAPSHOT`. Ist `version.properties` nicht ladbar, wird `UNKNOWN-SNAPSHOT` verwendet. Das parst als `0` und gilt immer als veraltet [V] `C:40-53`. Die Version stammt aus `pdf-over-commons/src/main/resources/version.properties:1` (`version=${project.version}`, per Maven-Filtering, `pdf-over-commons/pom.xml:37-44`).

### 3.4 Datenschutz-Hinweise

- Inhalt [V] `res/at/asit/pdfover/gui/messages_de.properties:112-114`, `Readme.md:20-26`:
  - Die Verarbeitung erfolgt lokal.
  - Die Signatur mit der Bürgerkarte erfolgt lokal.
  - Bei ID Austria wird das **zu signierende PDF an A-Trust gesendet**.
  - Für Speicherdauer und weitere Infos wird auf die A-Trust-Datenschutzerklärung verwiesen (`https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf`).
- Nicht im Text erwähnt [I]: der Update-Check gegen `updates.a-sit.at`, der IP und Zeitpunkt preisgibt, und Klartext-Passwörter auf der Disk bei `disk`-Speicherung bzw. `PROXY_PASS`.

### 3.5 TLS und Zertifikate

- Truststore [V]: Es wird der Standard-Truststore der gebündelten JRE verwendet. `Main` erzwingt `javax.net.ssl.trustStoreType=jks` (Workaround #95) und registriert den IAIK-JCE-Provider (`gui/Main.java:46-50`). Es gibt kein Pinning und keine eigene TrustManager-Implementierung (grep nach `SSLContext|TrustManager` ohne Treffer).
- `res/certificates/` (A-Trust-nQual-01a, A-Trust-nQual-03, A-Trust-Root-05, Comodo RSA CA, IAIK TLS CA + `certificates.xml`) ist **toter Bestand**: `RES_CERT_PATH`/`RES_CERT_LIST` sind definiert (`C:366-370`), werden aber nirgends verwendet [V]. Mehrere Zertifikate sind abgelaufen (nQual-01a 2014, nQual-03 2015, Root-05 2023) [V] (openssl, nur lesend).
- Die lokale BKU spricht **unverschlüsselt** HTTP mit `127.0.0.1:3495` (`C:105`; `cfgzip/cfg/config.properties:22-25` nennt alternativ HTTPS 3496) [V].
- `res/example.cer` sowie die Test-Keystores `res/keystore.jks` (nur für das Dummy-Jar-Signing, aus den Ressourcen ausgeschlossen: `pdf-over-gui/pom.xml:466`) und `pdf-over-gui/src/main/jnlp/keystore.ks` sind vorhanden. Inhalte und Passwörter werden hier bewusst nicht wiedergegeben.

---

## 4. Plattformintegration

### 4.1 Übersicht

| Funktion | Windows | macOS | Linux |
|---|---|---|---|
| Launcher | `PDF-Over.exe` (launch4j, GUI) + `PDF-Over-cli.exe` (Konsole) | Platypus-App, die `pdf-over_mac.sh` aufruft | `pdf-over_linux.sh` |
| Installer | MSI (Advanced Installer, manuell) | DMG (manuell: Platypus, Disk Utility) | IzPack-Installer-JAR; nightly `.deb` |
| Dateizuordnung PDF | Ja („Öffnen mit“-Eintrag, kein Default-Handler) | Ja (Platypus: Drop/Open für UTI `com.adobe.pdf`) [I] | **Nein** (keine `MimeType=` in Shortcuts) |
| Kontextmenü | „Senden an → PDF-Over“ (SendTo-Shortcut) | – | – |
| App-Menü | – | About/Preferences/Hide/Quit über `CocoaUIEnhancer` | – |
| Single-Instance | nein | nein | nein |
| Architekturen | x86_64 | x86_64 + aarch64 | x86_64 |

### 4.2 macOS

**App-Menü (`CocoaUIEnhancer`)** [V] `gui/osx/CocoaUIEnhancer.java`:
- Wird aktiviert, wenn `os.name` „mac“ und „OS X“ enthält (`gui/MainWindow.java:203-207,327-348`).
- Greift per Reflection auf **interne** SWT-Cocoa-Klassen zu (`org.eclipse.swt.internal.cocoa.OS/NSMenu/NSMenuItem/NSString/NSApplication`) und hängt mit `class_addMethod` zwei Selektoren an `SWTApplicationDelegate` an (`:177-273`).
- Die Menüpunkte werden per Index im Anwendungsmenü angesprochen: About = 0, Preferences = 2, Hide = 6, Quit = 10 (`:57-61`). About, Quit und Hide werden lokalisiert umbenannt (`main.about/quit/hide`), Preferences wird aktiviert.
- Preferences springt zu `ConfigurationUIState`, wenn der Config-Button aktiv ist. Quit hängt am `SWT.Close`-Listener des Display und ruft `StateMachine.exit()` auf (`:166-169`).
- **Bug** [V] `:105-109`: Die About-Aktion steht innerhalb von `if (log.isDebugEnabled())`. Der About-Dialog erscheint also nur bei DEBUG-Logging.
- Die Klasse steht laut Header unter EPL (`:50-52`). Das ist eine Fremdcode-Lizenz innerhalb des EUPL-Projekts.

**Weitere macOS-Spezifika** [V]:
- Der Startskript `res/scripts/pdf-over_mac.sh:1-4` setzt `LC_CTYPE=UTF-8` und startet `jre/bin/java -XstartOnFirstThread -cp "lib/*" at.asit.pdfover.gui.Main "$@"`. `-XstartOnFirstThread` ist für SWT/Cocoa Pflicht.
- Der App-Name im Menü kommt aus `Display.setAppName("PDF-Over")` (`C:61-64`), die Version aus `Display.setAppVersion` (`gui/MainWindow.java:220`). Das Shell-Icon ist `/icons/icon.png` (`gui/MainWindow.java:237-240`). Das Dock-Icon stammt aus der `icon.icns` des Platypus-Bundles (`release-instructions/release-Readme.md:105-107`). Weitere Dock-Integration (Badge, Menü) gibt es nicht.
- **App-Bundle (manuell)** [V] `release-instructions/release-Readme.md:78-155`:
  1. ZIP entpacken, `chmod +x`.
  2. `sign-jni.sh`: extrahiert `libjnidispatch.jnilib` (x86-64 und aarch64) aus `jna-5.12.1.jar`, signiert sie mit der Developer-ID von A-SIT und packt sie zurück (`res/installer-mac/sign-jni.sh:1-8`).
  3. **Platypus** (GUI): Script `pdf-over_mac.sh`, Name „PDF-Over“, Interface „None“, Identifier `at.a-sit.PDF-Over`, Icon `icon.icns`. „Accepts dropped items“ mit „Accept dropped files“, UTIs nur `com.adobe.pdf`. Bundled Files: `jre`, `lib`, `icons`.
  4. `signscript.sh`: `codesign --timestamp -o runtime` auf `Contents/MacOS/PDF-Over` und das App-Bundle (`res/installer-mac/signscript.sh:7-8`).
  5. DMG manuell mit dem Festplattendienstprogramm (Sparse Bundle 250 MB, Hintergrundbild `res/installer-mac/background.png`, Alias auf Programme), dann Konvertieren in „Compressed“.
  6. `xcrun notarytool submit --wait` mit Apple-ID-Credentials aus den Umgebungsvariablen `ALTOOL_USERNAME`/`ALTOOL_PASSWORD` (`res/installer-mac/notarizeAppCommand.sh:1`, Log-Abruf `checkNotaryLog.sh:1`), dann `xcrun stapler staple`.
- **„Öffnen mit“ / Drop aufs Dock-Icon** [I]: PDF-Over hat **keinen** Handler für `SWT.OpenDocument` bzw. Apple-Events (grep ohne Treffer). Die Dateiübergabe funktioniert nur, weil Platypus die geöffneten/gedroppten Dateien als Argumente an das Skript weiterreicht. Dort landen sie als Eingabedokumente (1.5). Ungeklärt ist das Verhalten beim Öffnen einer weiteren PDF, während die App schon läuft (siehe offene Fragen).
- Es gibt keine eigenen Entitlements. Die JRE-Binärdateien werden nicht separat signiert, nur das Bundle ohne `--deep` [V] `res/installer-mac/signscript.sh`. [I] Die Notarisierung funktioniert vermutlich, weil die Temurin-Binärdateien bereits von Adoptium signiert sind.

### 4.3 Windows

- **launch4j** [V] `pdf-over-gui/pom.xml:374-454`, aktiviert im Profil `windows` (`:623-641`):
  - `PDF-Over.exe` (`headerType gui`) und `PDF-Over-cli.exe` (`headerType console`), jeweils `dontWrapJar=true`.
  - Main `at.asit.pdfover.gui.Main`, Klassenpfad `lib/` + `preCp lib/swt.jar`, `chdir .`.
  - JRE-Pfad `jre`, `bundledJre64Bit`, `minVersion 11` (veraltet, gebündelt ist 21). Versionsinfo kommt aus `build-helper:parse-version`.
  - Im Windows-Profil wird zusätzlich eine Execution `l4j-uninst` referenziert, die in der Basis nicht definiert ist (`:636-639`) [V].
- **MSI (Advanced Installer, manuell)** [V] `res/installer-windows/PDF-Over.aip`, `release-instructions/release-Readme.md:40-76`:
  - Projektdatei Advanced Installer 11.5.1 „professional“ (`PDF-Over.aip:2`). Für die Pflege ist ein **kommerzielles** Tool nötig.
  - `ProductVersion` steht im Repo noch auf `4.4.0` (`:21`); die Dateiliste ist ebenfalls veraltet (4.4.0-JARs, `:279-660`). Die Pflege erfolgt manuell bei jedem Release („Major Upgrade“). Upgrade-Logik über `UpgradeCode` (`:23`, `:796-799`).
  - Installationsort `[ProgramFiles64Folder]PDF-Over` (`:738` `SET_APPDIR`). `ALLUSERS` ist 1, im DefaultBuild 2 (`:14`). [I] Das bedeutet per-machine bei Adminrechten, sonst per-user.
  - Launch-Condition: nur 64-Bit (`:764-768`).
  - **Registry / Dateizuordnung** (`:773-785`; Doku `release-Readme.md:64-72`):
    - `HKCR\.pdf\OpenWithProgids` → `PDF-Over.pdf`
    - `HKCR\PDF-Over.pdf` (Default `PDF-Over`, `DefaultIcon`, `shell\open\command` = `"…\PDF-Over.exe" "%1"`)
    - `HKCR\Applications\PDF-Over.exe` (`FriendlyAppName`, `DefaultIcon`, `shell\open\command`, `SupportedTypes\.pdf`)
    - Produktinfos unter `Software\<Manufacturer>\PDF-Over` (`Path`, `Version`).
    - PDF-Over erscheint damit in „Öffnen mit“, wird aber nicht als Default-Handler gesetzt.
  - **Shortcuts** (`:786-792`): Desktop, Startmenü (Root und Ordner `PDF-Over`), **SendTo** („Senden an → PDF-Over“ ist die Kontextmenü-Integration; bei Mehrfachauswahl kommen mehrere Argumente, die als Warteschlange signiert werden: `cli/InputDocumentArgument.java:57-60`, `gui/workflow/states/OutputState.java:95-97`). Dazu ein Uninstall-Shortcut (`msiexec /x`).
  - Ein eigenes Kontextmenü-Verb („Mit PDF-Over signieren“) gibt es **nicht** [V].
  - Signatur: Advanced Installer `DigCertStoreComponent` mit A-SIT-Code-Signing-Zertifikat aus dem Zertifikatsspeicher, SHA-256, Timestamp `http://timestamp.digicert.com` (`:662-664`). Laut Eintrag war das Zertifikat bis 01/2024 gültig. [I] Es wird pro Release manuell aktualisiert.
- Der Windows-Build kann im CI laufen (launch4j), das MSI entsteht aber **nur manuell** in der Advanced-Installer-GUI [V].

### 4.4 Linux

- **Startskript** [V] `res/scripts/pdf-over_linux.sh:1-3`: `GDK_BACKEND=x11,wayland exec jre/bin/java -cp "lib/*" at.asit.pdfover.gui.Main "$@"`. Bevorzugt wird X11/XWayland.
- **IzPack-Installer** [V] `res/izpack-linux/install.xml`, `pdf-over-gui/pom.xml:473-576`:
  - Das Ergebnis ist ein ausführbares **Installer-JAR** `pdf-over_linux-x86_64.jar`, beim Release umbenannt in `PDF-Over-<v>.jar` (`release-instructions/release-Readme.md:35-37`).
  - [I] Zum Starten des Installers braucht der Nutzer eine **vorinstallierte Java-Laufzeit** (`java -jar` bzw. Doppelklick). `install.xml:11` verlangt `javaversion 1.8`.
  - Panels: Hello, Target, Install, Shortcut, SimpleFinish; Sprachen eng/deu (`install.xml:24-27,41-47`).
  - Installiert `lib/`, `icons/`, die JRE (wird während der Installation aus dem `tar.gz` entpackt, `:62-64`), `pdf-over_linux.sh`, `uninstall_linux.sh` und `ReadMe.txt`. Die Executable-Bits werden gesetzt (`:69-72`).
  - Shortcuts über `unix_shortcutSpec.xml`: Programmgruppe „PDF-Over“ in „applications“ plus Desktop-Shortcut, `terminal=false`, Icon `icons/icon.png`. **Ohne MimeType und ohne `%f`**, also keine Dateizuordnung (`unix_shortcutSpec.xml:8-30`).
  - `ManifestEntries.mf` (`Permissions: all-permissions`, `Codebase: *`) wird nachträglich in das Installer-JAR eingefügt. Das ist ein Relikt aus der Java-Web-Start-Zeit (`pom.xml:265-288`).
  - Deinstallation: `uninstall_linux.sh` ruft `java -jar Uninstaller/uninstaller.jar` mit der **System-Java** auf, nicht mit der gebündelten JRE (`res/scripts/uninstall_linux.sh:1-3`).
- **Nightly-Debian-Paket** [V] `.github/workflows/build-linux-debian.yaml`, `.github/helper/debian-package-builder.sh`:
  - Erzeugt mit `fpm`, Name `pdf-over-nightly`, Version = Zeitstempel, `ARCH=all`, `--depends "java-runtime (>= 21)"`. Es bündelt **keine** JRE (CI-Build).
  - JARs nach `/usr/share/java/pdf-over-nightly`, Launcher `/usr/bin/pdf-over-nightly`, `.desktop` in `/usr/share/applications` (Kategorie Office; wieder ohne MimeType und ohne `%F`), Icon `/usr/share/pixmaps`.
  - Das Paket wird in das GitHub-Repo `a-sit/ppa-nightly` gepusht. Ein Debian-Changelog wird aus den Git-Tags generiert (`.github/helper/debian-changelog-generator.sh`).
  - Auf der Download-Seite wird kein `.deb` angeboten [E].

### 4.5 Plattformübergreifend

- Drag & Drop von PDFs auf den Öffnen-Bereich und in die Ausgabeansicht, Drag & Drop von Bildern in die Logo-Vorschau [V] `gui/composites/DataSourceSelectComposite.java:170-212`, `gui/composites/OutputComposite.java:182-210`, `SCC:297-357`.
- URLs werden über `java.awt.Desktop.browse` geöffnet, Fallback SWT `Program.launch` [V] `gui/utils/SWTUtils.java:259-279`. Das signierte PDF wird über `Desktop.open` bzw. `Program.launch` geöffnet (`gui/composites/OutputComposite.java:136-150`). Unter Linux mit Java ≤ 8 gibt es einen Sonderfall (`:434-453`), der heute toter Code ist.
- Es gibt keinen Single-Instance-Mechanismus (grep `ServerSocket|FileLock` ohne Treffer) [V]. Jedes „Öffnen mit“ startet einen neuen Prozess mit eigener JVM. [I] Gleichzeitig laufende Instanzen schreiben dieselbe `PDF-Over.config`, es gilt „last writer wins“.
- `sun.java2d.uiScale=1` ist gesetzt (AWT in SWT, #106) [V] `gui/Main.java:52-53`.

---

## 5. Build, CI und Release

### 5.1 Build-Struktur

- Maven-Multimodul `pdf-over` (packaging `pom`) mit den Modulen `pdf-over-commons`, `pdf-over-gui`, `pdf-over-signer`; `maven.compiler.release=21`; `pdfbox.version 2.0.34`; `swt.version 3.134.0` [V] `pom.xml:104-117`.
- Repositories [V] `pom.xml:269-289`, `pdf-over-gui/pom.xml:837-847`, `pdf-over-signer/pom.xml:75-85`:
  - lokales `file://…/repo` (IAIK-JARs),
  - `https://apps.egiz.gv.at/maven/` (PDF-AS, `iaik_jce_full 5.63_moa`, `webauthn-client-java`),
  - Maven Central,
  - ein toter `download.java.net/maven/2`.
- Staging in `pdf-over-gui/target/staging/<output-filename>` [V] `pdf-over-gui/pom.xml:11-15`:
  - `maven-jar-plugin`: Manifest mit Classpath und Main-Class (`:155-174`),
  - `copy-dependencies` nach `lib/` (ohne SWT; SWT wird als `lib/swt.jar` kopiert) (`:175-218`),
  - Ressourcen-Filterung für `ReadMe.txt` und das OS-Skript `*${os-name}*` (`:307-373`),
  - `truezip` packt Staging und JRE nach `pdf-over-build/<output-filename>` (ZIP) (`:239-264`),
  - `jarsigner` in der Phase `install` signiert alle JARs im Staging (`:219-238`).
- OS-Profile (je eines ist per OS-Aktivierung aktiv, abschaltbar mit `-Dno-native-profile`) [V] `pdf-over-gui/pom.xml:472-712`:

| Profil | SWT-Artefakt | gebündelte JRE | Output |
|---|---|---|---|
| `linux` | `org.eclipse.swt.gtk.linux.x86_64` | `OpenJDK21U-jre_x64_linux_hotspot_21.0.9_10.tar.gz` | IzPack `pdf-over_linux-x86_64.jar` |
| `windows` | `org.eclipse.swt.win32.win32.x86_64` | `OpenJDK21U-jre_x64_windows_hotspot_21.0.8_9.zip` | `pdf-over_windows-x86_64.zip` + launch4j-EXEs |
| `mac` | `org.eclipse.swt.cocoa.macosx.x86_64` | `OpenJDK21U-jre_x64_mac_hotspot_21.0.8_9.zip` (`…/Contents/Home`) | `pdf-over_mac-x86_64.zip` |
| `mac-aarch64` | `org.eclipse.swt.cocoa.macosx.aarch64` | `OpenJDK21U-jre_aarch64_mac_hotspot_21.0.9_10.zip` | `pdf-over_mac-aarch64.zip` |
| `ci-build` | – | ersetzt die JRE durch `_ci-dummy_<os>.zip` (Wrapper-Skripte `exec java "$@"`), Output `.` (unverpackt), schaltet IzPack ab | `pdf-over-build/` |
| `dummy-sign` / `pkcs11-sign` / `ks-sign` | – | Jar-Signing: Test-Keystore im Repo (Default) / Hardware-Token über den IAIK-PKCS#11-Provider / externer Keystore über `-Dks-file`, `-Dks-pass` | – |

- `release-instructions/publish.sh:54-73` baut nacheinander `linux windows mac mac-aarch64` mit `-Dno-native-profile` auf **einem** Host. Die Anleitung empfiehlt Linux/WSL (`release-Readme.md:23`). Auf diese Weise entstehen alle Bundles cross-plattform, MSI und DMG aber nur manuell auf Windows bzw. macOS.

### 5.2 CI-Matrix (GitHub Actions)

| Workflow | Runner | Trigger | Befehl | Artefakt | Fundstelle |
|---|---|---|---|---|---|
| Build Linux | `ubuntu-latest` | push/PR `main` | `mvn -B clean install -Plinux -Pci-build -Dno-native-profile` | `pdf-over.tar` (ohne JRE) | `.github/workflows/build-linux.yaml:11-42` |
| Build Windows | `windows-latest` | push/PR `main` | `… -Pwindows -Pci-build …` | Verzeichnis `pdf-over-build` (inkl. EXEs, ohne JRE) | `.github/workflows/build-windows.yaml:11-38` |
| Build MacOS (aarch64) | `macos-latest` | push/PR `main` | `… -Pmac-aarch64 -Pci-build …` | `pdf-over.tar` | `.github/workflows/build-mac-aarch64.yaml:11-42` |
| Build MacOS (x86_64) | `macos-latest` | push/PR `main` | `… -Pmac -Pci-build …` | `pdf-over.tar` | `.github/workflows/build-mac-x86_64.yaml:11-42` |
| Build Debian Package | `ubuntu-latest` | push `main`, manuell | Linux-CI-Build + `fpm` | `.deb`, Push nach `a-sit/ppa-nightly` (Secret-Token) | `.github/workflows/build-linux-debian.yaml:9-71` |
| CodeQL | `ubuntu-latest` | push/PR, wöchentlich Di 15:41 | Autobuild Java | – | `.github/workflows/codeql.yml:14-79` |
| nightly.link-Kommentar | `ubuntu-latest` | nach den Build-Workflows bei PRs | kommentiert Download-Links (`nightly.link`) | – | `.github/workflows/nighty-link-pr-comment.yml:1-77` |
| Create Release | `ubuntu-latest` | Tag `pdf-over-*` | `softprops/action-gh-release@v2`, nur generierte Release-Notes, **keine Binärdateien** | – | `.github/workflows/release.yml:1-16` |

Beobachtungen [V]:
- JDK über `actions/setup-java@v3` mit der Distribution `adopt` (veraltet). Viele Actions sind veraltet: `checkout@v3`, `github-script@v6`, `codeql-action@v2`.
- Die CI-Artefakte sind „Lightweight Builds“, die eine **installierte Java 21** voraussetzen.
- Vor jedem Upload werden ältere Artefakte desselben PR/Branch gelöscht.
- In der CI wird **nichts** signiert oder notarisiert, es werden keine Installer gebaut, und es gibt keine Tests außer dem Maven-Lauf: einzige Testklasse `VersionComparatorTest`.
- [I] Der Job „MacOS (x86_64)“ läuft auf einem Apple-Silicon-Runner. Das Profil wählt nur SWT-Artefakt und JRE aus, kompiliert wird plattformneutral.

### 5.3 Gebündelte JREs (Eclipse Temurin 21, „JRE“-Image)

Sie liegen als Binärdateien direkt im Git-Repository (kein LFS, keine `.gitattributes`), zusammen ca. 180 MB [V]:

| Datei | komprimiert | entpackt | Dateien | Fundstelle |
|---|---|---|---|---|
| `OpenJDK21U-jre_x64_windows_hotspot_21.0.8_9.zip` | 48 928 520 B (≈ 46,7 MiB) | 151 245 189 B (≈ 144 MiB) | 378 | `jre-bundles/` |
| `OpenJDK21U-jre_x64_mac_hotspot_21.0.8_9.zip` | 41 156 958 B (≈ 39,3 MiB) | 131 603 225 B (≈ 126 MiB) | 326 | `jre-bundles/` |
| `OpenJDK21U-jre_aarch64_mac_hotspot_21.0.9_10.zip` | 47 059 122 B (≈ 44,9 MiB) | 157 619 210 B (≈ 150 MiB) | 328 | `jre-bundles/` |
| `OpenJDK21U-jre_x64_linux_hotspot_21.0.9_10.tar.gz` | 51 952 734 B (≈ 49,5 MiB) | 164 837 489 B (≈ 157 MiB) | – | `jre-bundles/` |

- Das ist das volle Temurin-JRE ohne `jlink`-Reduktion. Die `release`-Datei listet sämtliche Module inkl. `jdk.compiler`, `jdk.jshell`, `jdk.jpackage` usw. [V] (`jdk-21.0.8+9-jre/release` im Windows-ZIP). `lib/modules` allein umfasst ca. 99,5 MB [V].
- Dass die Windows- und mac-x64-JREs eine ältere Patch-Version (21.0.8) haben als Linux und mac-aarch64 (21.0.9), zeigt die manuelle Pflege [V].

### 5.4 Installer-Größen (aktuelles Release 4.4.9)

[E] HTTP-`Content-Length`, gemessen am 2026-10-14 über die Download-Links von `https://technology.a-sit.at/en/pdf-over/`:

| Datei | Größe |
|---|---|
| `PDF-Over-4.4.9.msi` | 94 774 784 B (≈ 90,4 MiB) |
| `PDF-Over-4.4.9-x86_64.dmg` | 84 786 389 B (≈ 80,9 MiB) |
| `PDF-Over-4.4.9-aarch64.dmg` | 91 028 181 B (≈ 86,8 MiB) |
| `PDF-Over-4.4.9.jar` (Linux, IzPack) | 99 833 965 B (≈ 95,2 MiB) |

[I] Installiert belegt PDF-Over grob 170–200 MB: entpackte JRE (126–157 MiB) plus ca. 25–40 MB JARs plus ca. 2 MB PDF-AS-Config in `~/.pdf-over`. Etwa 50–60 % des Downloads und über 75 % der installierten Größe entfallen auf die JRE.

### 5.5 Signierung und Notarisierung

| Plattform | Was wird signiert | Wie | Automatisiert? | Fundstelle |
|---|---|---|---|---|
| alle | JARs | `maven-jarsigner` (Profile dummy/pkcs11/ks) | im lokalen Build | `pdf-over-gui/pom.xml:219-238,759-834` |
| Windows | MSI (und laut Option ggf. enthaltene Dateien [I]) | Advanced Installer, A-SIT-Code-Signing-Zertifikat (DigiCert), SHA-256, Timestamp DigiCert | **manuell** | `res/installer-windows/PDF-Over.aip:662-664`; `release-Readme.md:57-60` |
| macOS | JNA-JNI-Libs, App-Bundle | `codesign` Developer ID, Hardened Runtime, Timestamp; `notarytool`, `stapler` | **manuell** (Skripte plus GUI-Schritte) | `res/installer-mac/*.sh`; `release-Readme.md:97-155` |
| Linux | – | keine Paketsignatur (`.deb` unsigniert, Installer-JAR jarsigniert) | – | `.github/helper/debian-package-builder.sh` |

### 5.6 Release-Ablauf

[V] `release-instructions/release-Readme.md`:
1. JRE-Bundles aktuell halten (manuell).
2. `mvn release:prepare` (Tag `pdf-over-x.y.z`, nächste `-SNAPSHOT`). Dafür muss der Branch-Schutz kurz deaktiviert werden. Der Tag-Push löst `release.yml` aus (GitHub-Release ohne Assets).
3. Release-Tag auschecken und `publish.sh` mit Signier-Keystore ausführen, ergibt die vier Bundles.
4. Plattform-Nachbearbeitung: Linux nur umbenennen; Windows MSI in Advanced Installer bauen; macOS Platypus, Signatur, DMG, Notarisierung (siehe 4.2/4.3).
5. Die Installer per WordPress-Medienupload auf `technology.a-sit.at` veröffentlichen, SHA-256 in die DE- und EN-Seiten eintragen, Download-Buttons aktualisieren (`:158-164`).
6. Release auf Joinup anlegen (`:166-179`).
7. **Clients benachrichtigen:** per SFTP `/html/pdf-over/Release.txt` auf `updates.a-sit.at` auf die neue Versionsnummer setzen (`:181-184`). Ab dann sehen alle Installationen den Update-Hinweis.

### 5.7 Legacy-Artefakte

`pdf-over-gui/src/main/jnlp/pdf-over.jnlp` (Java Web Start, Java 1.6+, IzPack-Installer je OS) ist obsolet [V] `:1-40`. Ebenso `ManifestEntries.mf` und `izpack javaversion 1.8`.

---

## 6. Abhängigkeiten und Lizenzen

### 6.1 Direkte und wichtige transitive Abhängigkeiten

| Komponente | Version | Lizenz | Quelle im Repo | Relevanz für einen Nicht-Java-Klon |
|---|---|---|---|---|
| PDF-Over selbst | 4.4.10-SNAPSHOT | **EUPL-1.2** (`LICENSE`); Datei-Header: „EUPL 1.1 oder spätere Versionen“ | `LICENSE:1-2`, `pom.xml:84-90`, z. B. `CM:4-8` [V] | Lizenz der Vorlage, siehe 6.3 |
| PDF-AS (`pdf-as-lib`, `sigs-pades`, `pdf-as-pdfbox-2`) | 4.4.2 | EUPL (im Repo `license/EUPL v.1.1 - Licence.pdf`; [E] Joinup nennt EUPL-1.2 für 4.4.2) | `pdf-over-signer/pom.xml:13-30`; `pdf-as-4/license/` [V] | Java-Bibliothek, nicht direkt nutzbar. Profile, Logik und Texte als EUPL-Vorlage nutzbar |
| IAIK-JCE `iaik_jce_full` | 5.63_moa | **proprietär** (Stiftung SIC/IAIK), siehe 6.2 | `pdf-over-signer/pom.xml:31-35` (EGIZ-Repo) [V] | entfällt |
| IAIK ECCelerate, ECCelerate-CMS, IAIK-CMS | 3.01, 3.01, 2.15 (PDF-AS selbst nutzt 6.02/6.02/5.1.1) | proprietär; Manifest „**evaluation version**“ | `repo/iaik/**`, `pdf-over-signer/pom.xml:36-50`; `pdf-as-4/pdf-as-lib/build.gradle:77-80` [V] | entfällt |
| Eclipse SWT | 3.134.0 | EPL-2.0 [I] | `pom.xml:116` | entfällt (GUI-Toolkit) |
| Apache PDFBox | 2.0.34 | Apache-2.0 [I] | `pom.xml:115,245-259` | entfällt; Ersatz z. B. pdf-lib, lopdf, PDFium, MuPDF (Achtung: AGPL) |
| Logback / SLF4J | 1.5.18 / 2.0.9 | EPL-1.0 + LGPL-2.1 / MIT [I] | `pdf-over-gui/pom.xml:23-27`; `pdf-over-commons/pom.xml:13-18` | entfällt |
| Apache HttpClient 5 / commons-httpclient 3.1 | 5.5.1 / 3.1 (EOL) | Apache-2.0 [I] | `pom.xml:210-219` | entfällt |
| commons-io, commons-codec, commons-lang, commons-configuration 1.10 | 2.18.0, 1.15, 2.6, 1.10 | Apache-2.0 [I] | `pom.xml:200-229`; `pdf-over-gui/pom.xml:60-64` | entfällt |
| gson, org.json, jsoup | 2.12.1, 20250107, 1.19.1 | Apache-2.0, Public Domain [I], MIT [I] | `pdf-over-gui/pom.xml:45-59` | entfällt |
| metadata-extractor | 2.18.0 | Apache-2.0 [I] | `pdf-over-commons/pom.xml:25-28` (EXIF-Rotation des Logos: `pdf-over-commons/src/main/java/at/asit/pdfover/commons/utils/ImageUtil.java:105`) | Funktion übernehmen (EXIF-Orientierung) |
| `at.a-sit:webauthn-client-java` | 0.0.1 | **unbekannt** (nur im EGIZ-Repo) | `pdf-over-gui/pom.xml:71-75` | entfällt; Ersatz durch native WebAuthn-APIs bzw. libfido2 |
| JNA (über webauthn-client) | 5.12.1 | Apache-2.0 / LGPL-2.1 [I] | `res/installer-mac/sign-jni.sh:2` | entfällt |
| Transitiv über PDF-AS: BouncyCastle `bcprov`, ZXing, jose4j, JAXB, OGNL, commons-collections4 | 1.70, 3.5.0, 0.9.6, … | MIT, Apache-2.0, Apache-2.0, EDL/BSD, Apache-2.0, Apache-2.0 [I] | `pdf-as-4/pdf-as-lib/build.gradle:63-84` | entfällt |
| Lombok | 1.18.30/36 | MIT [I] | nur `provided` | entfällt |
| Eclipse Temurin JRE | 21.0.8/21.0.9 | GPL-2.0 mit Classpath Exception [I] | `jre-bundles/` | entfällt (Hauptgewinn bei der Größe) |
| DejaVu-Fonts | – | Bitstream-Vera-basierte freie Lizenz | `cfgzip/fonts/`, `cfgzip/licenses/fonts/BitStream.txt` [V] | übernehmbar, falls Fonts für den Signaturblock gebraucht werden |
| `arial.ttf` | – | [I] proprietär (Monotype), keine Lizenzdatei beigelegt, von keinem Profil referenziert | `cfgzip/fonts/arial.ttf` [V] | **nicht** übernehmen |
| Signatur-Bildmarken `signatur-logo_*.png`, `amtsignatur-logo_demo.png` | – | ungeklärt (amtliche Bildmarke bzw. „demo“) | `cfgzip/images/` [V] | Nutzungsrecht klären |
| Build-Tools: launch4j, IzPack, Advanced Installer, Platypus | – | BSD/MIT bzw. Apache-2.0, **kommerziell**, BSD [I] | `pdf-over-gui/pom.xml:374-377,543-545`; `.aip` | nur als Vorbild |
| `CocoaUIEnhancer` | – | EPL (laut Header) | `gui/osx/CocoaUIEnhancer.java:50-52` [V] | entfällt |

### 6.2 IAIK-Bibliotheken

- Die Manifeste der im Repo eingecheckten JARs tragen „evaluation version“: `iaik_cms` (Implementation-Version „5.0 evaluation version“ trotz Dateiname 2.15) und `iaik_eccelerate` „3.01 evaluation version“ [V] (`unzip -p … META-INF/MANIFEST.MF`). PDF-AS 4.4.2 bringt `iaik_jce_full-5.63_moa` („moa version“), `iaik_cms-5.1.1` („5.101 moa version“) und `iaik_eccelerate-6.02` („6.02 evaluation version“) mit [V] `pdf-as-4/pdf-as-lib/libs/`.
- Der About-Text von PDF-Over weist darauf hin, dass Komponenten eigene Lizenzen haben und deren freie Nutzung teils auf EUPL-lizenzierte Software beschränkt ist [V] `res/at/asit/pdfover/gui/messages.properties:115`.
- [E] Die IAIK-Toolkits sind proprietär (Stiftung SIC). Kostenfreie Lizenzen gibt es u. a. für Open-Source-Entwicklung und Evaluierung nach der SIC-Lizenz. Die IAIK-MOA-Lizenz erlaubt die Laufzeitnutzung nur zusammen mit unveränderten MOA-Modulen und verbietet die Weitergabe als eigenes Toolkit. Für einen **Nicht-Java-Klon ist IAIK technisch irrelevant**, Kryptografie liefern dort plattformeigene oder freie Bibliotheken (z. B. OS-Krypto-APIs, RustCrypto/ring, WebCrypto). Ein Klon sollte keinerlei IAIK-Artefakte übernehmen.

### 6.3 Implikationen für einen EUPL-Klon

[I] Das ist keine Rechtsberatung. Vor einer Veröffentlichung sollte eine juristische Prüfung erfolgen.
1. **Abgeleitetes Werk.** Die EUPL überlässt die Frage, wann ein „Derivative Work“ vorliegt, dem anwendbaren Urheberrecht (`LICENSE:26-30`). Eine Übersetzung des Codes in eine andere Sprache gilt üblicherweise als Bearbeitung. Ein Klon, der Logik, Struktur, Texte (`messages*.properties`), Profile (`cfg/profiles/*.properties`) oder Ressourcen übernimmt, sollte deshalb selbst unter **EUPL-1.2** stehen oder unter einer kompatiblen Lizenz aus dem Anhang: GPL-2/3, AGPL-3, OSL-2.1/3.0, EPL-1.0, CeCILL-2.0/2.1, MPL-2.0, LGPL-2.1/3, EUPL-1.1/1.2, LiLiQ-R/R+ (`LICENSE:115-130,264-283`) [V].
2. **Clean-Room-Ansatz.** Wird nur das *Verhalten* (Protokolle, Dateiformate, Workflow) nachgebaut und kein Code oder Text übernommen, ist eine freie Lizenzwahl eher möglich. Für Kompatibilität mit bestehenden Konfigurationen genügt die Kenntnis des Formats (Kapitel 1).
3. **Netzwerknutzung.** Die EUPL zählt auch das Bereitstellen von Funktionalität über ein Netz zur „Distribution/Communication“. Eine gehostete Web-/PWA-Variante des Klons löst also die Pflicht zur Quellcodebereitstellung aus.
4. **App Stores (iOS/Android).** Copyleft-Lizenzen und Store-Nutzungsbedingungen können kollidieren (bekannte GPL-Debatte). Ob die EUPL im Apple App Store unproblematisch ist, ist offen. Für den Rechteinhaber selbst besteht die Option einer Doppel- oder Ausnahmelizenzierung.
5. **Marken und Inhalte.** Der Name „PDF-Over“, die A-SIT-Logos, die ID-Austria-Bildmarke `res/img/id_austria.png` und die amtlichen Signaturlogos können eigenen Rechten unterliegen. Der Klon sollte einen eigenen Namen und eigene Icons verwenden, sofern er nicht vom Rechteinhaber stammt. Die Rechtsbelehrungs-Texte der Standard-Signaturhinweise (eIDAS Art. 25, § 20 E-GovG) sind Gesetzeszitate und frei verwendbar [I].
6. **Fonts.** `arial.ttf` nicht mitliefern. Die DejaVu-Fonts können mit der Lizenzdatei übernommen werden.
7. **Drittanbieter-Endpunkte.** Die Nutzung der A-Trust-Endpunkte (ID Austria) ist eine Frage der Dienstnutzung und nicht der Lizenz. Sie wird in den anderen Analysedokumenten behandelt.

---

## 7. Was ein Klon übernehmen sollte und was neu zu lösen ist

### 7.1 Übernehmen (Verhalten bzw. Kompatibilität)

| Aspekt | Begründung | Bezug |
|---|---|---|
| Semantik **aller** Einstellungen aus Tabelle 1.5 inkl. Defaults und Sonderregeln | Nutzererwartung, Migration | 1.5 |
| **Import** von `~/.pdf-over/PDF-Over.config` (Properties-Parser, ISO-8859-1) | Bestandsnutzer:innen | 1.3, 1.9 |
| Overlay-Prinzip: CLI-Argumente überschreiben temporär, werden nie persistiert; CLI-Flags kompatibel halten (`-i -o -b -n -p -ks -kstype -kspass -ksalias -kskeypass -proxy* -e -a -s -c -v`), nackte Pfade als Eingabedokumente, mehrere Dokumente als Warteschlange | Skripte, Integrationen, „Öffnen mit“/„Senden an“ | 1.5, 4.5 |
| Ausgabe-Logik: Ordner/Datei/`.`, Postfix `_signed`, Überschreib-Dialog, Skip-Finish | Workflow-Kompatibilität | `gui/composites/OutputComposite.java:308-366` |
| Signaturprofile und PDF-AS-Profil-Mapping inkl. DE/EN, `_NOTE`, `_PDFA`, Logo-Größe in mm | Optisch gleicher Signaturblock | 1.6, `cfgzip/cfg/profiles/` |
| Datenschutzhinweis (Upload des PDFs zu A-Trust), erweitert um den Update-Check | Transparenz, DSGVO | 3.4 |
| Rotierendes Datei-Log mit „Logs anzeigen“, Level per Datei erhöhbar | Support | 3.2 |
| Factory-Reset mit Backup statt Löschen | Robustheit | 1.8 |
| Versionsvergleich nach dem Algorithmus von 3.3 (falls Versionen im selben Schema) | Konsistenz | 3.3 |
| Integration „Öffnen mit“ und „Senden an“ (Windows), PDF-UTI (macOS), Desktop-Eintrag (Linux) | Power-User-Workflow | 4 |

### 7.2 Neu bzw. besser lösen

| Thema | PDF-Over heute | Empfehlung für den Klon |
|---|---|---|
| **Installationsgröße** | 81–95 MiB Download, ca. 170–200 MB installiert, davon ≥ 75 % JRE (5.3/5.4) | Runtime nicht bündeln: native Binary mit System-WebView (z. B. Tauri: WebView2/WKWebView/WebKitGTK) oder AOT-Framework (z. B. Flutter). [I] Erwartbar sind etwa 5–40 MB je nach Technologie. Electron bringt keinen Größenvorteil. |
| **Linux-Installer braucht Java** | IzPack-JAR [I] | AppImage und/oder Flatpak und/oder `.deb`/`.rpm`, jeweils ohne Java |
| **Konfigurationsort** | `~/.pdf-over` auf allen OS | Plattformübliche Orte: Windows `%APPDATA%\<App>`, macOS `~/Library/Application Support/<bundle-id>`, Linux `$XDG_CONFIG_HOME/<app>` (Logs nach `$XDG_STATE_HOME` bzw. `~/Library/Logs`), Mobile im App-Sandbox-Container. Einmalige Migration aus `~/.pdf-over` (1.9) |
| **Konfigurationsformat** | Properties, Latin-1, unversioniert, unbekannte Keys gehen verloren | Versioniertes JSON/TOML mit Schema-Migrationen; unbekannte Keys erhalten |
| **Secrets** | Klartext (`KEYSTORE_*PASS`, `PROXY_PASS`) | OS-Keychain: Windows Credential Manager/DPAPI, macOS/iOS Keychain, Linux Secret Service, Android Keystore. Option „temporär“ beibehalten |
| **PDF-AS-Profile/-Config** | In das Nutzerverzeichnis entpackt; Update überschreibt alles, nur Backup-ZIP; harte Schwelle `4.4.6`; Laufzeit-Schreibzugriff (`signature_field_name`) | Profile als **read-only Ressourcen** im App-Paket, Nutzer-Overrides getrennt speichern; Laufzeitzustand (gewähltes Signaturfeld) nicht in Ressourcendateien persistieren |
| **Update** | Nur Hinweis per `Release.txt` ohne Statuscode-Prüfung und Signatur, doppelter Request, ignoriert den konfigurierten Proxy | Signierter Update-Manifest (z. B. Ed25519/minisign), echte Auto-Updates: Windows MSIX/App Installer oder Updater-Framework, macOS Sparkle-artig (EdDSA), Linux über Paketquelle/Flatpak/AppImageUpdate; Mobile ausschließlich über die Stores. Opt-out und Datenschutzhinweis beibehalten |
| **Proxy** | Nur manuell, erst beim Signieren aktiv, ohne PAC/WPAD; Benutzer/Passwort ohne UI | Standardmäßig System-Proxy (inkl. PAC), manueller Override, für **alle** Requests (auch Update-Check); Auth über Keychain |
| **TLS** | JRE-cacerts, tote Zertifikate im Repo | OS-Truststore (plattformeigener Verifier), keine Legacy-Zertifikate; lokale BKU weiterhin `http://127.0.0.1:3495` (Protokollvorgabe) |
| **Datei-Öffnen-Events / Single-Instance** | Keine Apple-Events, keine Instanz-Weitergabe, jede Datei startet einen neuen Prozess | Single-Instance mit Weitergabe der Pfade an die laufende Instanz (Windows Named Pipe/Mutex, macOS `application:openURLs:`/Apple-Event, Linux D-Bus); Warteschlange für Mehrfachdateien |
| **Linux-Dateizuordnung** | fehlt | `.desktop` mit `MimeType=application/pdf;` und `Exec=… %F` |
| **Windows-Kontextmenü** | nur „Senden an“ | Zusätzlich ein Explorer-Verb „Mit … signieren“ für `.pdf` (per-user unter `HKCU\Software\Classes`, damit keine Adminrechte nötig sind). [I] Unter Windows 11 erscheint ein klassisches Verb nur unter „Weitere Optionen“; für das neue Menü braucht es ein `IExplorerCommand` mit Package Identity |
| **macOS-Menü** | Reflection auf interne SWT-API, About-Bug | Native Menüs des gewählten Frameworks |
| **Build/Release** | Viel manuelle Arbeit (Advanced-Installer-GUI, Platypus, Disk Utility), JREs im Git, Signieren lokal | Vollautomatische CI-Pipeline: Matrix Windows x64/ARM64, macOS Universal, Linux x64/ARM64, Android, iOS; Code-Signing und Notarisierung in der CI (Secrets in der CI, z. B. Cloud-HSM-Signing für Windows); Assets direkt am GitHub-Release mit Prüfsummen und SBOM; die Versionsdatei wird automatisch veröffentlicht |
| **Architekturen** | Windows und Linux nur x86_64 | zusätzlich ARM64 |
| **Bekannte Bugs nicht übernehmen** | Doppelter Update-Request; kein HTTP-Statuscheck; About nur bei DEBUG; Speichern fehlt bei Quit/`-s`; ungültiger `PROXY_PORT` blockiert den Start; fehlende `-c`-Datei führt zum Reset-Dialog; doppelter Key `error.Initialization` (DE); vertauschte Hilfetexte `-kspass`/`-kskeypass` (EN); `-v true` macht unsichtbar | korrigieren, mit Regressionstests |
| **Mobile (iOS/Android)** | nicht vorhanden | Kein Home-Verzeichnis und keine CLI: Konfiguration im App-Container, Eingang über Share-Sheet/„Öffnen in“ (iOS `CFBundleDocumentTypes`/Share Extension; Android Intent-Filter `ACTION_VIEW`/`ACTION_SEND` `application/pdf`); Ausgabe über Dokument-Picker/SAF statt Ausgabeordner. **Lokale BKU entfällt** (kein `127.0.0.1:3495`-Dienst auf Mobilgeräten [I]); Keystore nur per Datei-Import (`.p12`) in die Sandbox; Proxy über das System; Update-Check über die Stores; FIDO2 über Plattform-APIs, Machbarkeit siehe ID-Austria-Analyse |

### 7.3 Prioritäten für die Umsetzung (Vorschlag)

1. Konfig-Modell und Properties-Import (Kap. 1), Testfälle aus Tabelle 1.5.
2. Ressourcen: Profile, Fonts und Logos als read-only Assets mit Lizenzprüfung (6.1/6.3).
3. Plattformintegration Desktop: Öffnen-mit, Single-Instance, Kontextmenü.
4. Paketierung und signierte Auto-Updates in der CI.
5. Mobile-spezifische Ein- und Ausgabe.

---

## 8. Offene Fragen

1. Wie verhält sich das Platypus-Bundle beim Öffnen einer zweiten PDF, während PDF-Over bereits läuft (neuer Prozess, ignoriert, oder Übergabe an das laufende Skript)? Im Code ist es nicht klärbar.
2. Welche Lizenz hat `at.a-sit:webauthn-client-java` 0.0.1? Sie ist nur im EGIZ-Maven-Repo, nicht lokal verfügbar.
3. Welche genauen Nutzungsbedingungen gelten für die eingecheckten IAIK-„evaluation version“-JARs, und welche Versionen landen nach Maven-„nearest wins“ tatsächlich im Release (3.01/2.15 aus `repo/` vs. 6.02/5.1.1 aus PDF-AS)? Für den Klon ist das irrelevant, für die Dokumentation von PDF-Over aber nicht.
4. Dürfen die Signatur-Bildmarken (`signatur-logo_de/en.png`, `amtsignatur-logo_demo.png`) und `id_austria.png` in einem Klon verwendet werden?
5. Soll der Klon den Update-Endpoint `updates.a-sit.at/pdf-over/Release.txt` mitnutzen (nur bei Herausgabe durch A-SIT sinnvoll) oder einen eigenen, signierten Manifest-Endpoint bekommen?
6. Ist die EUPL-1.2 mit den App-Store-Bedingungen (Apple/Google) vereinbar, oder ist eine Doppel-/Ausnahmelizenz durch den Rechteinhaber nötig?
7. Soll der Klon Konfigurationsänderungen an `~/.pdf-over/cfg/` (manuell angepasste PDF-AS-Profile) importieren, oder nur `PDF-Over.config`?
8. Welche Rolle spielt die lokale BKU (Bürgerkartenumgebung, `127.0.0.1:3495`) künftig noch? Davon hängt ab, ob sie im Desktop-Klon Priorität hat.
9. Sollen gleichzeitig laufende Instanzen unterstützt werden, oder wird Single-Instance erzwungen? Das betrifft die Konsistenz der Konfiguration.
10. Werden Legacy-Keys aus sehr alten PDF-Over-Versionen gebraucht? Wegen des Shallow Clone ohne Historie war das hier nicht ermittelbar.

---

## 9. Quellen

**Code und Dateien (Auswahl):**
- `pdf-over-commons/src/main/java/at/asit/pdfover/commons/{Constants,Messages,BKUs,Profile}.java`
- `pdf-over-commons/src/main/resources/version.properties`
- `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/config/{ConfigurationManager,ConfigurationDataInMemory}.java`
- `pdf-over-gui/src/main/java/at/asit/pdfover/gui/workflow/states/{PrepareConfigurationState,ConfigurationUIState,PrepareSigningState,OpenState,OutputState,KSState,MobileBKUState}.java`
- `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/ConfigurationComposite.java`, `composites/configuration/*`
- `pdf-over-gui/src/main/java/at/asit/pdfover/gui/{Main,MainWindow}.java`, `osx/CocoaUIEnhancer.java`, `utils/{UpdateCheckManager,VersionComparator,Zipper,HttpClientUtils,LocaleSerializer,SWTUtils}.java`, `cliarguments/*`
- `pdf-over-gui/src/main/resources/{logback.xml,ReadMe.txt,scripts/*,installer-mac/*,installer-windows/PDF-Over.aip,izpack-linux/*,certificates/*,cfg/PDFASConfig.zip}`, `at/asit/pdfover/gui/messages*.properties`
- `pom.xml`, `pdf-over-gui/pom.xml`, `pdf-over-signer/pom.xml`, `pdf-over-commons/pom.xml`
- `.github/workflows/*`, `.github/helper/*`, `release-instructions/*`, `jre-bundles/*`, `repo/iaik/**`, `Readme.md`, `LICENSE`
- `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/{api/PdfAsFactory,settings/Settings}.java`, `pdf-as-4/pdf-as-lib/build.gradle`, `pdf-as-4/pdf-as-lib/libs/*`

**Extern [E]:**
- https://technology.a-sit.at/en/pdf-over/ (Version 4.4.9, Downloads; Größen per HTTP-HEAD gemessen am 2026-10-14)
- https://updates.a-sit.at/pdf-over/Release.txt (Inhalt `4.4.9`, gemessen am 2026-10-14)
- https://git.egiz.gv.at/moa-sig/plain/licenses/IAIK-LICENSE.txt?h=3.2.0
- https://git.egiz.gv.at/moa-sig/plain/licenses/SIC_LICENSE.txt?h=3.2.0 (nur über die Suchzusammenfassung, Abruf fehlgeschlagen)
- https://en.wikipedia.org/wiki/IAIK-JCE
