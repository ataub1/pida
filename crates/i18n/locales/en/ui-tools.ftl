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
# “Automatic positioning” like PDF-Over (SIGNATURE_POSITION=auto, ADR 0004 addendum 8)
ng-ui-set-auto-position = Automatic positioning
    .keywords = position positioning place placement automatic auto signature block location immediately sign
ng-ui-set-auto-position-hint = When a document is opened, the signature block is placed automatically and the document is ready to sign right away. You can still change the position if needed.

## Settings › Signing methods

ng-ui-set-default = Default signing method
    .keywords = default signing method preselection id austria keystore smart card citizen ask
ng-ui-set-signer-ida = ID Austria
ng-ui-set-signer-ida-desc = Confirm in the app, with a FIDO2 key or by SMS TAN
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
ng-ui-set-keystore-alias = Alias
ng-ui-set-keystore-storage = Password
ng-ui-set-keystore-storage-none = Don’t store
ng-ui-set-keystore-storage-memory = Until exit
ng-ui-set-keystore-storage-keychain = Keychain
ng-ui-set-keystore-format = Format: { $format }
ng-ui-set-keystore-unsupported = This file is not a supported keystore. Please choose a PKCS#12 file (.p12/.pfx).
ng-ui-set-keystore-key = Key
ng-ui-set-keystore-key-show = Show keys …
ng-ui-set-keystore-key-lead = The keys in the keystore are protected by its password. Enter it to show them.
ng-ui-set-keystore-password = Keystore password
ng-ui-set-keystore-password-empty = Please enter the password.
ng-ui-set-keystore-wrong-password = The password is incorrect. Please try again.
ng-ui-set-keystore-key-submit = Show
ng-ui-set-keystore-key-cancel = Cancel
ng-ui-set-keystore-key-entry = { $name } · valid until { $date }
ng-ui-set-keystore-key-alias = Alias: { $alias }
ng-ui-set-keystore-key-choose = Choose a key
ng-ui-set-keystore-key-several = The keystore contains several keys. Please choose the key for signing.
ng-ui-set-keystore-key-none = This file contains no key for signing.
ng-ui-set-keystore-alias-missing = The saved key “{ $alias }” is not in this keystore. Please choose a key.
ng-ui-set-keystore-key-use = Use this key
ng-ui-set-ks-remember = Remember password?
ng-ui-set-ks-remember-none = Don’t store
ng-ui-set-ks-remember-memory = Until exit
ng-ui-set-ks-remember-keychain = In the keychain
ng-ui-set-ks-remember-hint = You can change this later under Settings › Signing methods.
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
    .keywords = signature format format cms pades pdf-as keystore
ng-ui-set-reset = Reset
    .keywords = reset default defaults factory settings
ng-ui-set-migration = Import PDF-Over settings
    .keywords = pdf-over migration import legacy old settings first run setup
ng-ui-set-migration-hint = Looks for the settings of an earlier PDF-Over installation on this device and offers to import them.
ng-ui-set-migration-search = Search …

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

## Verification mode (ADR 0004 §3.9; wording §8 F3: PIDA never says “valid” or “legally valid”)

ng-ui-vf-title = Verification mode
ng-ui-vf-preview = Pages of the verified document
ng-ui-vf-page = Page { $n } of { $total }
ng-ui-vf-meta = { $pages } · { $revisions }
ng-ui-vf-pages =
    { $n ->
        [one] one page
       *[other] { $n } pages
    }
ng-ui-vf-revisions =
    { $n ->
        [one] one revision
       *[other] { $n } revisions
    }
ng-ui-vf-loading = Checking signatures …
ng-ui-vf-failed = The file could not be checked
ng-ui-vf-summary-none = This document contains no signature
ng-ui-vf-summary-none-text = There is nothing to check. You can sign the document.
ng-ui-vf-summary-intact =
    { $n ->
        [one] The signature is unchanged and cryptographically intact
       *[other] All { $n } signatures are unchanged and cryptographically intact
    }
ng-ui-vf-summary-intact-text = The document has not been changed since the last signature.
ng-ui-vf-summary-broken =
    { $n ->
        [one] Warning: one signature is not intact
       *[other] Warning: { $n } signatures are not intact
    }
ng-ui-vf-summary-broken-text = The document was changed after signing or a signature is damaged. Use the signed original file or request a new signed version.
ng-ui-vf-summary-unverifiable = Not all signatures could be checked
ng-ui-vf-summary-unverifiable-text = No signature is broken, but at least one uses a method that PIDA cannot check.
ng-ui-vf-summary-modified = Added to or changed after the last signature
ng-ui-vf-summary-modified-text = All signatures are intact, but the file was added to or changed after the last signature. That part is not covered by any signature.
ng-ui-vf-scope = PIDA checks on this device whether the document is unchanged since signing and shows the details of the certificates. PIDA does not check the chain of trust or revocation status.
ng-ui-vf-full-check = Full verification at signaturpruefung.gv.at
ng-ui-vf-signatures = Signatures
ng-ui-vf-unknown-signer = Signature { $n } (signatory unknown)
ng-ui-vf-status-intact = Intact
ng-ui-vf-status-broken = Changed
ng-ui-vf-status-unverifiable = Cannot be checked
ng-ui-vf-time = Signed on { $time } (as stated by the signatory)
ng-ui-vf-time-none = No signing time stated
ng-ui-vf-qes = Qualified electronic signature according to the certificate
ng-ui-vf-qc = Qualified certificate without a qualified signature creation device (according to the certificate)
ng-ui-vf-not-qualified = Not qualified (according to the certificate)
ng-ui-vf-qual-unknown = Qualification cannot be determined
ng-ui-vf-qual-hint = Taken from the certificate, not checked against the EU trusted lists.
ng-ui-vf-whole = Document unchanged since this signature
ng-ui-vf-revision = Unchanged as of this signature
ng-ui-vf-later =
    Then { $revisions ->
        [one] one more revision
       *[other] { $revisions } more revisions
    }, { $signatures ->
        [0] without another signature
        [one] one of them with a signature
       *[other] { $signatures } of them with a signature
    }
ng-ui-vf-broken = Document changed after this signature or signature damaged
ng-ui-vf-unverifiable = PIDA cannot check this signature
ng-ui-vf-time-outside = The stated time is outside the validity period of the certificate.
ng-ui-vf-details = Certificate details
ng-ui-vf-cert-org = Organisation
ng-ui-vf-cert-issuer = Issuer
ng-ui-vf-cert-serial = Serial number
ng-ui-vf-cert-validity = Validity period
ng-ui-vf-cert-range = { $from } to { $to }
ng-ui-vf-cert-algorithm = Method
ng-ui-vf-cert-format = Format
ng-ui-vf-cert-qualification = Qualification
ng-ui-vf-cert-field = Signature field
ng-ui-vf-cert-appearance = Signature image
ng-ui-vf-cert-page = Page { $n }
ng-ui-vf-cert-invisible = invisible
ng-ui-vf-cert-trust = Chain of trust
ng-ui-vf-cert-trust-not-checked = Not checked
ng-ui-vf-cert-code = Technical code
ng-ui-vf-cert-unknown = unknown
ng-ui-vf-format-pades = PAdES (ETSI.CAdES.detached)
ng-ui-vf-format-pkcs7 = PKCS#7 (adbe.pkcs7.detached)
ng-ui-vf-format-pkcs7-sha1 = PKCS#7 with SHA-1 (adbe.pkcs7.sha1)
ng-ui-vf-format-other = Other format
ng-ui-vf-show = Show on page
ng-ui-vf-shown = Signature image of { $name } highlighted on page { $page }.
ng-ui-vf-sign = Sign this document
ng-ui-vf-other = Check another document
ng-ui-vf-exit = Leave verification mode

## Command palette (⌘/Ctrl+K; the commands are in ui-workspace.ftl, area shell)

ng-ui-pal-title = Commands
ng-ui-pal-search = Search commands …
ng-ui-pal-empty = No results
ng-ui-pal-group-actions = Actions
ng-ui-pal-group-view = View
ng-ui-pal-group-help = Help

## Keyboard shortcuts (“?”)

ng-ui-sh-title = Keyboard shortcuts
ng-ui-sh-group-general = General
ng-ui-sh-group-workspace = Workspace
ng-ui-sh-palette = Command palette

## First run (ADR 0004 §3.12; the PIDA seal stays, ADR 0005)

ng-ui-ob-title = Welcome to PIDA
ng-ui-ob-progress = Step { $n } of { $total }
ng-ui-ob-offer-title = Import PDF-Over settings
ng-ui-ob-import-title = PDF-Over settings found
ng-ui-ob-import-text = This device has settings from an earlier PDF-Over installation. Do you want to import them? The old file stays unchanged.
ng-ui-ob-import-list = Will be imported:
ng-ui-ob-import-item = { $item }: { $detail }
ng-ui-ob-import-seal = Emblem: the PIDA seal stays.
ng-ui-ob-import-accept = Import
ng-ui-ob-import-decline = Don’t import
ng-ui-ob-signer-title = What do you usually sign with?
ng-ui-ob-signer-text = This is preselected for every document. You can switch at any time.
ng-ui-ob-signer-keystore-off = Set it up later in the settings.
ng-ui-ob-block-title = This is what your signature looks like
ng-ui-ob-block-text = The signature image appears on the document. Profile, emblem and note can be changed later in the settings.
ng-ui-ob-back = Back
ng-ui-ob-next = Next
ng-ui-ob-finish = Let’s go
ng-ui-ob-later = Set up later
