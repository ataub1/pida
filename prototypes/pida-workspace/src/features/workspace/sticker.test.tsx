/**
 * Arbeitsbereich: Tastaturbedienung des Stickers (Pfeile, Umschalt fein, Bild auf/ab) mit Live-Region,
 * Ziehen mit Einrasten, Überdeckungswarnung, „Hier signieren“-Markierungen, Optionen.
 */
import { fireEvent, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../test/render';

function live() {
  return document.querySelector('.canvas [role="status"]') as HTMLElement;
}

function mm(text: string, what: 'links' | 'unten'): number {
  const m = new RegExp(`(\\d+) mm von ${what}`).exec(text);
  if (!m) throw new Error(`keine Angabe „von ${what}“ in: ${text}`);
  return Number(m[1]);
}

describe('Sticker per Tastatur', () => {
  it('verschiebt mit Pfeiltasten (Umschalt feiner) und beschreibt die Position', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    const sticker = screen.getByRole('application', { name: 'Signaturblock' });
    expect(sticker).toHaveAttribute('aria-roledescription', 'verschiebbarer Signaturblock');
    expect(sticker).toHaveAccessibleDescription(/Pfeiltasten/);
    sticker.focus();

    await user.keyboard('{ArrowLeft}');
    const a = live().textContent ?? '';
    expect(a).toMatch(/^Seite 3 von 3, \d+ mm von links, \d+ mm von unten\./);
    expect(a).toContain('Freie Fläche.');
    const left1 = parseFloat(sticker.style.left);

    await user.keyboard('{Shift>}{ArrowLeft}{/Shift}');
    const left2 = parseFloat(sticker.style.left);
    await user.keyboard('{ArrowLeft}');
    const left3 = parseFloat(sticker.style.left);
    const fine = left1 - left2;
    const coarse = left2 - left3;
    expect(fine).toBeGreaterThan(0);
    expect(coarse).toBeCloseTo(fine * 10, 5);

    await user.keyboard('{ArrowDown}');
    const b = live().textContent ?? '';
    expect(mm(b, 'unten')).toBeLessThan(mm(a, 'unten'));

    await user.keyboard('{PageUp}');
    expect(live().textContent).toMatch(/^Seite 2 von 3/);
    expect(screen.getByText(/^Seite 2 · /)).toBeInTheDocument();
  });

  it('warnt bei Überdeckung von Inhalt und findet auf Wunsch freie Fläche', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    const sticker = screen.getByRole('application', { name: 'Signaturblock' });
    sticker.focus();
    for (let i = 0; i < 6; i++) await user.keyboard('{Alt>}{ArrowUp}{/Alt}');
    expect(live().textContent).toContain('Achtung: überdeckt Inhalt.');
    expect(sticker).toHaveClass('is-overlap');
    const inspector = screen.getByRole('complementary', { name: 'Signatur' });
    expect(within(inspector).getByText('Der Signaturblock überdeckt Inhalt der Seite.')).toBeInTheDocument();
    await user.click(within(inspector).getByRole('button', { name: 'Freie Fläche suchen' }));
    expect(within(inspector).queryByText('Der Signaturblock überdeckt Inhalt der Seite.')).not.toBeInTheDocument();
    expect(live().textContent).toContain('Freie Fläche.');
  });
});

describe('Sticker ziehen', () => {
  it('rastet beim Ziehen an Rändern ein und zeigt freie Flächen', () => {
    renderApp({ scenario: 'blank' });
    const sticker = screen.getByRole('application', { name: 'Signaturblock' });
    // jsdom: keine Layoutmaße → Seitenspalte und Sticker bei (0,0); Maßstab = Startwert 1,15
    fireEvent.pointerDown(sticker, { button: 0, pointerId: 1, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(sticker, { pointerId: 1, clientX: 200, clientY: 300 });
    expect(document.querySelectorAll('.free-band').length).toBeGreaterThan(0);
    // fast am linken Rand (56 pt) → rastet links ein
    const scale = 1.15;
    const x = 0; // Seite beginnt links bündig in der Seitenspalte (oben 24 px Abstand)
    fireEvent.pointerMove(sticker, { pointerId: 1, clientX: x + (58 + 115) * scale, clientY: 24 + (300 + 41) * scale });
    expect(document.querySelector('.snap-guide-v')).not.toBeNull();
    fireEvent.pointerUp(sticker, { pointerId: 1, clientX: x + (58 + 115) * scale, clientY: 24 + (300 + 41) * scale });
    expect(live().textContent).toMatch(/Rastet ein: linker Rand/);
    expect(document.querySelector('.free-band')).toBeNull();
  });
});

describe('Sticker per Doppelklick', () => {
  it('setzt den Block per Doppelklick auf eine Seite (einfacher Klick verschiebt nichts)', () => {
    renderApp({ scenario: 'contract' });
    const page1 = screen.getByRole('img', { name: 'Seite 1 von 3' });
    const sticker = screen.getByRole('application', { name: 'Signaturblock' });
    const before = sticker.style.top;
    fireEvent.click(page1, { clientX: 300, clientY: 300 });
    expect(sticker.style.top).toBe(before);
    fireEvent.doubleClick(page1, { clientX: 300, clientY: 300 });
    expect(live().textContent).toMatch(/^Seite 1 von 3/);
    expect(screen.getByText(/^Seite 1 · /)).toBeInTheDocument();
  });
});

describe('Signaturfelder, Optionen', () => {
  it('setzt den Block per „Hier signieren“ in ein leeres Signaturfeld', async () => {
    const { user } = renderApp({ scenario: 'field' });
    const inspector = screen.getByRole('complementary', { name: 'Signatur' });
    expect(screen.getByText('Signaturfeld gefunden:')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Hier signieren: Signaturfeld „Unterschrift Antragsteller/in“' }));
    expect(screen.queryByText('Signaturfeld gefunden:')).not.toBeInTheDocument();
    expect(within(inspector).getByText('Im Signaturfeld „Unterschrift Antragsteller/in“')).toBeInTheDocument();
    expect(live().textContent).toContain('Im Signaturfeld „Unterschrift Antragsteller/in“.');
    expect(screen.queryByRole('button', { name: /Hier signieren/ })).not.toBeInTheDocument();
    await user.click(within(inspector).getByRole('button', { name: 'Frei platzieren' }));
    expect(screen.getByRole('button', { name: /Hier signieren/ })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Ins Feld setzen' }));
    expect(within(inspector).getByText('Im Signaturfeld „Unterschrift Antragsteller/in“')).toBeInTheDocument();
  });

  it('„Unsichtbar signieren“ blendet den Sticker aus, „Auf neuer Seite“ hängt eine Seite an', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    await user.click(screen.getByRole('switch', { name: 'Auf neuer Seite' }));
    expect(screen.getByRole('img', { name: 'Neue Seite für die Signatur' })).toBeInTheDocument();
    expect(screen.getByText(/^Seite 4 · /)).toBeInTheDocument();
    await user.click(screen.getByRole('switch', { name: 'Auf neuer Seite' }));
    await user.click(screen.getByRole('switch', { name: 'Unsichtbar signieren' }));
    expect(screen.queryByRole('application', { name: 'Signaturblock' })).not.toBeInTheDocument();
    expect(screen.getByText(/Unsichtbare Signatur/)).toBeInTheDocument();
  });

  it('zoomt und blendet die Seitenleiste ein/aus', async () => {
    const { user } = renderApp({ scenario: 'contract' });
    const nav = screen.getByRole('navigation', { name: 'Seiten' });
    expect(within(nav).getAllByRole('button')).toHaveLength(3);
    expect(within(nav).getByRole('button', { name: 'Seite 3, enthält den Signaturblock' })).toBeInTheDocument();
    const value = () => screen.getByLabelText(/^Zoom \d+ %$/).textContent;
    const before = value();
    await user.click(screen.getByRole('button', { name: 'Vergrößern' }));
    expect(value()).not.toBe(before);
    await user.click(screen.getByRole('button', { name: 'An Breite anpassen' }));
    expect(value()).toBe(before);
    await user.click(screen.getByRole('button', { name: 'Seitenleiste ein-/ausblenden' }));
    expect(screen.queryByRole('navigation', { name: 'Seiten' })).not.toBeInTheDocument();
  });
});
