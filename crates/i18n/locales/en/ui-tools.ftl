### PIDA – texts of the new interface, UI-B (Einstellungen, Prüfmodus, Befehlspalette, Kürzel, Ersteinrichtung, Zuletzt signiert), English (EUPL-1.2)
###
### Maintained by hand (ADR 0004 §4.5). IDs: `ng-ui-<area>-<name>`, areas of this file:
### set, vf, pal, sh, ob, recent. All catalogs of a language form one Fluent bundle: do not reuse an ID from
### `pida.ftl`, `pdfover.ftl` or the other `ui-*.ftl`; every ID with the same variables in `de/`
### and `en/` (test `crates/i18n/tests/catalogs.rs`). Plurals via selectors.
###
### Texts from the prototype (`prototypes/pida-workspace/src/i18n/en.ts`), adapted to the core. The rows of the
### settings carry search synonyms as attribute `.keywords` (space separated, lower case).

## Settings: frame, search, sections (ADR 0004 §3.11)

ng-ui-set-title = Settings
ng-ui-set-instant = Changes apply immediately.
ng-ui-set-search = Search settings
ng-ui-set-search-clear = Clear search
ng-ui-set-search-empty = No setting found for “{ $q }”.
ng-ui-set-search-hits =
    { $n ->
        [0] No results
        [one] One result
       *[other] { $n } results
    }
ng-ui-set-sections = Sections
ng-ui-set-sec-signature = Signature block
ng-ui-set-sec-document = Document
ng-ui-set-sec-signers = Signing methods
ng-ui-set-sec-save = Saving
ng-ui-set-sec-appearance = Appearance
ng-ui-set-sec-privacy = Privacy
ng-ui-set-sec-advanced = Advanced
ng-ui-set-sec-about = About PIDA
ng-ui-set-policy = Set by your organisation.

## Settings › Signature block (live editor)

ng-ui-set-preview = Preview
    .keywords = signature block appearance preview look
ng-ui-set-preview-alt = Preview of the signature block: { $profile }, { $lang }
ng-ui-set-preview-alt-logo = Preview of the signature block: { $profile }
ng-ui-set-preview-invisible = Invisible signature – no signature block appears on the document.
ng-ui-set-profile = Profile
    .keywords = profile official signature authority standard invisible
ng-ui-set-profile-small = Standard (small)
ng-ui-set-profile-small-desc = Signatory, date, verification note
ng-ui-set-profile-amt = Official signature
ng-ui-set-profile-amt-desc = For authorities, with issuer and serial number
ng-ui-set-profile-logo = Emblem only
ng-ui-set-profile-logo-desc = Just the emblem, without text
ng-ui-set-profile-invisible = Invisible
ng-ui-set-profile-invisible-desc = Signature without a visible signature block
ng-ui-set-profile-more = More profiles
ng-ui-set-block-lang = Signature block language
    .keywords = language block language german english deutsch
ng-ui-set-lang-system = As system
ng-ui-set-lang-de = Deutsch
ng-ui-set-lang-en = English
ng-ui-set-emblem = Emblem
    .keywords = logo emblem image picture seal graphic company logo
ng-ui-set-emblem-default = Default emblem (PIDA seal)
ng-ui-set-emblem-current = Current: { $name }
ng-ui-set-emblem-hint = Drop an image here · PNG, JPEG or GIF. The image stays on this device.
ng-ui-set-emblem-choose = Choose file …
ng-ui-set-emblem-reset = Use default
ng-ui-set-emblem-dialog = Choose emblem
ng-ui-set-logo-size = Emblem size
    .keywords = logo size emblem centimetres
ng-ui-set-note = Note text
    .keywords = note text eidas legal effect
ng-ui-set-note-hint = Leave empty for a block without a note.
ng-ui-set-note-reset = Restore default text

## Settings › Document (PDF/A, placeholder search like PDF-Over USE_MARKER/USE_FIELDS)

ng-ui-set-pdfa = PDF/A-compatible signature
    .keywords = pdf/a pdfa archive archiving long-term compliant iso 19005
ng-ui-set-pdfa-hint = PIDA detects PDF/A documents automatically and signs them PDF/A-compliant. This option enforces it for documents without a PDF/A identifier, too.
ng-ui-set-placeholder = Search for signature placeholders automatically
    .keywords = placeholder signature placeholder qr qr code signature field signature fields field form acroform marker automatic
ng-ui-set-placeholder-hint = When a document is opened, a placeholder found in it determines where the signature goes.
ng-ui-set-placeholder-kind = Look for
ng-ui-set-placeholder-qr = QR code placeholders
ng-ui-set-placeholder-qr-desc = The signature goes on the QR code in the document.
ng-ui-set-placeholder-field = Empty signature fields
ng-ui-set-placeholder-field-desc = The signature goes into an empty field of the PDF form.

## Settings › Signing methods

ng-ui-set-default = Default signing method
    .keywords = default signing method preselection id austria keystore smart card citizen ask
ng-ui-set-signer-ida = ID Austria
ng-ui-set-signer-ida-desc = Confirm in the app, with a FIDO2 key or by SMS TAN
ng-ui-set-signer-ida-desc-mobile = Confirm in the “ID Austria” app or by SMS TAN
ng-ui-set-signer-keystore = Keystore
ng-ui-set-signer-keystore-desc = Certificate file (PKCS#12) on this device
ng-ui-set-signer-keystore-off = Set up the keystore below first.
ng-ui-set-signer-card = Smart card
ng-ui-set-signer-card-desc = Card in a reader with local smart card software
ng-ui-set-signer-ask = Ask every time
ng-ui-set-signer-ask-desc = Choose the signing method for each document
ng-ui-set-ida-user = ID Austria: phone number or user name
    .keywords = id austria phone phone number mobile cell user user name account login
ng-ui-set-ida-remember = Remember user name
ng-ui-set-ida-remember-hint = Only the user name is stored, never the password.
ng-ui-set-fido2 = Prefer FIDO2 security key
    .keywords = fido2 fido security key key yubikey token
ng-ui-set-fido2-hint = Skips the app confirmation when A-Trust offers FIDO2.
ng-ui-set-keystore = Keystore
    .keywords = keystore pkcs#12 pkcs12 p12 pfx certificate certificate file alias key
ng-ui-set-keystore-enabled = Use keystore
ng-ui-set-keystore-file = Keystore file
ng-ui-set-keystore-none = No file chosen
ng-ui-set-keystore-choose = Choose file …
ng-ui-set-keystore-type = Type
ng-ui-set-keystore-alias = Alias
ng-ui-set-keystore-storage = Password
ng-ui-set-keystore-storage-none = Don’t store
ng-ui-set-keystore-storage-memory = Until exit
ng-ui-set-keystore-storage-keychain = Keychain
ng-ui-set-card = Smart card
    .keywords = smart card card reader card reader bku smart card software citizen
ng-ui-set-card-status = Smart card software
ng-ui-set-card-found = Detected
ng-ui-set-card-missing = Not found

## Settings › Saving

ng-ui-set-target = Location
    .keywords = folder location output output folder target save
ng-ui-set-target-beside = Next to the original
ng-ui-set-target-beside-desc = In the same folder as the document
ng-ui-set-target-folder = In a fixed folder
ng-ui-set-target-ask = Ask every time
ng-ui-set-target-ask-desc = Choose the location after signing
ng-ui-set-folder = Folder
ng-ui-set-folder-none = No folder chosen yet
ng-ui-set-folder-choose = Choose folder …
ng-ui-set-postfix = File name suffix
    .keywords = file name name suffix postfix ending signed pattern
ng-ui-set-postfix-hint = Appended to the document’s name. Empty means “_signed”.
ng-ui-set-example = Example

## Settings › Appearance

ng-ui-set-theme = Colour scheme
    .keywords = colour color scheme dark light mode night theme
ng-ui-set-theme-system = System
ng-ui-set-theme-light = Light
ng-ui-set-theme-dark = Dark
ng-ui-set-ui-lang = Interface language
    .keywords = language interface german english deutsch

## Settings › Privacy (ADR 0003, ADR 0004 §3.10)

ng-ui-set-privacy-info = Privacy information
    .keywords = privacy data protection a-trust upload transfer gdpr
ng-ui-set-certcache = Cache signing certificate
    .keywords = certificate certificate cache cache store
ng-ui-set-recent = Remember “Recently signed”
    .keywords = recently signed recent history list
ng-ui-set-recent-hint = Only name, folder and time of the most recently signed documents (at most 10) are stored, on this device only.
ng-ui-set-recent-count =
    { $n ->
        [0] The list is empty.
        [one] One entry in the list.
       *[other] { $n } entries in the list.
    }
ng-ui-set-recent-clear = Clear list
ng-ui-set-recent-cleared = The “Recently signed” list has been cleared.

## Settings › Advanced

ng-ui-set-proxy = Proxy
    .keywords = proxy network host port internet firewall
ng-ui-set-proxy-host = Host
ng-ui-set-proxy-port = Port
ng-ui-set-proxy-user = User (optional)
ng-ui-set-updates = Check for updates automatically
    .keywords = update updates version
ng-ui-set-cms = Signature format (keystore)
    .keywords = signature format format cms pades pdf-as keystore
ng-ui-set-reset = Reset
    .keywords = reset default defaults factory settings

## Settings › About

ng-ui-set-about = PIDA
    .keywords = about version licence license licenses third-party
ng-ui-set-logs = Logs
    .keywords = log logs logging troubleshooting

## Recently signed (start view, ADR 0004 §3.10)

ng-ui-recent-title = Recently signed
ng-ui-recent-empty = No signed documents yet.
ng-ui-recent-open = Open “{ $name }” in verification mode
ng-ui-recent-remove = Remove “{ $name }” from the list
ng-ui-recent-missing = File no longer available
ng-ui-recent-verify-later = Verification mode will follow in one of the next versions.
