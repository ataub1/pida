"""Generates crates/sigblock/src/afm_data.rs from the Adobe Core14 AFM files (WinAnsi widths only)."""
import sys

AFMDIR = sys.argv[1]
OUT = sys.argv[2]
FACES = [
    ('HELVETICA', 'Helvetica'),
    ('HELVETICA_BOLD', 'Helvetica-Bold'),
    ('COURIER', 'Courier'),
    ('COURIER_BOLD', 'Courier-Bold'),
    ('TIMES_ROMAN', 'Times-Roman'),
    ('TIMES_BOLD', 'Times-Bold'),
    ('TIMES_ITALIC', 'Times-Italic'),
]


WINANSI = {
 0x20:'space',0x21:'exclam',0x22:'quotedbl',0x23:'numbersign',0x24:'dollar',0x25:'percent',0x26:'ampersand',0x27:'quotesingle',
 0x28:'parenleft',0x29:'parenright',0x2A:'asterisk',0x2B:'plus',0x2C:'comma',0x2D:'hyphen',0x2E:'period',0x2F:'slash',
 0x30:'zero',0x31:'one',0x32:'two',0x33:'three',0x34:'four',0x35:'five',0x36:'six',0x37:'seven',0x38:'eight',0x39:'nine',
 0x3A:'colon',0x3B:'semicolon',0x3C:'less',0x3D:'equal',0x3E:'greater',0x3F:'question',0x40:'at',
 0x5B:'bracketleft',0x5C:'backslash',0x5D:'bracketright',0x5E:'asciicircum',0x5F:'underscore',0x60:'grave',
 0x7B:'braceleft',0x7C:'bar',0x7D:'braceright',0x7E:'asciitilde',
 0x80:'Euro',0x82:'quotesinglbase',0x83:'florin',0x84:'quotedblbase',0x85:'ellipsis',0x86:'dagger',0x87:'daggerdbl',0x88:'circumflex',
 0x89:'perthousand',0x8A:'Scaron',0x8B:'guilsinglleft',0x8C:'OE',0x8E:'Zcaron',0x91:'quoteleft',0x92:'quoteright',0x93:'quotedblleft',
 0x94:'quotedblright',0x95:'bullet',0x96:'endash',0x97:'emdash',0x98:'tilde',0x99:'trademark',0x9A:'scaron',0x9B:'guilsinglright',
 0x9C:'oe',0x9E:'zcaron',0x9F:'Ydieresis',
 0xA0:'space',0xA1:'exclamdown',0xA2:'cent',0xA3:'sterling',0xA4:'currency',0xA5:'yen',0xA6:'brokenbar',0xA7:'section',0xA8:'dieresis',
 0xA9:'copyright',0xAA:'ordfeminine',0xAB:'guillemotleft',0xAC:'logicalnot',0xAD:'hyphen',0xAE:'registered',0xAF:'macron',0xB0:'degree',
 0xB1:'plusminus',0xB2:'twosuperior',0xB3:'threesuperior',0xB4:'acute',0xB5:'mu',0xB6:'paragraph',0xB7:'periodcentered',0xB8:'cedilla',
 0xB9:'onesuperior',0xBA:'ordmasculine',0xBB:'guillemotright',0xBC:'onequarter',0xBD:'onehalf',0xBE:'threequarters',0xBF:'questiondown',
 0xC0:'Agrave',0xC1:'Aacute',0xC2:'Acircumflex',0xC3:'Atilde',0xC4:'Adieresis',0xC5:'Aring',0xC6:'AE',0xC7:'Ccedilla',0xC8:'Egrave',
 0xC9:'Eacute',0xCA:'Ecircumflex',0xCB:'Edieresis',0xCC:'Igrave',0xCD:'Iacute',0xCE:'Icircumflex',0xCF:'Idieresis',0xD0:'Eth',0xD1:'Ntilde',
 0xD2:'Ograve',0xD3:'Oacute',0xD4:'Ocircumflex',0xD5:'Otilde',0xD6:'Odieresis',0xD7:'multiply',0xD8:'Oslash',0xD9:'Ugrave',0xDA:'Uacute',
 0xDB:'Ucircumflex',0xDC:'Udieresis',0xDD:'Yacute',0xDE:'Thorn',0xDF:'germandbls',0xE0:'agrave',0xE1:'aacute',0xE2:'acircumflex',
 0xE3:'atilde',0xE4:'adieresis',0xE5:'aring',0xE6:'ae',0xE7:'ccedilla',0xE8:'egrave',0xE9:'eacute',0xEA:'ecircumflex',0xEB:'edieresis',
 0xEC:'igrave',0xED:'iacute',0xEE:'icircumflex',0xEF:'idieresis',0xF0:'eth',0xF1:'ntilde',0xF2:'ograve',0xF3:'oacute',0xF4:'ocircumflex',
 0xF5:'otilde',0xF6:'odieresis',0xF7:'divide',0xF8:'oslash',0xF9:'ugrave',0xFA:'uacute',0xFB:'ucircumflex',0xFC:'udieresis',0xFD:'yacute',
 0xFE:'thorn',0xFF:'ydieresis',
}
for c in range(0x41, 0x5B): WINANSI[c] = chr(c)
for c in range(0x61, 0x7B): WINANSI[c] = chr(c)


def winansi_names():
    names = {}
    for code, name in WINANSI.items():
        uv = ord(bytes([code]).decode('cp1252'))
        names[code] = (name, uv)
    return names


def parse_afm(path):
    widths = {}
    header = {}
    in_metrics = False
    for line in open(path, encoding='latin-1'):
        line = line.strip()
        if line.startswith('StartCharMetrics'):
            in_metrics = True
            continue
        if line.startswith('EndCharMetrics'):
            in_metrics = False
            continue
        if in_metrics:
            parts = [p.strip() for p in line.split(';')]
            wx = None
            name = None
            for p in parts:
                if p.startswith('WX '):
                    wx = int(float(p[3:]))
                elif p.startswith('N '):
                    name = p[2:]
            if name is not None:
                widths[name] = wx
        else:
            k, _, v = line.partition(' ')
            header[k] = v
    return widths, header


names = winansi_names()
out = []
out.append('//! Standard-14-Metriken (generiert, nicht von Hand bearbeiten).')
out.append('//!')
out.append('//! Abgeleitet aus den Adobe Core 14 AFM Files (Copyright (c) 1985, 1987, 1989, 1990, 1991, 1992,')
out.append('//! 1993, 1997 Adobe Systems Incorporated. All Rights Reserved), Lizenzhinweis in')
out.append('//! `assets/licenses/Adobe-Core14-AFM-License.html`. **Änderung:** Es wurden nur die Breiten (`WX`) der')
out.append('//! Zeichen der WinAnsiEncoding (Code 0xA0 = `space`, 0xAD = `hyphen` wie PDFBox) sowie Ascender,')
out.append('//! Descender, CapHeight, XHeight und FontBBox übernommen und als Rust-Tabellen abgelegt;')
out.append('//! `0` = Code nicht belegt. Kerning-Paare entfallen (PDF-AS misst ohne Kerning).')
out.append('')
for const, fname in FACES:
    widths, header = parse_afm(f'{AFMDIR}/{fname}.afm')
    table = [0] * 256
    for code, (name, uv) in names.items():
        if name not in widths:
            raise SystemExit(f'{fname}: missing glyph {name}')
        table[code] = widths[name]
    bbox = [int(x) for x in header['FontBBox'].split()]
    out.append(f'/// `{fname}`: Breiten je WinAnsi-Code.')
    out.append(f'pub(crate) const {const}_WIDTHS: [u16; 256] = [')
    for i in range(0, 256, 16):
        out.append('    ' + ', '.join(str(w) for w in table[i:i + 16]) + ',')
    out.append('];')
    asc = header.get('Ascender', '0')
    desc = header.get('Descender', '0')
    cap = header.get('CapHeight', '0')
    xh = header.get('XHeight', '0')
    out.append(f'/// `{fname}`: Ascender, Descender, CapHeight, XHeight, FontBBox.')
    out.append(f'pub(crate) const {const}_METRICS: super::AfmMetrics = super::AfmMetrics {{ ascender: {asc}, descender: {desc}, cap_height: {cap}, x_height: {xh}, bbox: [{bbox[0]}, {bbox[1]}, {bbox[2]}, {bbox[3]}] }};')
    out.append('')

# unicode -> code table for 0x80..0x9F
out.append('/// Unicode → WinAnsi-Code für den Bereich 0x80–0x9F (CP1252).')
out.append('pub(crate) const WINANSI_HIGH: [(char, u8); %d] = [' % len([c for c in names if 0x80 <= c < 0xA0]))
for code in sorted(names):
    if 0x80 <= code < 0xA0:
        out.append(f"    ('\\u{{{names[code][1]:04X}}}', 0x{code:02X}),")
out.append('];')
open(OUT, 'w').write('\n'.join(out) + '\n')
print('ok', len(names))
