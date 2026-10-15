// QR-Modulmatrix für Platzhalter-Testdokumente (nur Testwerkzeug, Java nur als Orakel/Generator).
//
// Aufruf (Single-File-Source-Modus, ZXing aus der PDF-AS-4.4.2-CLI-Distribution):
//   java -cp <pdf-as-cli>/lib/core-3.5.0.jar tools/golden/MakeQr.java "PDF-AS-POS;id=1"
// Ausgabe: eine Zeile je Modulzeile aus '0' (hell) und '1' (dunkel), ohne Ruhezone,
// Fehlerkorrektur L wie der PDF-AS-Platzhalter-Generator (QRCodeGenerator).

import com.google.zxing.BarcodeFormat;
import com.google.zxing.EncodeHintType;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel;
import java.util.EnumMap;
import java.util.Map;

public class MakeQr {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("Aufruf: MakeQr <Text>");
            System.exit(2);
        }
        Map<EncodeHintType, Object> hints = new EnumMap<>(EncodeHintType.class);
        hints.put(EncodeHintType.ERROR_CORRECTION, ErrorCorrectionLevel.L);
        hints.put(EncodeHintType.MARGIN, 0);
        hints.put(EncodeHintType.CHARACTER_SET, "UTF-8");
        BitMatrix m = new QRCodeWriter().encode(args[0], BarcodeFormat.QR_CODE, 0, 0, hints);
        StringBuilder sb = new StringBuilder();
        for (int y = 0; y < m.getHeight(); y++) {
            for (int x = 0; x < m.getWidth(); x++) {
                sb.append(m.get(x, y) ? '1' : '0');
            }
            sb.append('\n');
        }
        System.out.print(sb);
    }
}
