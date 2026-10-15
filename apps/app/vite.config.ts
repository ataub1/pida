/// <reference types="vitest/config" />
// Vite/Vitest-Konfiguration der Oberfläche von PIDA.
//
// - Die Fluent-Kataloge kommen zur Build-Zeit aus `crates/i18n/locales/{de,en}/*.ftl` (eine Textquelle
//   für Kern, CLI und UI, Plan §7.5) über den Alias `@locales` und `?raw`.
// - `--mode mock` (`.env.mock`: `VITE_MOCK=1`) startet die Oberfläche mit dem TS-Mock-Kern, ohne Rust.
// - Port 1420 fest (Tauri `devUrl`).
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '../..');
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  resolve: {
    alias: {
      '@locales': path.resolve(repoRoot, 'crates/i18n/locales'),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    fs: {
      // Kataloge liegen außerhalb von apps/app.
      allow: [here, path.resolve(repoRoot, 'crates/i18n/locales')],
    },
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    // WebView2 (Chromium) bzw. WKWebView/WebKitGTK (Safari 16)
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari16',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    outDir: 'dist',
    emptyOutDir: true,
    // React Aria Components + Fluent-Kataloge (DE/EN) in einem Paket; lokal geladen, kein Netz.
    chunkSizeWarningLimit: 900,
  },
  test: {
    environment: 'jsdom',
    globals: false,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: false,
    restoreMocks: true,
    testTimeout: 15000,
  },
});
