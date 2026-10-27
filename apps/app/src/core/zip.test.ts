import { describe, expect, it } from 'vitest';
import { crc32, zipStored } from './zip';

describe('zipStored', () => {
  it('CRC-32 wie ZIP (Prüfwert „123456789“)', () => {
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
  });

  it('Einträge, Verzeichnis und Ende; doppelte Namen nummeriert', () => {
    const a = new TextEncoder().encode('%PDF-a');
    const b = new TextEncoder().encode('%PDF-bb');
    const zip = zipStored([
      { name: 'vertrag_signed.pdf', data: a },
      { name: 'Vertrag_signed.pdf', data: b },
    ]);
    const v = new DataView(zip.buffer);
    expect(v.getUint32(0, true)).toBe(0x04034b50);
    const end = zip.length - 22;
    expect(v.getUint32(end, true)).toBe(0x06054b50);
    expect(v.getUint16(end + 10, true)).toBe(2);
    const dirOffset = v.getUint32(end + 16, true);
    expect(v.getUint32(dirOffset, true)).toBe(0x02014b50);
    const text = new TextDecoder().decode(zip);
    expect(text).toContain('vertrag_signed.pdf');
    expect(text).toContain('Vertrag_signed (2).pdf');
    // Inhalt unkomprimiert direkt nach dem lokalen Kopf
    expect(new TextDecoder().decode(zip.slice(30 + 'vertrag_signed.pdf'.length, 30 + 'vertrag_signed.pdf'.length + a.length))).toBe('%PDF-a');
  });
});
