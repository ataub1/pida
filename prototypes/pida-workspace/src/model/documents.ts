/**
 * Mock-Dokumente des Prototyps. Die Seiten werden als HTML gesetzt (MockPage); die Lage jedes Blocks
 * steht hier in pt, damit freie Flächen, Einrasten und Überdeckung ohne Layout-Messung funktionieren.
 * Texte sind frei erfunden (keine echten Personen, Firmen oder Verträge).
 */
import { MARGIN } from './geometry';
import type { Block, DocSpec, ExistingSignature, PageSpec, SignatureField } from './types';

export const A4 = { w: 595, h: 842 } as const;
const CONTENT_W = A4.w - MARGIN.left - MARGIN.right;

/** Schriftmaße der Mock-Seiten (müssen zu `.doc-*` in `styles/document.css` passen). */
export const DOC_FONT = { para: 9, lineHeight: 13, charFactor: 0.56 } as const;

/** Grobe Zeilenzahl für Fließtext (gieriger Umbruch mit vorsichtiger Zeichenbreite). */
export function estimateLines(text: string, width: number, fontSize = DOC_FONT.para): number {
  const cpl = Math.max(8, Math.floor(width / (fontSize * DOC_FONT.charFactor)));
  let n = 1;
  let cur = 0;
  for (const word of text.split(/\s+/)) {
    const l = word.length;
    if (cur === 0) cur = l;
    else if (cur + 1 + l <= cpl) cur += 1 + l;
    else {
      n += 1;
      cur = l;
    }
  }
  return n;
}

/** Setzt Blöcke von oben nach unten in den Satzspiegel. */
class PageBuilder {
  private y: number = MARGIN.top;
  private readonly blocks: Block[] = [];

  letterhead(org: string, lines: string[]) {
    const h = 16 + lines.length * 11;
    this.blocks.push({ kind: 'letterhead', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, org, lines });
    this.y += h + 22;
    return this;
  }
  title(text: string) {
    this.blocks.push({ kind: 'title', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h: 24 }, text });
    this.y += 28;
    return this;
  }
  subtitle(text: string) {
    this.blocks.push({ kind: 'subtitle', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h: 14 }, text });
    this.y += 28;
    return this;
  }
  heading(text: string) {
    this.y += 8;
    this.blocks.push({ kind: 'heading', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h: 15 }, text });
    this.y += 19;
    return this;
  }
  para(text: string) {
    const h = estimateLines(text, CONTENT_W) * DOC_FONT.lineHeight;
    this.blocks.push({ kind: 'para', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, text });
    this.y += h + 8;
    return this;
  }
  list(items: string[]) {
    const h = items.reduce((s, it) => s + estimateLines(it, CONTENT_W - 16) * DOC_FONT.lineHeight + 3, 0);
    this.blocks.push({ kind: 'list', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, items });
    this.y += h + 8;
    return this;
  }
  parties(lines: string[]) {
    const h = lines.length * 14;
    this.blocks.push({ kind: 'parties', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, lines });
    this.y += h + 12;
    return this;
  }
  table(columns: string[], rows: string[][], numeric: boolean[], totalRow = false) {
    const h = 22 + rows.length * 19;
    this.blocks.push({ kind: 'table', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, columns, rows, numeric, totalRow });
    this.y += h + 12;
    return this;
  }
  form(rows: [string, string][]) {
    const h = rows.length * 24;
    this.blocks.push({ kind: 'form', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h }, rows });
    this.y += h + 10;
    return this;
  }
  sigline(left: [string, string], right: [string, string]) {
    this.y += 26;
    this.blocks.push({ kind: 'sigline', rect: { x: MARGIN.left, y: this.y, w: CONTENT_W, h: 44 }, left, right });
    this.y += 44 + 8;
    return this;
  }
  signature(signatureId: string, x: number, w = 230, h = 82) {
    this.y += 14;
    this.blocks.push({ kind: 'signature', rect: { x, y: this.y, w, h }, signatureId });
    this.y += h + 8;
    return this;
  }
  space(h: number) {
    this.y += h;
    return this;
  }
  /** aktuelle Schreibposition (für Felder unter dem Text) */
  get cursor() {
    return this.y;
  }
  footer(text: string) {
    this.blocks.push({ kind: 'footer', rect: { x: MARGIN.left, y: A4.h - 34, w: CONTENT_W, h: 12 }, text });
    return this;
  }
  build(): PageSpec {
    return { w: A4.w, h: A4.h, blocks: [...this.blocks] };
  }
}

const page = () => new PageBuilder();

const MUSTER_IT = {
  org: 'Muster IT-Dienstleistungen GmbH',
  lines: ['Praterstraße 12 · 1020 Wien · FN 512345a · UID ATU12345678', 'office@muster-it.example · +43 1 234 56 78'],
};

/* ---------------------------------------------------------------- Vertrag (3 Seiten) */

function contractPages(): PageSpec[] {
  const f = (n: number) => `Dienstleistungsvertrag DV-2026-0412 · Seite ${n} von 3`;
  const p1 = page()
    .letterhead(MUSTER_IT.org, MUSTER_IT.lines)
    .title('Dienstleistungsvertrag')
    .subtitle('Vertragsnummer DV-2026-0412 · Betrieb und Wartung der Kundenplattform')
    .parties([
      'abgeschlossen zwischen',
      'Stadtwerke Beispielstadt GmbH, Hauptplatz 1, 4020 Linz',
      '– im Folgenden „Auftraggeber“ –',
      'und',
      'Muster IT-Dienstleistungen GmbH, Praterstraße 12, 1020 Wien',
      '– im Folgenden „Auftragnehmer“ –',
    ])
    .heading('§ 1 Vertragsgegenstand')
    .para(
      'Der Auftragnehmer übernimmt für den Auftraggeber den Betrieb, die Wartung und die Weiterentwicklung der Kundenplattform „Mein Stadtwerk“ einschließlich der Schnittstellen zu Abrechnung und Zählerdatenmanagement. Art und Umfang der Leistungen ergeben sich aus diesem Vertrag sowie aus der Leistungsbeschreibung in Anlage 1, die einen integrierenden Bestandteil des Vertrags bildet.',
    )
    .heading('§ 2 Leistungsumfang')
    .para('Die Leistungen des Auftragnehmers umfassen insbesondere:')
    .list([
      'den Betrieb der Plattform in einem Rechenzentrum in Österreich mit einer Verfügbarkeit von mindestens 99,5 % im Kalendermonat,',
      'das Einspielen von Sicherheits- und Funktionsupdates innerhalb der in Anlage 1 vereinbarten Fristen,',
      'die Störungsbehebung während der Servicezeiten Montag bis Freitag von 7:00 bis 18:00 Uhr,',
      'monatliche Berichte über Verfügbarkeit, Störungen und durchgeführte Änderungen.',
    ])
    .para(
      'Leistungen, die über den vereinbarten Umfang hinausgehen, erbringt der Auftragnehmer nur nach schriftlicher Beauftragung durch den Auftraggeber zu den Stundensätzen gemäß § 4.',
    )
    .heading('§ 3 Vertragsdauer')
    .para(
      'Der Vertrag tritt mit Unterzeichnung durch beide Vertragsparteien in Kraft und wird auf unbestimmte Zeit abgeschlossen. Die Leistungserbringung beginnt am 1. Jänner 2027. Die ersten 24 Monate gelten als Mindestvertragsdauer, während der eine ordentliche Kündigung ausgeschlossen ist.',
    )
    .footer(f(1))
    .build();

  const p2 = page()
    .heading('§ 4 Vergütung')
    .para(
      'Für die Leistungen nach diesem Vertrag erhält der Auftragnehmer die folgende Vergütung. Alle Beträge verstehen sich in Euro zuzüglich der gesetzlichen Umsatzsteuer.',
    )
    .table(
      ['Leistung', 'Abrechnung', 'Betrag (netto)'],
      [
        ['Einrichtung und Datenmigration', 'einmalig', '4.800,00 €'],
        ['Betrieb und Wartung', 'monatlich', '1.250,00 €'],
        ['Rufbereitschaft außerhalb der Servicezeiten', 'monatlich', '320,00 €'],
        ['Weiterentwicklung', 'je Stunde', '95,00 €'],
        ['Summe laufend pro Monat', '', '1.570,00 €'],
      ],
      [false, false, true],
      true,
    )
    .para(
      'Die monatliche Vergütung ist jeweils im Voraus bis zum fünften Werktag des Monats zur Zahlung fällig. Die Beträge werden jährlich zum 1. Jänner entsprechend dem von Statistik Austria verlautbarten Verbraucherpreisindex 2020 wertgesichert.',
    )
    .heading('§ 5 Abnahme')
    .para(
      'Die Einrichtung gilt als abgenommen, wenn der Auftraggeber nicht binnen zehn Werktagen nach Mitteilung der Betriebsbereitschaft schriftlich wesentliche Mängel rügt. Unwesentliche Mängel hindern die Abnahme nicht und werden im Rahmen der Wartung behoben.',
    )
    .heading('§ 6 Gewährleistung und Haftung')
    .para(
      'Der Auftragnehmer leistet Gewähr dafür, dass die Plattform die in Anlage 1 beschriebenen Eigenschaften aufweist. Die Haftung für leichte Fahrlässigkeit ist, ausgenommen bei Personenschäden, ausgeschlossen. Die Haftung für Folgeschäden und entgangenen Gewinn ist mit der Höhe der Jahresvergütung begrenzt.',
    )
    .heading('§ 7 Datenschutz')
    .para(
      'Soweit der Auftragnehmer personenbezogene Daten im Auftrag verarbeitet, schließen die Parteien eine Vereinbarung gemäß Art. 28 DSGVO ab (Anlage 2). Der Auftragnehmer verarbeitet diese Daten ausschließlich innerhalb der Europäischen Union und setzt dem Stand der Technik entsprechende technische und organisatorische Maßnahmen ein.',
    )
    .footer(f(2))
    .build();

  const p3 = page()
    .heading('§ 8 Vertraulichkeit')
    .para(
      'Die Vertragsparteien verpflichten sich, alle im Rahmen dieses Vertrags erlangten Informationen über die jeweils andere Partei vertraulich zu behandeln. Diese Verpflichtung gilt auch nach Beendigung des Vertragsverhältnisses für die Dauer von fünf Jahren fort.',
    )
    .heading('§ 9 Kündigung')
    .para(
      'Nach Ablauf der Mindestvertragsdauer kann der Vertrag von jeder Partei unter Einhaltung einer Frist von sechs Monaten zum Ende eines Kalendervierteljahres gekündigt werden. Das Recht zur außerordentlichen Kündigung aus wichtigem Grund bleibt unberührt.',
    )
    .heading('§ 10 Schlussbestimmungen')
    .para(
      'Änderungen und Ergänzungen dieses Vertrags bedürfen der Schriftform; qualifiziert elektronisch signierte Erklärungen erfüllen dieses Erfordernis. Es gilt österreichisches Recht unter Ausschluss der Verweisungsnormen. Gerichtsstand ist Linz.',
    )
    .sigline(['Linz, am', 'Für den Auftraggeber'], ['Wien, am', 'Für den Auftragnehmer'])
    .footer(f(3))
    .build();
  return [p1, p2, p3];
}

export function contractDoc(fileName = 'Vertrag.pdf', id = 'doc-contract'): DocSpec {
  return { id, fileName, folder: '~/Dokumente/Verträge', sizeKb: 214, pages: contractPages(), fields: [], signatures: [], locked: false };
}

/* ---------------------------------------------------------------- Leeres Dokument */

export function blankDoc(): DocSpec {
  return {
    id: 'doc-blank',
    fileName: 'Leeres_Dokument.pdf',
    folder: '~/Downloads',
    sizeKb: 3,
    pages: [{ w: A4.w, h: A4.h, blocks: [] }],
    fields: [],
    signatures: [],
    locked: false,
  };
}

/* ---------------------------------------------------------------- Antrag mit Signaturfeld */

export function fieldDoc(): DocSpec {
  const b1 = page()
    .letterhead('Fördergesellschaft Beispielland', ['Abteilung Wirtschaft und Innovation · Landhausplatz 3 · 6020 Innsbruck'])
    .title('Antrag auf Förderung')
    .subtitle('Programm „Digital starten 2026“ · Antragsnummer DS-26-01873')
    .heading('1. Angaben zur Antragstellerin / zum Antragsteller')
    .form([
      ['Name / Firma', 'Max Mustermann e.U.'],
      ['Anschrift', 'Mariahilfer Straße 88/4, 1070 Wien'],
      ['UID-Nummer', 'ATU87654321'],
      ['E-Mail', 'max@mustermann.example'],
    ])
    .heading('2. Vorhaben')
    .form([
      ['Projekttitel', 'Online-Terminbuchung und digitale Rechnungslegung'],
      ['Projektzeitraum', '01.11.2026 – 30.06.2027'],
      ['Beantragte Förderung', '12.400,00 €'],
    ])
    .heading('3. Erklärung')
    .para(
      'Ich bestätige, dass die Angaben in diesem Antrag vollständig und richtig sind, und nehme zur Kenntnis, dass unrichtige Angaben zur Rückforderung der Förderung führen können. Ich stimme der Verarbeitung meiner Daten zur Abwicklung des Antrags zu.',
    );
  const fieldY = b1.cursor + 18;
  const p1 = b1.footer('Antrag DS-26-01873 · Seite 1 von 2').build();
  // Beschriftung des Feldes ist Inhalt; das Feld selbst ist eine leere Fläche.
  p1.blocks.push({ kind: 'subtitle', rect: { x: A4.w - MARGIN.right - 240, y: fieldY - 14, w: 240, h: 11 }, text: 'Unterschrift der Antragstellerin / des Antragstellers' });
  const field: SignatureField = {
    id: 'field-1',
    kind: 'field',
    name: 'Unterschrift_Antragsteller',
    label: 'Unterschrift Antragsteller/in',
    page: 0,
    rect: { x: A4.w - MARGIN.right - 240, y: fieldY, w: 240, h: 86 },
  };

  const p2 = page()
    .heading('4. Beilagen')
    .list([
      'Kostenaufstellung mit Angeboten von mindestens zwei Anbietern,',
      'Projektbeschreibung (höchstens drei Seiten),',
      'Bestätigung über die Eintragung im Gewerberegister bzw. Firmenbuch.',
    ])
    .heading('5. Hinweise zur Einreichung')
    .para(
      'Der Antrag ist vollständig ausgefüllt und qualifiziert elektronisch signiert über das Förderportal einzureichen. Anträge ohne gültige Signatur können nicht bearbeitet werden. Die Bearbeitungsdauer beträgt in der Regel sechs bis acht Wochen.',
    )
    .footer('Antrag DS-26-01873 · Seite 2 von 2')
    .build();

  return {
    id: 'doc-field',
    fileName: 'Antrag_Foerderung.pdf',
    folder: '~/Dokumente/Förderung',
    sizeKb: 162,
    pages: [p1, p2],
    fields: [field],
    signatures: [],
    locked: false,
  };
}

/* ---------------------------------------------------------------- Bereits signiert */

export const SIG_GRUBER: ExistingSignature = {
  id: 'sig-gruber',
  signer: 'Dr. Anna Gruber',
  role: 'Vermieterin',
  time: '2026-10-12T14:03:51+02:00',
  status: 'valid',
  level: 'QES',
  method: 'ID Austria',
  issuer: 'a-sign-premium-mobile-07 (Demo-CA)',
  serial: '1 304 118 557',
  validFrom: '2025-03-04',
  validTo: '2030-03-04',
  algorithm: 'ECDSA P-256 · SHA-256',
  format: 'PAdES-BASELINE-B',
  page: 1,
  rect: null,
  coversWholeDocument: true,
};

export function signedDoc(): DocSpec {
  const p1 = page()
    .title('Mietvertrag')
    .subtitle('über die Wohnung Top 7, Lindengasse 5, 8010 Graz')
    .parties([
      'Vermieterin: Dr. Anna Gruber, Lindengasse 5, 8010 Graz',
      'Mieter: Max Mustermann, geb. 01.01.1990, derzeit wohnhaft in 1070 Wien',
    ])
    .heading('§ 1 Mietgegenstand')
    .para(
      'Vermietet wird die Wohnung Top 7 im dritten Stock des Hauses Lindengasse 5 mit einer Nutzfläche von 68 m², bestehend aus Vorraum, Küche, zwei Zimmern, Bad mit WC und Balkon, samt Kellerabteil Nr. 7.',
    )
    .heading('§ 2 Mietdauer')
    .para(
      'Das Mietverhältnis beginnt am 1. November 2026 und wird auf drei Jahre befristet abgeschlossen. Es endet am 31. Oktober 2029, ohne dass es einer Kündigung bedarf. Der Mieter kann nach Ablauf eines Jahres unter Einhaltung einer dreimonatigen Frist kündigen.',
    )
    .heading('§ 3 Mietzins')
    .table(
      ['Position', '', 'monatlich'],
      [
        ['Hauptmietzins', '', '780,00 €'],
        ['Betriebskosten (Akonto)', '', '165,00 €'],
        ['Umsatzsteuer 10 %', '', '94,50 €'],
        ['Gesamtmietzins', '', '1.039,50 €'],
      ],
      [false, false, true],
      true,
    )
    .footer('Mietvertrag Lindengasse 5 / Top 7 · Seite 1 von 2')
    .build();
  const b2 = page()
    .heading('§ 4 Kaution')
    .para(
      'Der Mieter leistet bei Unterzeichnung eine Kaution in Höhe von drei Bruttomonatsmieten. Die Kaution wird auf einem Sparbuch veranlagt und nach Rückgabe des Mietgegenstands samt Zinsen zurückgezahlt.',
    )
    .heading('§ 5 Erhaltung und Rückgabe')
    .para(
      'Der Mieter hat den Mietgegenstand pfleglich zu behandeln. Bei Ende des Mietverhältnisses ist die Wohnung geräumt und gereinigt, unter Berücksichtigung der gewöhnlichen Abnützung, zurückzustellen.',
    )
    .heading('§ 6 Unterschriften')
    .para('Die Vertragsparteien unterzeichnen diesen Vertrag mit qualifizierter elektronischer Signatur.')
    .signature('sig-gruber', MARGIN.left);
  const p2 = b2.footer('Mietvertrag Lindengasse 5 / Top 7 · Seite 2 von 2').build();
  const sigBlock = p2.blocks.find((b) => b.kind === 'signature');
  return {
    id: 'doc-signed',
    fileName: 'Mietvertrag_signiert.pdf',
    folder: '~/Dokumente/Wohnung',
    sizeKb: 311,
    pages: [p1, p2],
    fields: [],
    signatures: [{ ...SIG_GRUBER, rect: sigBlock?.rect ?? null }],
    locked: false,
  };
}

/* ---------------------------------------------------------------- Prüfmodus */

export function verifyDoc(fileName = 'Kaufvertrag_Fahrzeug_signiert.pdf', variant: 'valid' | 'modified' = 'valid'): DocSpec {
  const b = page()
    .title('Kaufvertrag über ein gebrauchtes Kraftfahrzeug')
    .subtitle('Privatverkauf · Vertrag Nr. KV-2026-031')
    .parties(['Verkäufer: Max Mustermann, Mariahilfer Straße 88/4, 1070 Wien', 'Käuferin: Sabine Hofer, Hauptstraße 21, 3100 St. Pölten'])
    .heading('1. Kaufgegenstand')
    .form([
      ['Fahrzeug', 'Kombi, Erstzulassung 03/2021, 58.400 km'],
      ['Fahrgestellnummer', 'WVWZZZ1KZ6W000000 (Beispiel)'],
      ['Kaufpreis', '14.900,00 €'],
    ])
    .heading('2. Gewährleistung')
    .para(
      'Das Fahrzeug wird unter Ausschluss jeglicher Gewährleistung verkauft, wie besichtigt und probegefahren. Der Verkäufer erklärt, dass ihm keine verborgenen Mängel bekannt sind und das Fahrzeug unfallfrei ist.',
    )
    .heading('3. Übergabe')
    .para('Die Übergabe des Fahrzeugs samt Typenschein, zwei Schlüsseln und Serviceheft erfolgt am 20. Oktober 2026 gegen Zahlung des Kaufpreises.')
    .signature('sig-mustermann', MARGIN.left);
  const p = b.footer('Kaufvertrag KV-2026-031 · Seite 1 von 1').build();
  const s1 = p.blocks.find((x) => x.kind === 'signature');
  const second: Block = { kind: 'signature', rect: { x: A4.w - MARGIN.right - 230, y: s1 ? s1.rect.y : 600, w: 230, h: 82 }, signatureId: 'sig-hofer' };
  p.blocks.push(second);
  const signatures: ExistingSignature[] = [
    {
      ...SIG_GRUBER,
      id: 'sig-mustermann',
      signer: 'Max Mustermann',
      role: 'Verkäufer',
      time: '2026-10-14T09:12:44+02:00',
      serial: '1 288 004 912',
      validFrom: '2024-11-20',
      validTo: '2029-11-20',
      page: 0,
      rect: s1?.rect ?? null,
      coversWholeDocument: variant === 'valid',
      status: variant === 'valid' ? 'valid' : 'invalid',
    },
    {
      ...SIG_GRUBER,
      id: 'sig-hofer',
      signer: 'Sabine Hofer',
      role: 'Käuferin',
      time: '2026-10-14T17:40:02+02:00',
      serial: '1 297 551 030',
      validFrom: '2025-06-02',
      validTo: '2030-06-02',
      page: 0,
      rect: second.rect,
      coversWholeDocument: true,
    },
  ];
  return { id: `verify-${fileName}`, fileName, folder: '~/Dokumente/Auto', sizeKb: 188, pages: [p], fields: [], signatures, locked: false };
}

/* ---------------------------------------------------------------- Stapel */

function offerDoc(): DocSpec {
  const p1 = page()
    .letterhead(MUSTER_IT.org, MUSTER_IT.lines)
    .parties(['Stadtwerke Beispielstadt GmbH', 'z. H. Frau Mag. Petra Lechner', 'Hauptplatz 1, 4020 Linz'])
    .title('Angebot Nr. 2026-118')
    .subtitle('Wien, 15. Oktober 2026 · gültig bis 30. November 2026')
    .para(
      'Sehr geehrte Frau Mag. Lechner, vielen Dank für Ihre Anfrage. Gerne bieten wir Ihnen die Erweiterung der Kundenplattform um eine Terminbuchung für Zählertausch und Hausanschlüsse zu folgenden Konditionen an:',
    )
    .table(
      ['Position', 'Menge', 'Betrag (netto)'],
      [
        ['Konzeption und Abstimmung', '3 PT', '2.850,00 €'],
        ['Umsetzung Terminbuchung', '8 PT', '7.600,00 €'],
        ['Test, Abnahme und Schulung', '2 PT', '1.900,00 €'],
        ['Gesamt', '', '12.350,00 €'],
      ],
      [false, true, true],
      true,
    )
    .footer('Angebot 2026-118 · Seite 1 von 2')
    .build();
  const p2 = page()
    .heading('Konditionen')
    .para('Die Preise verstehen sich zuzüglich 20 % Umsatzsteuer. Zahlungsziel 30 Tage netto ab Rechnungsdatum. Es gelten die Bestimmungen des Dienstleistungsvertrags DV-2026-0412.')
    .para('Wir freuen uns auf Ihre Beauftragung und stehen für Rückfragen gerne zur Verfügung.')
    .para('Mit freundlichen Grüßen')
    .para('Muster IT-Dienstleistungen GmbH')
    .footer('Angebot 2026-118 · Seite 2 von 2')
    .build();
  return { id: 'doc-offer', fileName: 'Angebot_2026-118.pdf', folder: '~/Dokumente/Angebote', sizeKb: 96, pages: [p1, p2], fields: [], signatures: [], locked: false };
}

function confirmationDoc(): DocSpec {
  const b = page()
    .letterhead(MUSTER_IT.org, MUSTER_IT.lines)
    .title('Auftragsbestätigung Nr. 4711')
    .subtitle('zu Angebot 2026-097 vom 2. Oktober 2026')
    .para('Wir bestätigen Ihren Auftrag vom 9. Oktober 2026 und erbringen die folgenden Leistungen zu den vereinbarten Konditionen:')
    .table(
      ['Leistung', 'Termin', 'Betrag (netto)'],
      [
        ['Sicherheitsaudit der Plattform', 'KW 45', '3.200,00 €'],
        ['Penetrationstest Schnittstellen', 'KW 46', '2.400,00 €'],
        ['Gesamt', '', '5.600,00 €'],
      ],
      [false, false, true],
      true,
    )
    .para('Bitte bestätigen Sie den Auftrag durch Ihre Signatur im dafür vorgesehenen Feld.');
  const y = b.cursor + 24;
  const p = b.footer('Auftragsbestätigung 4711 · Seite 1 von 1').build();
  const qr: SignatureField = { id: 'qr-1', kind: 'qr', name: 'PDF-AS-POS;id=1', label: 'QR-Code-Platzhalter', page: 0, rect: { x: A4.w - MARGIN.right - 230, y, w: 230, h: 82 } };
  return { id: 'doc-confirmation', fileName: 'Auftragsbestaetigung_4711.pdf', folder: '~/Dokumente/Aufträge', sizeKb: 74, pages: [p], fields: [qr], signatures: [], locked: false };
}

export function payslipDoc(fileName = 'Gehaltszettel_2026-09.pdf', id = 'doc-payslip'): DocSpec {
  const p = page()
    .letterhead('Beispiel Handels GmbH', ['Personalverrechnung · Industriestraße 4 · 4600 Wels'])
    .title('Abrechnung September 2026')
    .form([
      ['Name', 'Max Mustermann'],
      ['Personalnummer', '00417'],
      ['Eintritt', '01.03.2019'],
    ])
    .table(
      ['Lohnart', 'Menge', 'Betrag'],
      [
        ['Gehalt', '', '3.450,00 €'],
        ['Überstunden 50 %', '4 h', '132,80 €'],
        ['Sozialversicherung', '', '− 620,47 €'],
        ['Lohnsteuer', '', '− 498,12 €'],
        ['Auszahlung', '', '2.464,21 €'],
      ],
      [false, true, true],
      true,
    )
    .footer('Vertraulich · Seite 1 von 1')
    .build();
  return { id, fileName, folder: '~/Dokumente/Personal', sizeKb: 184, pages: [p], fields: [], signatures: [], locked: true };
}

export function batchDocs(): DocSpec[] {
  return [offerDoc(), confirmationDoc(), payslipDoc('Lieferschein_Scan.pdf', 'doc-scan'), contractDoc()];
}

/** Zuletzt signierte Dokumente (Startbildschirm). */
export const RECENT_SEED = [
  { id: 'recent-1', fileName: 'Mietvertrag_signiert.pdf', signedAt: '2026-10-13T08:47:00+02:00', signer: 'idaustria' as const, pages: 2 },
  { id: 'recent-2', fileName: 'Angebot_2026-117_signiert.pdf', signedAt: '2026-10-12T11:05:00+02:00', signer: 'idaustria' as const, pages: 2 },
  { id: 'recent-3', fileName: 'Kaufvertrag_Fahrzeug_signiert.pdf', signedAt: '2026-10-14T09:12:00+02:00', signer: 'idaustria' as const, pages: 1 },
];

/** Signatur von Max Mustermann (Demo-Person, die in PIDA signiert) – für „Zuletzt signiert“. */
const SIG_MAX = { ...SIG_GRUBER, signer: 'Max Mustermann', serial: '1 288 004 912', validFrom: '2024-11-20', validTo: '2029-11-20' };

/**
 * Mietvertrag, den Max Mustermann nach Dr. Anna Gruber gegengezeichnet hat („Zuletzt signiert“ →
 * Prüfmodus). So stimmt der Unterzeichner mit der Liste der eigenen Signaturen überein.
 */
export function countersignedLeaseDoc(): DocSpec {
  const base = signedDoc();
  const last = base.pages[1];
  const gruber = base.signatures[0];
  if (!last || !gruber?.rect) return base;
  const rect = { x: A4.w - MARGIN.right - 230, y: gruber.rect.y, w: 230, h: 82 };
  last.blocks.push({ kind: 'signature', rect, signatureId: 'sig-max-lease' });
  return {
    ...base,
    id: 'verify-lease',
    signatures: [gruber, { ...SIG_MAX, id: 'sig-max-lease', role: 'Mieter', time: '2026-10-13T08:47:12+02:00', page: 1, rect }],
  };
}

/** Angebot mit einer Signatur, nach der das Dokument verändert wurde (Prüfmodus: ungültig). */
export function signedOfferDoc(): DocSpec {
  const base = offerDoc();
  // Nummer passend zum Dateinamen „Angebot_2026-117_signiert.pdf“
  for (const p of base.pages) {
    p.blocks = p.blocks.map((b) => (b.kind === 'title' || b.kind === 'footer' ? { ...b, text: b.text.replace('2026-118', '2026-117') } : b));
  }
  const last = base.pages[1];
  if (!last) return base;
  const body = last.blocks.filter((b) => b.kind !== 'footer');
  const bottom = body.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0);
  const rect = { x: MARGIN.left, y: bottom + 14, w: 230, h: 82 };
  last.blocks.push({ kind: 'signature', rect, signatureId: 'sig-bauer' });
  return {
    ...base,
    id: 'verify-offer',
    fileName: 'Angebot_2026-117_signiert.pdf',
    signatures: [
      {
        ...SIG_GRUBER,
        ...SIG_MAX,
        id: 'sig-bauer',
        role: 'Muster IT-Dienstleistungen GmbH',
        time: '2026-10-12T11:05:31+02:00',
        status: 'invalid',
        page: 1,
        rect,
        coversWholeDocument: false,
      },
    ],
  };
}

/** Prüfdokument zu einem Eintrag der Liste „Zuletzt signiert“. */
export function verifySpecForRecent(recentId: string): DocSpec {
  if (recentId === 'recent-1') return countersignedLeaseDoc();
  if (recentId === 'recent-2') return signedOfferDoc();
  return verifyDoc();
}
