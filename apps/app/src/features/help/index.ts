/**
 * Offline-Hilfe in allen Builds (Desktop, Mobile, Browser). Das Panel mit Texten und Stilen ist ein eigener Chunk und
 * wird erst beim ersten Öffnen geladen. `VITE_HELP=0` blendet die Hilfe aus (Einstiege und Panel).
 */
import { lazy } from 'react';

export const HELP_AVAILABLE = import.meta.env.VITE_HELP !== '0';

export const HelpPanel = HELP_AVAILABLE ? lazy(() => import('./HelpPanel').then((m) => ({ default: m.HelpPanel }))) : () => null;
