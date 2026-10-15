/**
 * Einstieg der Oberfläche: Tauri-Bridge zum Rust-Kern, oder der TS-Mock-Kern bei `VITE_MOCK=1`
 * (`npm run dev:mock`) bzw. außerhalb von Tauri (Browser-Vorschau).
 */
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { createTauriBridge, inTauri, type Bridge } from './core/bridge';
import type { MockCore } from './core/mock';
import './styles.css';

async function bridge(): Promise<Bridge> {
  const forceMock = import.meta.env.VITE_MOCK === '1';
  if (!forceMock && inTauri()) return createTauriBridge();
  // Mock-Kern nur bei Bedarf laden (eigener Chunk, nicht im Startpfad der App).
  const { MockCore: Mock } = await import('./core/mock');
  const mock = new Mock();
  installBrowserDrop(mock);
  return mock;
}

/** Browser-Vorschau: HTML5-Drop als Ersatz für die Fenster-Ereignisse von Tauri (nur Dateinamen). */
function installBrowserDrop(mock: MockCore): void {
  let depth = 0;
  window.addEventListener('dragenter', (e) => {
    if (!e.dataTransfer?.types.includes('Files')) return;
    depth += 1;
    mock.simulateFileDrag({ type: 'enter', paths: [], x: e.clientX, y: e.clientY });
  });
  window.addEventListener('dragover', (e) => {
    if (!e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
    mock.simulateFileDrag({ type: 'over', x: e.clientX, y: e.clientY });
  });
  window.addEventListener('dragleave', () => {
    depth = Math.max(0, depth - 1);
    if (depth === 0) mock.simulateFileDrag({ type: 'leave' });
  });
  window.addEventListener('drop', (e) => {
    e.preventDefault();
    depth = 0;
    const names = Array.from(e.dataTransfer?.files ?? []).map((f) => `/Browser/${f.name}`);
    mock.simulateFileDrag({ type: 'drop', paths: names, x: e.clientX, y: e.clientY });
  });
}

const root = document.getElementById('root');
if (root) {
  bridge()
    .then((b) => {
      createRoot(root).render(
        <StrictMode>
          <App bridge={b} />
        </StrictMode>,
      );
    })
    .catch(() => {
      root.textContent = 'PDF-Over NG: Die Oberfläche konnte nicht gestartet werden. / The user interface could not be started.';
    });
}
