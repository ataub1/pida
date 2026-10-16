/**
 * Stabile Exporte des Pakets UI-B für Einstellungen und Datenschutz (ADR 0004 §6.1, in E0 festgelegt):
 * `App.tsx` und die UI-A-Bausteine (Konto-Chip, Signierkarte, „Signatur fortsetzen“) binden nur diese ein.
 * Seit B3 ist `SettingsPanel` die Seitenleiste mit Suche und sofortiger Wirkung (der bisherige Dialog ist
 * gelöscht).
 */
export { PrivacyInfo, PrivacyNote } from './privacy';
export { SettingsPanel, type SettingsPanelProps } from './SettingsPanel';
export { DEFAULT_STORAGE_CHOICE, PasswordStorageQuestion } from './StorageQuestion';
