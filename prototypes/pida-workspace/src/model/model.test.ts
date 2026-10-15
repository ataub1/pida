/** Reine Regeln: automatische Position, Einrasten, freie Flächen, Überdeckung, Dateinamen. */
import { describe, expect, it } from 'vitest';
import { blankDoc, contractDoc, estimateLines, fieldDoc } from './documents';
import { blockSize, maskUser, outputName } from './format';
import { autoPosition, freeBands, isFree, MARGIN, overlaps, snap } from './geometry';

const SIZE = { w: 230, h: 82 };

describe('Platzierung', () => {
  it('setzt den Block rechtsbündig auf freie Fläche unter den Inhalt der letzten Seite', () => {
    const doc = contractDoc();
    const last = doc.pages[doc.pages.length - 1]!;
    const pos = autoPosition(last, SIZE)!;
    expect(pos.x).toBe(last.w - MARGIN.right - SIZE.w);
    const contentBottom = Math.max(...last.blocks.filter((b) => b.kind !== 'footer').map((b) => b.rect.y + b.rect.h));
    expect(pos.y).toBeGreaterThan(contentBottom);
    expect(isFree({ ...pos, ...SIZE }, last)).toBe(true);
  });

  it('setzt den Block auf einer leeren Seite rechts unten', () => {
    const page = blankDoc().pages[0]!;
    const pos = autoPosition(page, SIZE)!;
    expect(pos.x + SIZE.w).toBe(page.w - MARGIN.right);
    expect(pos.y + SIZE.h).toBeGreaterThan(page.h * 0.8);
  });

  it('meldet Überdeckung und freie Bänder', () => {
    const page = contractDoc().pages[0]!;
    expect(overlaps({ x: 100, y: 120, ...SIZE }, page).length).toBeGreaterThan(0);
    const last = contractDoc().pages[2]!;
    const bands = freeBands(last);
    expect(bands.length).toBeGreaterThan(0);
    expect(bands.some((b) => b.h > 200)).toBe(true);
  });

  it('rastet an rechtem und unterem Rand ein (Ecke rechts unten)', () => {
    const page = blankDoc().pages[0]!;
    const r = snap(page.w - MARGIN.right - SIZE.w - 3, page.h - MARGIN.bottom - SIZE.h + 4, SIZE, page, 6);
    expect(r.snapped).toEqual(['right', 'bottom']);
    expect(r.x).toBe(page.w - MARGIN.right - SIZE.w);
    expect(r.guides).toHaveLength(2);
    expect(snap(200, 300, SIZE, page, 6).snapped).toEqual([]);
  });

  it('das Signaturfeld des Antrags überdeckt keinen Inhalt', () => {
    const doc = fieldDoc();
    const f = doc.fields[0]!;
    expect(overlaps(f.rect, doc.pages[f.page]!)).toHaveLength(0);
  });

  it('Textblöcke sind großzügig bemessen (kein Überlauf)', () => {
    expect(estimateLines('kurz', 400)).toBe(1);
    expect(estimateLines('wort '.repeat(200), 483)).toBeGreaterThan(10);
  });
});

describe('Format', () => {
  it('bildet den Ausgabenamen nach Muster', () => {
    expect(outputName('Vertrag.pdf', '{name}_signiert')).toBe('Vertrag_signiert.pdf');
    expect(outputName('Vertrag.pdf', '')).toBe('Vertrag_signiert.pdf');
    expect(outputName('a.b.pdf', 'x/{name}')).toBe('x_a.b.pdf');
    expect(outputName('Vertrag.pdf', '{name}_{datum}', new Date(2026, 9, 15))).toBe('Vertrag_2026-10-15.pdf');
  });

  it('maskiert die Telefonnummer im Konto-Chip', () => {
    expect(maskUser('+43 660 1234123')).toBe('+43 660 ···123');
  });

  it('Blockgröße je Profil und Hinweis', () => {
    expect(blockSize({ profile: 'standard', lang: 'de', logo: null, logoName: null, note: null })).toEqual({ w: 230, h: 82 });
    expect(blockSize({ profile: 'standard', lang: 'de', logo: null, logoName: null, note: '' })).toEqual({ w: 230, h: 49 });
    expect(blockSize({ profile: 'amt', lang: 'en', logo: null, logoName: null, note: null }).w).toBe(400);
  });
});
