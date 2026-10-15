// Orakel für pida-java-properties (nur Tests): liest Fälle aus einer Datei (Trenner: Zeile
// "=====CASE=====", Inhalt Latin-1-Bytes), lädt jeden mit Properties.load(InputStream) und gibt die
// Einträge in Einfügereihenfolge des JDK nicht wieder (Hashtable), sondern sortiert aus.
// Aufruf: java PropsOracle.java <fälle-datei>
// Ausgabe: "CASE\t<n>", dann "E\t<key>\t<value>" bzw. "ERR\t<Ausnahmeklasse>"; danach "STORE\t<escaped>"
// = Properties.store(OutputStream, null) ohne Datumszeile.
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Properties;

public class PropsOracle {
    public static void main(String[] args) throws Exception {
        byte[] all = Files.readAllBytes(Path.of(args[0]));
        String text = new String(all, StandardCharsets.ISO_8859_1);
        String[] cases = text.split("=====CASE=====\n", -1);
        for (int i = 1; i < cases.length; i++) {
            System.out.println("CASE\t" + i);
            Properties p = new Properties();
            try {
                p.load(new ByteArrayInputStream(cases[i].getBytes(StandardCharsets.ISO_8859_1)));
            } catch (Exception e) {
                System.out.println("ERR\t" + e.getClass().getSimpleName());
                continue;
            }
            List<String> keys = new ArrayList<>(p.stringPropertyNames());
            Collections.sort(keys);
            for (String k : keys) System.out.println("E\t" + esc(k) + "\t" + esc(p.getProperty(k)));
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            p.store(out, null);
            String stored = out.toString(StandardCharsets.ISO_8859_1);
            // erste Zeile ist das Datum
            stored = stored.substring(stored.indexOf('\n') + 1);
            System.out.println("STORE\t" + esc(stored));
        }
    }

    static String esc(String s) {
        StringBuilder sb = new StringBuilder();
        for (char c : s.toCharArray()) {
            if (c == '\\') sb.append("\\\\");
            else if (c == '\t') sb.append("\\t");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c < 0x20 || c > 0x7e) sb.append(String.format("\\u%04X", (int) c));
            else sb.append(c);
        }
        return sb.toString();
    }
}
