// Orakel für pdfover-i18n (nur Tests): liest .properties-Dateien wie PDF-Over (ResourceBundle,
// Java >= 9: UTF-8, Rückfall ISO-8859-1) und gibt alle Einträge sortiert und ASCII-escaped aus.
// Aufruf: java DumpBundle.java <datei>...
// Ausgabe je Datei: "FILE\t<pfad>", danach je Key
//   "E\t<key>\t<value>"      – Rohwert wie Messages.getString
//   "F\t<key>\t<formatted>"  – String.format(Locale.ROOT, value, args) wie Messages.formatString,
//                               mit "<n>" für %s und 1000+n für %d
// (Backslash, Tab, Zeilenumbrüche und Nicht-ASCII-Zeichen escaped, Unicode als Backslash-u-XXXX).
import java.io.FileInputStream;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.PropertyResourceBundle;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class DumpBundle {
    private static final Pattern PH = Pattern.compile("%%|%([sd])");

    public static void main(String[] args) throws Exception {
        for (String file : args) {
            PropertyResourceBundle bundle;
            try (InputStream in = new FileInputStream(file)) {
                bundle = new PropertyResourceBundle(in);
            }
            List<String> keys = new ArrayList<>(bundle.keySet());
            Collections.sort(keys);
            System.out.println("FILE\t" + file);
            for (String k : keys) {
                String v = bundle.getString(k);
                System.out.println("E\t" + esc(k) + "\t" + esc(v));
                List<Object> fmtArgs = new ArrayList<>();
                Matcher m = PH.matcher(v);
                while (m.find()) {
                    if (m.group(1) == null) continue;
                    int n = fmtArgs.size() + 1;
                    fmtArgs.add(m.group(1).equals("s") ? (Object) ("<" + n + ">") : (Object) Integer.valueOf(1000 + n));
                }
                System.out.println("F\t" + esc(k) + "\t" + esc(String.format(Locale.ROOT, v, fmtArgs.toArray())));
            }
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
