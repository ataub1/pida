// PIDA – Test-Orakel (nur Tests, nie im Produkt): bettet Bildmarken genau wie PDF-AS 4.4.2
// ein (PDFAsVisualSignatureBuilder, Profil ohne PDF/A: ImageIO.read → removeAlphaChannel, falls
// eine Transparenz ohne Alpha-Raster vorliegt → PDFBox LosslessFactory.createFromImage) und gibt
// je Datei Farbraum, Bits, Samples und SMask aus. Aufruf durch
// crates/sigblock/src/appearance.rs (Test `pdfas_lossless_factory_oracle`):
//   java -cp "<PDF-AS-lib>/*" PdfAsEmblem.java a.png b.png …
// Ausgabe je Datei eine Zeile: Name|Farbraum|Komponenten|Bits|Breite|Höhe|Samples-Hex|SMask-Hex
import java.awt.image.BufferedImage;
import java.io.File;
import javax.imageio.ImageIO;
import org.apache.pdfbox.cos.COSBase;
import org.apache.pdfbox.cos.COSName;
import org.apache.pdfbox.cos.COSStream;
import org.apache.pdfbox.pdmodel.PDDocument;
import org.apache.pdfbox.pdmodel.common.PDStream;
import org.apache.pdfbox.pdmodel.graphics.image.LosslessFactory;
import org.apache.pdfbox.pdmodel.graphics.image.PDImageXObject;

public class PdfAsEmblem {
  static String hex(byte[] b) {
    StringBuilder s = new StringBuilder();
    for (byte x : b) s.append(String.format("%02x", x));
    return s.toString();
  }

  public static void main(String[] args) throws Exception {
    try (PDDocument doc = new PDDocument()) {
      for (String f : args) {
        BufferedImage img = ImageIO.read(new File(f));
        if (img.getAlphaRaster() == null && img.getColorModel().hasAlpha()) {
          img = at.gv.egiz.pdfas.common.utils.ImageUtils.removeAlphaChannel(img);
        }
        PDImageXObject x = LosslessFactory.createFromImage(doc, img);
        COSBase sm = x.getCOSObject().getDictionaryObject(COSName.SMASK);
        String smask = "-";
        if (sm instanceof COSStream) {
          smask = hex(new PDImageXObject(new PDStream((COSStream) sm), null).getStream().toByteArray());
        }
        System.out.printf("%s|%s|%d|%d|%d|%d|%s|%s%n", new File(f).getName(), x.getColorSpace().getName(),
            x.getColorSpace().getNumberOfComponents(), x.getBitsPerComponent(), x.getWidth(), x.getHeight(),
            hex(x.getStream().toByteArray()), smask);
      }
    }
  }
}
