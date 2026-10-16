/**
 * Stabile Exporte des Pakets UI-B für die Ersteinrichtung (ADR 0004 §6.1, in E0 festgelegt). Seit B4 die
 * Ersteinrichtung in drei Schritten bzw. das Migrationsangebot im neuen Stil (der alte `MigrationDialog` ist
 * gelöscht); `MigrationOfferPane` nutzen auch die Einstellungen (Erweitert › PDF-Over-Einstellungen übernehmen).
 */
export { MigrationOfferPane, type MigrationOfferPaneProps } from './MigrationOffer';
export { Onboarding, type OnboardingProps } from './Onboarding';
