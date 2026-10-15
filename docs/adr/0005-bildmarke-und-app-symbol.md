# ADR 0005 – Siegel als Standard-Bildmarke und App-Symbol

| | |
|---|---|
| **Status** | angenommen (15.10.2026) |
| **Kontext** | Die kleinen Standardblöcke und „nur Bildmarke“ (`BASE_LOGO`) von PDF-Over zeigen ohne eigene Bildmarke `signatur-logo_de/en.png` aus der PDF-AS-Konfiguration; ob ein Klon diese Bildmarken verwenden darf, ist offen (docs/analyse/05 §8 Frage 4, Plan Q12/E-05: „sonst neutrale Grafiken“). Das App-Symbol war ein Platzhalter (blaues Dokument, `apps/app/src-tauri/icons/app-icon.svg`). Der Prototyp `prototypes/pida-workspace` zeigt im Signaturblock ein eigenes, neutrales Siegel (`DefaultEmblem` in `src/ui/Logo.tsx`). |
| **Entscheidung** | Auftraggeber, 15.10.2026: Das Siegel aus dem Prototyp wird (1) die neue Standard-Bildmarke im sichtbaren Signaturblock und (2) – davon abgeleitet – das App-Symbol für iOS und Android, einheitlich auch für macOS, Windows, Linux und das Favicon. |
| **Betrifft** | `pida-sigblock` (Asset, Profil-Vorgabe, Tests), Golden-/Pixeltests, `pida-pades`-Tests (PDF/A), `apps/app/src-tauri/icons/**`, `gen/apple/Assets.xcassets`, `gen/android/…/res/{mipmap-*,values}`, `design/brand/**` |

## 1. Entscheidung

1. **Siegel = Standard-Bildmarke der sichtbaren Standardblöcke.** Für die acht Profile
   `SIGNATURBLOCK_SMALL_{DE,EN}[_NOTE][_PDFA]` und für `BASE_LOGO` („nur Bildmarke“, unter „Weitere
   Profile“, ADR 0004) ist ohne eigene Bildmarke das PIDA-Siegel (`crates/sigblock/assets/images/pida-seal.png`)
   die Bildmarke – statt `signatur-logo_de/en.png`. Wie im Prototyp (`SignatureBlock.tsx`: ohne eigenes Logo
   `DefaultEmblem`) zeigt PIDA damit ohne eigene Bildmarke überall dasselbe Zeichen.
   - **Unverändert:** Amtssignatur (`AMTSSIGNATURBLOCK_*`, `amtsignatur-logo_demo.png` – eine Behörde setzt dort
     ohnehin ihre eigene Bildmarke) und alle Basisprofile.
   - **Vorrang wie bisher:** Eine eigene Bildmarke (Einstellung `signature.emblem`, CLI `-e`, PDF-Over-Overlay
     `value.SIG_LABEL`) geht vor (`SigBlockInput::emblem` → `ProfileOverrides::sig_label` → Profilwert).
   - **Eigene PDF-AS-Konfiguration** (`ProfileStore::load_dir`, z. B. migrierte Profile aus `~/.pdf-over/cfg`)
     behält ihre eigenen Bildmarken; das Siegel ist eine Vorgabe der eingebetteten Konfiguration.
2. **App-Symbol = abgeleitetes Siegel** auf vollflächigem Grund in der Tinte der Oberfläche, mit Fassungen für
   alle Plattformen (§3.2). Master und Generator liegen unter `design/brand/`.
3. **Nur eigene Zeichen.** Siegel und App-Symbol sind eigene Werke des Projekts (EUPL-1.2), ohne Hoheitszeichen
   und ohne fremde Marken (§4).

## 2. Dateien

| Datei | Inhalt |
|---|---|
| `design/brand/pida-seal.svg` | **Master Siegel** (viewBox 40 × 40), exakt `DefaultEmblem` |
| `design/brand/pida-seal-1024.png` | Siegel 1024 × 1024, transparent |
| `design/brand/pida-seal-sigblock.png` = `crates/sigblock/assets/images/pida-seal.png` | Rasterfassung für den Signaturblock, 652 × 684 px RGBA (§3.1) |
| `design/brand/pida-app-icon.svg` | **Master App-Symbol** (1024 × 1024); die Gruppe zwischen `glyph:start`/`glyph:end` ist das Zeichen |
| `design/brand/pida-app-icon-1024.png`, `pida-app-icon-play-512.png` | App-Symbol deckend: 1024 px ohne Alphakanal (App Store über den Asset-Katalog), 512 px für den Google-Play-Eintrag als 32-Bit-PNG (RGBA, überall deckend – Vorgabe der Play Console) |
| `design/brand/variants/*.svg` | erzeugte Varianten: iOS dunkel/getönt, Android Vordergrund/monochrom/klassisch/rund, macOS, Windows/Linux |
| `design/brand/favicon.svg` | Favicon (abgerundetes Quadrat, Zeichen 112 %) – **übernimmt der Orchestrator** nach `apps/app/public/app-icon.svg` (Favicon und Kopfzeile, `index.html`, `App.tsx`) |
| `design/brand/preview.png` | Vorschaubogen: Siegel im echten Signaturblock, App-Symbol in 1024/180/120/87/58/40/29 px hell/dunkel, Android-Masken, monochrom, iOS dunkel/getönt, Desktop |
| `design/brand/tools/build.mjs` (+ `package.json`, `package-lock.json`) | Generator aller abgeleiteten Dateien (`@resvg/resvg-js` 2.6.2, `pngjs` 7.0.0, nur dort; Desktop über die Tauri-CLI der App) |
| `design/brand/tools/render-block.sh`, `render_block.py` | Vorschaubogen: signiert `testdata/pdf/pdfas/pdf-as-legacy/simple.pdf` mit `pida-cli` (Test-Keystore, `PIDA_HOME` im Temp-Ordner), rendert das Widget mit PDFium (pypdfium2) |
| `apps/app/src-tauri/icons/*` | Desktop: `icon.icns` (macOS-Raster), `icon.ico`, `icon.png`, `32x32`, `64x64`, `128x128[@2x]`, `Square*Logo`, `StoreLogo` |
| `apps/app/src-tauri/icons/{ios,android}/**` | Symbolordner der Tauri-CLI (iOS in allen Einzelgrößen, Android wie in `gen/`) |
| `apps/app/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/` | eine Größe 1024 (Xcode erzeugt die übrigen): `AppIcon-1024.png` (hell, deckend), `-dark.png` (transparent), `-tinted.png` (Graustufen) |
| `apps/app/src-tauri/gen/android/app/src/main/res/` | `mipmap-anydpi-v26/ic_launcher{,_round}.xml` (adaptiv + `<monochrome>`), `mipmap-*dpi/ic_launcher{,_round,_foreground,_monochrome}.png`, `values/ic_launcher_background.xml` (`#16191F`) |

Entfernt: `apps/app/src-tauri/icons/app-icon.svg` (Platzhalter) und die alten Einzelgrößen im iOS-Asset-Katalog.

Neu erzeugen (Repo-Wurzel): `npm --prefix design/brand/tools ci`, dann
`design/brand/tools/render-block.sh` (alles samt Vorschaubogen) oder `node design/brand/tools/build.mjs` (ohne
Vorschaubogen). `build.mjs` erwartet die Tauri-CLI unter `apps/app/node_modules` (`npm --prefix apps/app ci`)
oder `--tauri <tauri.js>`. **Nicht** `tauri icon` direkt auf die App anwenden: es überschreibt die
Mobile-Fassungen mit Tauri-Vorgaben (weißer Android-Hintergrund, keine monochrome Ebene, iOS ohne
dunkel/getönt). Der Test `seal_asset_matches_design_master` hält Asset und Generatorausgabe gleich.

## 3. Herleitung

### 3.1 Siegel im Signaturblock

- **Geometrie und Farben** exakt wie `DefaultEmblem`: weißer Kreis r 17,5 mit Ring `#3a414d` (1,6),
  gepunkteter Innenring r 13,6 (`#3a414d`, 0,7, Strich 1,2/1,4), Häkchen `#1f2630` (2,6, runde Enden),
  rot-weiß-roter Streifen `#c8102e` 10 × (1,3 + 1,1 + 1,3).
- **652 × 684 px = 2 × `signatur-logo_de/en.png` (326 × 342):** Die Bildhöhe in der Zelle ist
  Spaltenbreite · Höhe/Breite (PDF-AS `TableFactory`), die gezeichnete Größe `floor(min(w/W, h/H) · W|H)`;
  mit gleichem Seitenverhältnis und dem Faktor 2 (Zweierpotenz) sind diese Gleitkomma-Rechnungen bitgleich, so
  bleiben Zeilenhöhen und Blockmaße für **jede** Blockbreite gleich – auch bei Platzhaltern mit anderer Breite.
  Das Siegel (652 × 652) liegt mittig, oben und unten 16 px transparent – genau wie `DefaultEmblem` (viewBox
  40 × 40, `xMidYMid meet`) in dieser Fläche; ein resvg-Render des Prototyp-SVG ist pixelgleich. Auflösung:
  Standardbreite 230 pt (Bild 33 pt) ≈ 1400 dpi, 495 pt (Bild 82 pt) ≈ 570 dpi, „nur Bildmarke“ 23 mm
  ≈ 750 dpi, beim Höchstwert 120 mm ≈ 145 dpi (mit 326 px wären es 710, 290, 380 bzw. 72 dpi).
  Die doppelte Auflösung kostet je Signatur ≈ 17 KB (signierte Testdatei 64 KB statt 47 KB, mit dem
  PDF-AS-Logo 66 KB). Die Emblem-Grenze 480 × 600 gilt nur für eigene Bildmarken (`Emblem::decode`), nicht
  für die eingebetteten Profilbilder (`decode_original`, wie PDF-AS). Nachweis
  `tests/seal.rs::block_dimensions_unchanged_against_pdfas_logo`: Für alle acht kleinen Profile, sieben Breiten
  von 90 bis 595 pt, kurze/lange Namen, mit/ohne Hinweis, und für `BASE_LOGO` mit dem PDF-Over-Overlay (5 bis
  120 mm, mit/ohne PDF/A) ist der Content-Stream des Inner-Forms byte-gleich zu dem mit PDF-AS-Logo.
- **Transparenz:** außerhalb des Kreises Alpha 0 (voll transparente Pixel tragen Weiß). Die bestehende
  Emblem-Pipeline schreibt eine `SMask` (ohne PDF/A und PDF/A-2/3, ICCBased-sRGB) bzw. komponiert für
  PDF/A-1 auf Weiß. Verwirft ein Programm die Transparenz (PDF-AS `removeAlphaChannel`, `pdfas_quirks`), bleibt
  der Grund trotzdem weiß. veraPDF (PDF/A-1b, -2b, -3b) bleibt ohne neue Regelverletzung
  (`crates/pades/tests/verapdf.rs`, Varianten `small-pdfa`, `small-note-pdfa`).
- **Dateigröße:** flache Farben komprimieren gut; die signierte Testdatei ist mit Siegel 64 KB, mit dem
  PDF-AS-Logo als Bildmarke 66 KB.
- **Gestaltung wie im Prototyp:** Der gepunktete Innenring (r 13,6) läuft unter dem Akzent durch (Akzent
  y 30,2–33,9, Ring bis 33,95); ein Strichrest von 0,05 Einheiten ist erst bei starker Vergrößerung unter dem
  unteren roten Streifen zu sehen. Bewusst so belassen (pixelgleich zu `DefaultEmblem`); eine Korrektur wäre
  eine Änderung am Prototyp-Zeichen.
- **Umsetzung:** Die PDF-AS-Profildateien bleiben byte-gleich zum Auslieferungsstand (Herkunft nachvollziehbar).
  `ProfileStore::builtin()` setzt beim Laden für die neun Profile `value.SIG_LABEL = ./images/pida-seal.png` als
  eigenen Schlüssel (vor der Vererbung, ersetzt den aus `BASE_NORMAL[_EN]` geerbten bzw. bei `BASE_LOGO` den
  eigenen Wert);
  `ProfileStore::builtin_pdfas()` liefert die Konfiguration ohne diese Vorgabe. Konstanten
  `pida_sigblock::assets::{PIDA_SEAL, PIDA_SEAL_PROFILES}`. Golden-Vergleiche gegen PDF-AS
  (`tests/pdfas442`) verwenden `builtin_pdfas()`, `tests/golden.rs` und `tests/render.rs` übergeben das
  PDF-AS-Logo wie bisher ausdrücklich als Bildmarke. Kompatibilitätsregister in `crates/sigblock/src/lib.rs`.
- Der Alternativtext `sigLogoAltText` („Abgebildet ist eine Standard-Signaturbildmarke.“) passt unverändert.
  Das Häkchen ist Schmuck und für alle Signaturen gleich; es sagt nichts über die Gültigkeit aus (dafür
  steht im Block die Prüfinformation).

### 3.2 App-Symbol

| Element | Siegel (40er-Raster) | App-Symbol (1024 px) |
|---|---|---|
| Grund | transparent, weißer Kreis | vollflächig `#16191f` (Tinte der Oberfläche: `--c-primary`, `--c-logo-bg`) |
| Ring | r 17,5, Strich 1,6 `#3a414d` | r 304, Strich 64, weiß; Außenmaß Ø 672 = 65,6 % |
| gepunkteter Innenring | ja | **entfällt** (bei 29–60 px nicht lesbar) |
| Häkchen | Strich 2,6 | Strich 80, weiß, Spannweite ≈ 342 px (63 % des Innenkreises statt 54 %) |
| Akzent | 10 × 3,7 (27 % des Ring-Ø) | 260 × 78 px (28 + 22 + 28; 39 % des Ring-Ø), weißer Mittelstreifen sichtbar |

Kontrast auf `#16191f`: Weiß 17,6 : 1, Rot 3,0 : 1 (vom weißen Mittelstreifen getrennt). Inhalt optisch
mittig (Abstand Häkchen–Ring oben = Akzent–Ring unten ≈ 82 px).

| Plattform | Fassung |
|---|---|
| iOS / iPadOS | Asset-Katalog mit **einer** Größe 1024 (iOS 16 als Mindestziel, Xcode erzeugt die Größen). Hell: vollflächig, deckend, **ohne Alphakanal**, ohne eigene Rundung (System-Maske). iOS 18+: **dunkel** = Zeichen in Farbe auf transparentem Grund (das System legt den dunklen Verlauf darunter), **getönt** = Graustufen auf Schwarz (Ring, Häkchen, zwei Akzentbalken weiß, Mittelstreifen frei). Geprüft mit `xcrun actool` (Xcode 27: keine Warnung, `Assets.car` mit `UIAppearanceDark` und `ISAppearanceTintable`). |
| Android | Adaptiv (minSdk 28): Hintergrund Farbe `#16191F`, Vordergrund 108 dp mit dem Zeichen auf 72 % (Ring-Außenmaß ≈ 51 dp, innerhalb der Safe Zone Ø 66 dp und der sichtbaren 72 dp – auch für Kreis, Squircle, Tropfen), **monochrome Ebene** für Themen-Symbole (Android 13+): Zeichen weiß, Mittelstreifen des Akzents frei. `ic_launcher_round.xml` ebenfalls adaptiv. Klassische PNGs (48 dp) nur als Rückfall. Geprüft mit `aapt2 compile/link` gegen android-36. |
| macOS | `icon.icns` im macOS-Raster: abgerundetes Quadrat 824 × 824 auf 1024 (Rand 100, Radius 185,4) |
| Windows / Linux | `icon.ico`, PNG-Größen, `Square*Logo`: abgerundetes Quadrat 960 × 960 (Radius 200) |
| Favicon | abgerundetes Quadrat vollflächig (Radius 224), Zeichen 112 % für 16–32 px |

## 4. Abgrenzung zu amtlichen Symbolen und fremden Marken

- **Kein Hoheitszeichen:** kein Bundesadler, kein Bundes- oder Landeswappen, keine Nachbildung davon
  (Wappengesetz). Rot-weiß-rot erscheint nur als schmaler **Farbakzent**: drei Streifen ungleicher Stärke
  (1,3 : 1,1 : 1,3 bzw. 28 : 22 : 28) in einem Band 2,7 : 1 (Siegel) bzw. 3,3 : 1 (App-Symbol) – nicht die
  Flagge (2 : 3, gleiche Drittel), nicht freistehend, immer zusammen mit Ring und Häkchen.
- **Keine Amtssignatur-Bildmarke:** Das Siegel steht nur in den Standardblöcken und in „nur Bildmarke“. Die Bildmarke einer
  Amtssignatur (E-GovG § 19) ist die der Behörde; die Amtssignatur-Profile bleiben unverändert und werden
  mit der eigenen Bildmarke der Behörde verwendet.
- **Keine fremden Marken:** keine Zeichen von ID Austria, A-Trust, A-SIT, PDF-Over, PDF-AS oder Bund
  (`gv.at`); keine Anlehnung an das Logo „ID Austria“. Ring und Häkchen sind allgemeine Formen.
- **Kein Prüfsiegel:** Das Zeichen bescheinigt nichts (keine Zertifizierung, kein „geprüft“); siehe §3.1.
- Die bisherigen PDF-AS-Bildmarken bleiben nur in der Amtssignatur-Demo (die Behörde setzt ihre eigene
  Bildmarke) und in `builtin_pdfas()` für den Golden-Abgleich.

## 5. Offene Punkte

- **Favicon/Kopfzeile:** `apps/app/public/app-icon.svg` (Favicon in `index.html`, Bild in `App.tsx`) zeigt noch den
  Platzhalter; der Orchestrator ersetzt die Datei nach dem Merge von Etappe 1 durch `design/brand/favicon.svg`.
- **Wortmarke:** Die Oberfläche verwendet weiter die Wortmarke des Prototyps (`LogoMark`, Blatt mit
  Eselsohr). Ob sie an das App-Symbol angeglichen wird, ist eine eigene Entscheidung.
- **Rechtliche Prüfung** von Name und Zeichen (Plan Q12/E-05, Markenrecherche) steht aus.
- **macOS 26 / iOS 26:** Für die Darstellung mit „Liquid Glass“ wäre zusätzlich eine Icon-Composer-Datei
  (`.icon`) möglich; der Asset-Katalog bleibt gültig.
- Ein erneutes `tauri ios init` / `tauri android init` erzeugt die Vorlagen unter `gen/` neu (iOS dann wieder
  als Tauri-Vorlage ohne dunkel/getönt); danach `build.mjs` erneut ausführen.
