### PDF-Over NG – user interface texts, English (EUPL-1.2)
###
### Own IDs of the user interface (`ng-ui-*`), complementing crates/i18n/locales/en/pdfover.ftl and
### pdfover-ng.ftl. The integration moves them to pdfover-ng.ftl (ADR 0001 rule 11); every ID has an
### entry in de/ and en/ with the same variables (checked in src/i18n/i18n.test.ts).

## General

ng-ui-yes = Yes
    .accesskey = Y
ng-ui-no = No
    .accesskey = N
ng-ui-close = Close
ng-ui-continue = Continue
    .accesskey = C
ng-ui-skip-to-content = Skip to content
ng-ui-link-external = (opens in browser)
ng-ui-page-n = Page { $page }
ng-ui-page-previous = Previous page
ng-ui-page-next = Next page
ng-ui-unit = Unit
ng-ui-starting = PDF-Over NG is starting
ng-ui-finished = PDF-Over NG has exited.
ng-ui-finished-code = Exited with code { $code }.
ng-ui-mainbar-label = Steps
ng-ui-step-done = done
ng-ui-lang-system = System language
ng-ui-lang-de = German
ng-ui-lang-en = English
ng-ui-drop-not-here = Documents can be dropped in the “Open” step or on the result.

## Errors of the user interface or the shell

ng-ui-core-unavailable = The signature core could not be started. Please try again; if the error persists, check the logs (Settings › About).
ng-ui-event-invalid = This action is not yet supported by this version of the signature core.
ng-ui-bridge-error = The connection to the signature core is disrupted.
ng-ui-error-unknown = Unexpected error ({ $code }).
ng-ui-open-failed = Opening failed.
ng-ui-dialog-failed = The file dialog could not be opened.

## Open

ng-ui-open-title = Open document
ng-ui-open-analyzing = Checking document …
ng-ui-dropzone-label = Drop area for PDF documents
ng-ui-dropzone-hint = Several documents are signed one after the other. Keyboard: Ctrl+O or ⌘O opens the file dialog.
ng-ui-dropzone-release = Release to open the documents
ng-ui-queue-title = Queue ({ $count })
ng-ui-queue-current = current

## Placeholder

ng-ui-placeholder-title = Signature placeholder found
ng-ui-placeholder-qr = QR code placeholder on page { $page }
ng-ui-placeholder-field = Signature field “{ $name }”
ng-ui-placeholder-kind = Kind of placeholders

## Positioning

ng-ui-position-title = Position the signature
ng-ui-position-help = Drag the signature block to the desired place or use the arrow keys (Shift: 1 pt, Ctrl/⌘: 50 pt), Page Up/Down for the page and Enter to sign.
ng-ui-position-block-role = movable signature block
ng-ui-position-announce = Page { $page } of { $pages }, { $left } mm from the left, { $bottom } mm from the bottom
ng-ui-position-pages = Pages
ng-ui-position-page-alt = Page { $page } of { $pages } – { $name }
ng-ui-position-new-page-alt = New blank page
ng-ui-position-preview-failed = The page preview is not available.
ng-ui-position-coordinates = Position
ng-ui-position-x = X from the left ({ $unit })
ng-ui-position-y = Y top edge from the bottom ({ $unit })
ng-ui-position-page-field = Page
ng-ui-position-auto = Automatic
    .accesskey = A
ng-ui-position-auto-done = The signature block was placed automatically.
ng-ui-position-invisible = Sign invisibly
    .accesskey = I
ng-ui-position-signed-hint = The document is already signed; a new page cannot be added.

## Signing method

ng-ui-backend-title = Choose signing method
ng-ui-backend-hint = A default signing method can be set in the settings (Advanced).
ng-ui-backend-atrust-desc = Sign with ID Austria: confirm in the app, by SMS TAN or with a FIDO2 security key.
ng-ui-backend-local-desc = Sign with the citizen card via a locally installed citizen card environment.
ng-ui-backend-keystore-desc = Sign with a PKCS#12 file (keystore).
ng-ui-backend-unavailable = Currently not available.
ng-ui-backend-local-unavailable = No local citizen card environment found.
ng-ui-backend-keystore-unavailable = Keystore not configured (Settings › Keystore).
ng-ui-backend-leaves-device = The document is sent to A-Trust for signing.
ng-ui-backend-qes = Qualified signature
ng-ui-factor-app = App
ng-ui-factor-sms = SMS
ng-ui-factor-card = Card
ng-ui-privacy-title = Privacy
ng-ui-privacy-lead = Please read the data protection notice before the document is sent.
ng-ui-privacy-accept = Agree and continue
    .accesskey = A
ng-ui-privacy-decline = Decline
    .accesskey = D

## Signing

ng-ui-progress-connecting = Connecting …
ng-ui-progress-certificate = Reading certificate …
ng-ui-progress-server = Waiting for the signature service …
ng-ui-progress-document = Signing document { $index } of { $total } …
ng-ui-progress-done = Signature created.
ng-ui-sms-title = Enter SMS TAN
ng-ui-qr-title = Scan QR code
ng-ui-qr-alt = QR code to scan with the “ID Austria” app
ng-ui-app-title = Confirm in the app
ng-ui-fido2-title = FIDO2 security key
ng-ui-fido2-hint = Connect your security key and follow the instructions of the system (enter PIN, touch the key).
ng-ui-fido2-failed = FIDO2 failed: { $detail }
ng-ui-countdown = Time remaining: { $time }
ng-ui-countdown-expired = The time has expired.
ng-ui-login-rejected = The login was rejected. Please enter the password again.
ng-ui-username-required = Please enter the username or phone number.
ng-ui-secret-title = Password
ng-ui-secret-prompt = Please enter the password:
ng-ui-secret-retry = The password is wrong.
ng-ui-secret-subject = For: { $subject }
ng-ui-proxy-password-prompt = Please enter the proxy password:
ng-ui-proxy-password-retry = The proxy password is wrong.
ng-ui-notice-keystore-conversion = JKS and JCEKS keystores are not supported. Please convert the keystore to PKCS#12 (e.g. with keytool -importkeystore).

## Result

ng-ui-output-reveal = Show in folder
    .accesskey = F
ng-ui-output-saving = Saving the signed document …
ng-ui-output-next = Next document: { $name }
ng-ui-output-next-button = Sign next document
    .accesskey = N
ng-ui-output-more = Open more documents
    .accesskey = M
ng-ui-output-drop-hint = You can drop more documents here.
ng-ui-output-saved-next = { $file } was saved, continuing with the next document.

## Settings

ng-ui-settings-interface = Interface
ng-ui-settings-reset = Reset settings …
    .accesskey = t
ng-ui-settings-reset-confirm = All settings are reset to their defaults; the previous ones are kept as a backup.
ng-ui-settings-reset-confirm-button = Reset
    .accesskey = R
ng-ui-settings-preview-alt = Preview of the signature block
ng-ui-settings-preview-note = The preview shows the saved settings.
ng-ui-emblem-none = No emblem selected
ng-ui-emblem-drop-hint = You can drag an image file (PNG, JPEG, GIF) onto the preview.
ng-ui-theme = Appearance
ng-ui-theme-system = Same as system
ng-ui-theme-light = Light
ng-ui-theme-dark = Dark
ng-ui-cms-profile = Signature format (keystore)
ng-ui-cms-profile-hint = Applies to signatures created locally.
ng-ui-cms-pdfas = Compatible with PDF-AS
ng-ui-cms-pades = PAdES-BASELINE-B
ng-ui-postfix-hint = Empty means “_signed”.
ng-ui-proxy-mode = Use proxy
ng-ui-proxy-system = System proxy
ng-ui-proxy-none = No proxy
ng-ui-proxy-manual = Manual
ng-ui-proxy-password-hint = The password is requested when needed and stored in the keychain.
ng-ui-certcache = Cache the signing certificate
ng-ui-certcache-hint = Stores only the public certificate so that the preview shows the real signature block.
ng-ui-keystore-keychain = Store in the operating system keychain
ng-ui-keystore-alias-hint = Leave empty if the keystore contains only one key.
ng-ui-problem-required = Please fill in this field.
ng-ui-problem-port = The port must be between 1 and 65535.
ng-ui-problem-invalid = The value is invalid.
ng-ui-about-version = Version { $version }
ng-ui-about-thirdparty = The license notices of the components used are included in the program package.
