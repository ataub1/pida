// Signieren mit QR-Platzhaltersuche über die PDF-AS-4.4.2-Bibliothek (nur Testwerkzeug).
//
// Die PDF-AS-CLI (`at.gv.egiz.pdfas.cli.Main`) bietet keine Option für die Platzhaltersuche:
// `SignParameter.isPlaceHolderSearchEnabled()` ist dort immer false. Dieses Programm ist eine
// Kopie von `Main.perform_sign` mit Keystore-Connector (`-c ks`) und ergänzt genau das, was
// PDF-Over in `PdfAs4Signer.prepare` setzt:
//   param.setPlaceHolderSearchEnabled(true);
//   config.setValue("placeholder_mode", "1");
//   config.setValue("enable_placeholder_search", "true");
//   param.setPlaceHolderId(<id>)   (falls angegeben)
//
// Aufruf: java -cp "<pdf-as-cli>/lib/*" tools/golden/PlaceholderSign.java \
//   <cfg-root> <profil> <position|-> <keystore.p12> <alias> <passwort> <platzhalter-id|-> <ein.pdf> <aus.pdf>

import at.gv.egiz.pdfas.common.utils.StreamUtils;
import at.gv.egiz.pdfas.lib.api.ByteArrayDataSource;
import at.gv.egiz.pdfas.lib.api.Configuration;
import at.gv.egiz.pdfas.lib.api.PdfAs;
import at.gv.egiz.pdfas.lib.api.PdfAsFactory;
import at.gv.egiz.pdfas.lib.api.sign.SignParameter;
import at.gv.egiz.pdfas.sigs.pades.PAdESSignerKeystore;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.HashMap;
import java.util.UUID;
import javax.activation.DataSource;

public class PlaceholderSign {
    public static void main(String[] args) throws Exception {
        if (args.length != 9) {
            System.err.println("Aufruf: PlaceholderSign <cfg> <profil> <pos|-> <p12> <alias> <pw> <id|-> <ein> <aus>");
            System.exit(2);
        }
        String cfg = args[0];
        String profile = args[1];
        String pos = "-".equals(args[2]) ? null : args[2];
        String ks = args[3];
        String alias = args[4];
        String pw = args[5];
        String placeholderId = "-".equals(args[6]) ? null : args[6];
        File in = new File(args[7]);
        File out = new File(args[8]);

        DataSource dataSource;
        try (FileInputStream fis = new FileInputStream(in)) {
            dataSource = new ByteArrayDataSource(StreamUtils.inputStreamToByteArray(fis));
        }
        PdfAs pdfAs = PdfAsFactory.createPdfAs(new File(cfg));
        Configuration configuration = pdfAs.getConfiguration();
        try (FileOutputStream fos = new FileOutputStream(out, false)) {
            SignParameter p = PdfAsFactory.createSignParameter(configuration, dataSource, fos);
            p.setTransactionId(UUID.randomUUID().toString());
            p.setPlainSigner(new PAdESSignerKeystore(ks, alias, pw, pw, "PKCS12"));
            p.setDataSource(dataSource);
            p.setSignaturePosition(pos);
            p.setSignatureProfileId(profile);
            p.setDynamicSignatureBlockArguments(new HashMap<>());
            // wie PDF-Over (PdfAs4Signer.prepare)
            p.setPlaceHolderSearchEnabled(true);
            p.getConfiguration().setValue("placeholder_mode", "1");
            p.getConfiguration().setValue("enable_placeholder_search", "true");
            if (placeholderId != null) {
                p.setPlaceHolderId(placeholderId);
            }
            pdfAs.sign(p);
        }
        System.out.println("Signed document " + out);
    }
}
