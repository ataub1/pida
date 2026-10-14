### PDF-Over NG – own texts, English (EUPL-1.2)
###
### Maintained by hand. All IDs start with `ng-` so they never collide with the IDs generated from
### PDF-Over (`pdfover.ftl`). New message IDs of other packages are added here by the integration
### (ADR 0001 rule 11); every ID needs an entry in `de/` and `en/` with the same variables.

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
