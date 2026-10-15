### PDF-Over NG – own texts, English (EUPL-1.2)
###
### Maintained by hand. Every ID needs an entry in `de/` and `en/` with the same variables
### (test `crates/i18n/tests/code_catalog.rs`). Two kinds of IDs:
###
### - Own texts (package E and UI): prefix `ng-`, so they never collide with the IDs generated from
###   PDF-Over (`pdfover.ftl`).
### - Stable codes of the crates (ADR 0001 rule 11, plan §7.5): the ID is the code itself
###   (`cms-der`, `backend-network`, `fido2-timeout` …). Codes with a dot (`render.failed`,
###   `placement.notice.clamped`) are not valid in Fluent; their ID is derived like the PDF-Over keys
###   with `pdfover_i18n::fluent_id_for_java_key` (dot → hyphen: `render-failed`,
###   `placement-notice-clamped`).
###
### Variables are named after the fields of the error variant (`$detail` for free text). The test
### checks that every code produced by the sources (`fn code`, `fn message_id`, `NOTICE_*`, code
### literals) is present here in both languages.

## Configuration: errors (`pdfover_config::ConfigError::message_id`)

ng-config-error-io = Input/output error in the settings: { $detail }
ng-config-error-parse = The settings cannot be read: { $detail }
ng-config-error-serialize = The settings could not be saved: { $detail }
ng-config-error-unsupported-schema = The settings were written by a newer version (schema { $found }, supported up to { $supported }).
ng-config-error-legacy = The PDF-Over configuration cannot be read: { $detail }
ng-config-error-no-home = The configuration folder cannot be determined.
ng-config-error-secret = Error accessing the keychain: { $detail }
ng-config-file-missing = The configuration file “{ $path }” was not found.
ng-config-reset-done = The previous settings were moved to “{ $path }”.
ng-secrets-not-persistent = No keychain available: passwords are only kept until the program exits.

## Configuration: reading PDF-Over.config (`pdfover_config::LegacyWarning::code`)

ng-config-legacy-invalid-bku = BKU: the value “{ $value }” is invalid; no default is used.
ng-config-legacy-invalid-proxy-port = PROXY_PORT: “{ $value }” is not a valid number and is ignored.
ng-config-legacy-proxy-port-out-of-range = PROXY_PORT: { $value } is outside 1 to 65535 and is ignored.
ng-config-legacy-invalid-value = { $key }: the value “{ $value }” is invalid; the default applies.
ng-config-legacy-unknown-locale = { $key }: “{ $value }” is not a valid language; the system language applies.
ng-config-legacy-unsupported-locale = { $key }: the language “{ $value }” is not supported; { $lang } applies.
ng-config-legacy-unknown-profile = SIGNATURE_PROFILE: “{ $value }” is unknown; the standard signature block applies.
ng-config-legacy-unknown-pass-store-type = KEYSTORE_PASS_STORE_TYPE: “{ $value }” is unknown; keystore passwords are not saved.

## Configuration: import from ~/.pdf-over (`pdfover_config::migrate`)

ng-migrate-offer-title = Import settings from PDF-Over?
ng-migrate-offer-text = Settings of a previous PDF-Over installation were found (“{ $path }”). Do you want to import them? The old file remains unchanged.
ng-migrate-secrets-consent = The old configuration contains passwords in plain text. Do you want to move them to the operating system keychain?
ng-migrate-remove-plaintext = The passwords are still stored in plain text in “{ $path }”. You can have them removed there.
ng-migrate-done = Imported: { $imported } settings; not imported: { $skipped }.
ng-migrate-undo = Undo import
ng-migrate-key-ignored = { $key } is not imported (no function in PDF-Over NG).
ng-migrate-unknown-key = The unknown entry { $key } is ignored.
ng-migrate-path-missing = { $key }: the path “{ $path }” does not exist.
ng-migrate-path-dropped = { $key }: the path “{ $path }” does not exist and is not imported.
ng-migrate-emblem-unsupported = EMBLEM: “{ $path }” is not a supported image (PNG, JPEG, GIF) and is not imported.
ng-migrate-emblem-from-cache = EMBLEM: the original “{ $path }” is missing; the emblem cached by PDF-Over is imported.
ng-migrate-secret-ignored = { $key }: the password is not imported because the storage type is not “permanent”.
ng-migrate-secret-no-target = { $key }: the password cannot be assigned without the matching keystore file or proxy host and user.
ng-migrate-signature-field-discarded = The signature field permanently stored by PDF-Over is discarded (B-05); the selection now applies to the current document only.
ng-migrate-custom-profile = Custom PDF-AS profile found: “{ $file }”.
ng-migrate-note-default = The signature note equals the default text and will follow profile and language from now on.
ng-migrate-placeholder-search-off = Placeholder search was enabled, but neither QR code nor signature field search was selected; it stays disabled.
ng-migrate-proxy-without-host = PROXY_PORT or PROXY_USER without PROXY_HOST are not imported.
ng-migrate-keystore-jks = Keystore type { $value }: JKS or JCEKS keystores must be converted to PKCS#12 before use.

## CMS signatures (`pdfover_cades::CmsError::code`)

cms-der = The signature data (CMS) cannot be parsed.
cms-certificate = A certificate cannot be read.
cms-unsupported-algorithm = The signature or hash algorithm is not supported.
cms-not-signed-data = The data is not a CMS signature (SignedData).
cms-signer-count = The signature does not contain exactly one signer.
cms-missing-attribute = A required signature attribute is missing.
cms-digest-mismatch = The signature digest does not match the document.
cms-invalid-signature = The signature value is invalid.
cms-signer-certificate-missing = The signer certificate is missing from the signature.
cms-signing-certificate-mismatch = The signing-certificate attribute refers to a different certificate.
cms-unexpected-signer = The signature was created with a certificate other than the expected one.
cms-key = The key cannot be used.
cms-key-mismatch = The private key does not match the certificate.
cms-too-large = The signature exceeds the reserved space.
cms-io = Read error while verifying the signature.

## Keystore (`pdfover_backend_keystore::KeystoreError::code`)

keystore-file-not-found = The keystore file was not found.
keystore-wrong-password = The keystore password is wrong.
keystore-alias-not-found = The alias was not found in the keystore.
keystore-not-a-key-entry = The alias is not a key entry.
keystore-conversion-needed = The keystore format (JKS/JCEKS) is not supported – please convert it to PKCS#12.
keystore-unsupported-pbe = The keystore encryption is not supported – please export it again.
keystore-different-key-password = The key is protected by a separate password – please export the PKCS#12 with a single password.
keystore-unsupported-key = The key type is not supported.
keystore-cms = The certificate or key in the keystore cannot be used.
keystore-io = The keystore file cannot be read.
keystore-malformed = The file is not a valid PKCS#12 keystore.
keystore-no-key-entry = The keystore contains no key with a certificate.
keystore-alias-required = The keystore contains several keys – please specify an alias.
keystore-key-mismatch = The key and certificate in the keystore do not match.

## Signature backends (`pdfover_backend_api::BackendError::code`); variables = fields of the variant

backend-cancelled = The operation was cancelled.
backend-unavailable = The signing method is not available: { $detail }
backend-network = The connection to the signature service failed: { $detail }
backend-proxy-auth = The proxy server requires authentication.
backend-document-too-large = The document exceeds the size limit of the signature service.
backend-http = The signature service rejected the request: HTTP { $status } { $reason }
backend-security-layer = Operation aborted by the signature server ({ $code }): { $info }
backend-unknown-page = Unknown page of the signature service ({ $diagnostic }). The operation was cancelled.
backend-loop = The signature service is responding in an endless loop; the operation was cancelled.
backend-protocol = Unexpected response from the signature service: { $detail }
backend-wrong-password = The password is wrong.
backend-keystore = Keystore error: { $detail }
backend-keystore-format = The keystore format { $format } is not supported – please convert it to PKCS#12.
backend-certificate-mismatch = The signature was created with a certificate other than the expected one.
backend-fido2 = FIDO2 error: { $detail }
backend-timeout = Timeout: the signature service did not respond in time.
backend-internal = Internal error: { $detail }

## Backend notices and forms without a PDF-Over key (`NoticeCode::AtrustInterstitial` as title,
## `FormProblem::LoginRejected`); mapping of all variants: crates/i18n/tests/code_catalog.rs

ng-notice-atrust-interstitial = Notice from A-Trust
ng-form-login-rejected = The login was not accepted. Please enter the password again.

## ID Austria via A-Trust, scenario A (`pdfover_backend_atrust`)

atrust-unknown-page = Unknown page of the signature service ({ $diagnostic }). The operation was cancelled.
atrust-signature-data-link-rejected = The “show signature data” link does not lead to A-Trust and is not opened.

## FIDO2 (`pdfover_backend_atrust::Fido2Error::code` and detail codes of the providers)

fido2-unavailable = FIDO2 is not available on this device.
fido2-cancelled = The operation was cancelled at the security key.
fido2-invalid-options = The FIDO2 request from the signature service could not be read.
fido2-device = Security key error: { $detail }
fido2-pin-unsupported = This security key requires a PIN; PIN entry is not yet available on this system – please use the app or SMS.
fido2-no-credential = This security key does not hold an ID Austria registration.
fido2-timeout = Timeout: the security key was not touched.
fido2-locked = The security key is locked.
fido2-u2f-only = The security key does not support FIDO2 (U2F only).
fido2-no-user-verification = The security key cannot verify the user (no PIN or biometrics set up).
fido2-hid-permission = No access to the security key – the udev rule (uaccess) for FIDO devices is missing.

## Signature block (`pdfover_sigblock::SigBlockError::code`, `NOTICE_CHARS_REPLACED`)

sigblock-unknown-profile = The signature profile “{ $profile }” does not exist.
sigblock-profile = The signature profile is invalid: { $detail }
sigblock-unsupported-expression = The expression { $expr } in the signature profile is not supported.
sigblock-unsupported-date-pattern = The date format “{ $pattern }” in the signature profile is not supported.
sigblock-font = The signature block font could not be loaded: { $detail }
sigblock-missing-glyph = The character “{ $char }” is not available in the signature block font.
sigblock-image = The signature logo could not be read (supported: PNG, JPEG, GIF).
sigblock-missing-resource = A resource of the signature profile is missing: { $detail }
sigblock-pdf = The signature appearance could not be created.
sigblock-io = Error reading the signature block configuration.
sigblock-chars-replaced = Some characters are not available in the signature block font and were replaced by “?”: { $chars }

## Rendering (`pdfover_render::RenderError::code`, code `render.…` → ID `render-…`)

render-library_not_found = The PDF rendering library (PDFium) could not be found: { $details }
render-load_failed = The document could not be loaded: { $details }
render-password_required = The document is password protected.
render-page_out_of_range = Page { $page } does not exist (the document has { $count } pages).
render-failed = The page could not be rendered: { $details }

## Placement (`pdfover_placement::PlacementError::code`, `PlacementNotice::code`; `placement.…` → `placement-…`)

placement-invalid_position = Invalid position specification: { $position }
placement-no_such_page = Page { $page } does not exist.
placement-placeholder_not_found = No matching signature placeholder was found.
placement-not_enough_space = There is not enough space for the signature block, and no new page may be added to an already signed document.
placement-notice-new_page_not_allowed = The document is already signed – the signature block was placed on the last page instead of a new page.
placement-notice-overlaps_content = The signature block covers page content.
placement-notice-overlaps_widget = The signature block covers an existing form or signature field.
placement-notice-clamped = The position was adjusted to fit the page.
placement-notice-scan_failed = The free space on the page could not be determined; the signature block was placed at the top of the page.

## Input analysis (`pdfover_pades::Finding::code`, plan §6.1)

pdf-xref-repaired = The document's cross-reference table is damaged and was repaired while reading. Signing is possible but does not fix the damage.
pdf-xref-reconstructed = The document's cross-reference table is missing or unreadable. The document cannot be signed without repair.
pdf-tail-garbage = There is extraneous data after the end-of-file marker (%%EOF).
pdf-docmdp-p1 = The document is certified and does not allow any changes (DocMDP P=1); it cannot be signed.
pdf-user-password = The document is protected by an open password and cannot be signed.
pdf-xfa-dynamic = The document is a dynamic XFA form and cannot be signed.
pdf-usage-rights = The document contains extended Adobe Reader rights. Signing invalidates them; Adobe Reader will show a warning afterwards.
