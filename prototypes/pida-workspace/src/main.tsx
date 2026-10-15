import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { createState } from './app/store';
import type { Lang, ScenarioId, ThemePref } from './model/types';
import './styles/tokens.css';
import './styles/base.css';
import './styles/components.css';
import './styles/document.css';
import './styles/views.css';

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

const SCENARIOS: ScenarioId[] = ['start', 'blank', 'contract', 'field', 'signed', 'batch', 'verify', 'error-password', 'error-atrust'];
const params = new URLSearchParams(window.location.search);
const fromUrl = params.get('scenario') ?? params.get('szenario');
const scenario = SCENARIOS.includes(fromUrl as ScenarioId) ? (fromUrl as ScenarioId) : 'start';
const theme = read('pida.theme');
const lang = params.get('lang') ?? read('pida.lang');
const onboarded = read('pida.onboarded') === '1' || params.get('onboarding') === '0';
const isMac = /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);

const initial = createState({
  scenario,
  onboarded,
  settings: {
    theme: theme === 'light' || theme === 'dark' || theme === 'system' ? (theme as ThemePref) : 'system',
    uiLang: lang === 'en' || lang === 'de' ? (lang as Lang) : 'de',
  },
});

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App initial={initial} isMac={isMac} />
    </StrictMode>,
  );
}
