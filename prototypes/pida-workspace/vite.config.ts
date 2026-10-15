/// <reference types="vitest/config" />
// Vite/Vitest-Konfiguration des PIDA-Prototyps (Arbeitsbereich).
//
// - Eigenständig, ohne Rust-Kern und ohne Tauri: alle Daten sind Mock-Daten im Quelltext.
// - Port 1430 fest (apps/app nutzt 1420), `strictPort`, damit Links in der Abnahme stabil sind.
// - Keine Netzwerkzugriffe zur Laufzeit, keine externen Fonts/CDNs.
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1430,
    strictPort: true,
  },
  preview: {
    port: 1430,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 900,
  },
  test: {
    environment: 'jsdom',
    globals: false,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: false,
    restoreMocks: true,
    testTimeout: 20000,
  },
});
