"""Erzeugt kleine Bild-Fixtures für die Emblem-Tests (testdata/sigblock/images/).

Aufruf (mit Pillow, z. B. aus der .venv des Hauptcheckouts):
    python make_image_fixtures.py <ziel-ordner>

Alle Bilder sind synthetisch (Farbflächen), ohne Personendaten.
"""
import sys
from pathlib import Path

from PIL import Image

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)


def two_tone(w, h, left, right, mode='RGB'):
    img = Image.new(mode, (w, h), left)
    for y in range(h):
        for x in range(w // 2, w):
            img.putpixel((x, y), right)
    return img


# JPEG 40×20, links rot, rechts blau, EXIF-Orientierung 6 (90° im UZS anzeigen)
img = two_tone(40, 20, (255, 0, 0), (0, 0, 255))
exif = Image.Exif()
exif[0x0112] = 6
img.save(out / 'exif6.jpg', quality=95, exif=exif.tobytes())
# dasselbe ohne EXIF
img.save(out / 'plain.jpg', quality=95)
# PNG mit eXIf-Orientierung 6: PDF-Over wertet EXIF nur bei JPEG aus
img.save(out / 'exif6.png', exif=exif.tobytes())

# GIF 10×10 mit transparentem Index (linke Hälfte transparent)
gif = Image.new('P', (10, 10), 0)
gif.putpalette([255, 255, 255, 0, 128, 0] + [0] * (254 * 3))
for y in range(10):
    for x in range(5, 10):
        gif.putpixel((x, y), 1)
gif.save(out / 'transparent.gif', transparency=0)

# großes PNG 1000×1500 (wird auf 400×600 verkleinert)
Image.new('RGB', (1000, 1500), (10, 200, 30)).save(out / 'large.png')
print('ok')
