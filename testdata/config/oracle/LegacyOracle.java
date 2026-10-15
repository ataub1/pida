// Orakel für pida-config (nur Tests, kein Teil des Produkts).
//
// Nachbau der Lese-Regeln von PDF-Over 4.4.x (ConfigurationManager.loadFromDisk,
// ConfigurationDataInMemory, MobileBKUValidator.normalizeMobileNumber, LocaleSerializer) mit den
// echten Java-Bibliotheksfunktionen (Properties.load, String.trim/equalsIgnoreCase, Integer.parseInt,
// Double.parseDouble, Locale.getAvailableLocales). Existenzprüfungen von Dateien entfallen.
//
// Aufruf: java LegacyOracle.java [--latin1|--utf8] <datei>...   (Standard: --latin1 wie PDF-Over;
//         --utf8 liest über einen UTF-8-Reader und entfernt ein BOM, wie die Heuristik von PIDA)
//         java LegacyOracle.java --locales                       (verfügbare Locales, eine je Zeile)
// Ausgabe je Datei: "FILE\t<pfad>", dann "RAW\t<key>\t<wert>" und "VAL\t<name>\t<wert>".
// Werte ASCII-escaped (Backslash, Tab, Zeilenumbruch, Nicht-ASCII als Backslash-u-XXXX); fehlend = <null>.
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Properties;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class LegacyOracle {
    public static void main(String[] args) throws Exception {
        boolean utf8 = false;
        for (String a : args) {
            if (a.equals("--locales")) {
                TreeSet<String> s = new TreeSet<>();
                for (Locale l : Locale.getAvailableLocales()) if (!l.toString().isEmpty()) s.add(l.toString());
                for (String x : s) System.out.println(x);
                continue;
            }
            if (a.equals("--utf8")) { utf8 = true; continue; }
            if (a.equals("--latin1")) { utf8 = false; continue; }
            Properties p = new Properties();
            try (InputStream in = new FileInputStream(a)) {
                if (utf8) {
                    String s = new String(in.readAllBytes(), StandardCharsets.UTF_8);
                    if (!s.isEmpty() && s.charAt(0) == 0xFEFF) s = s.substring(1);
                    p.load(new StringReader(s));
                } else {
                    p.load(in);
                }
            }
            System.out.println("FILE\t" + a);
            dump(p);
        }
    }

    static void out(String kind, String name, String value) {
        System.out.println(kind + "\t" + esc(name) + "\t" + (value == null ? "<null>" : esc(value)));
    }

    static void val(String name, String value) { out("VAL", name, value); }

    static String blankToNull(String s) { return (s == null || s.trim().isEmpty()) ? null : s; }

    static String blankToEmpty(String s) { return (s == null || s.trim().isEmpty()) ? "" : s; }

    static String lang(String s) {
        if (s == null || s.isEmpty()) return null;
        for (Locale l : Locale.getAvailableLocales()) if (l.toString().equals(s)) return l.getLanguage();
        return null;
    }

    static final String NUMBER_REGEX = "^((\\+[\\d]{2})|(00[\\d]{2})|(0)|(10301))([1-9][\\d]+)$";

    static String normalize(String number) {
        number = number.trim();
        String numberWithoutWhitespace = number.replaceAll("\\s", "");
        Matcher matcher = Pattern.compile(NUMBER_REGEX).matcher(numberWithoutWhitespace);
        if (!matcher.find()) return number;
        if (matcher.groupCount() != 6) return number;
        String countryCode = matcher.group(1);
        String normalNumber = matcher.group(6);
        if (countryCode.equals("10301")) return numberWithoutWhitespace;
        countryCode = countryCode.replace("00", "+");
        if (countryCode.equals("0")) countryCode = "+43";
        return countryCode + normalNumber;
    }

    static void dump(Properties d) {
        List<String> keys = new ArrayList<>(d.stringPropertyNames());
        Collections.sort(keys);
        for (String k : keys) out("RAW", k, d.getProperty(k));

        val("EMBLEM", blankToNull(d.getProperty("EMBLEM")));

        double logo = 23;
        try {
            String s = d.getProperty("LOGO_ONLY_SIZE");
            if (s != null) logo = Double.parseDouble(s);
        } catch (NumberFormatException e) { /* ignoriert wie PDF-Over */ }
        val("LOGO_ONLY_SIZE", Double.toString(logo));

        String mn = d.getProperty("MOBILE_NUMBER");
        val("MOBILE_NUMBER", (mn == null || mn.trim().isEmpty()) ? null : normalize(mn));
        val("MOBILE_REMEMBER_PASSWORD", String.valueOf("true".equals(d.getProperty("MOBILE_REMEMBER_PASSWORD"))));

        val("PROXY_HOST", blankToNull(d.getProperty("PROXY_HOST")));
        val("PROXY_USER", blankToNull(d.getProperty("PROXY_USER")));
        val("PROXY_PASS", blankToNull(d.getProperty("PROXY_PASS")));
        val("OUTPUT_FOLDER", blankToNull(d.getProperty("OUTPUT_FOLDER")));

        String postFix = d.getProperty("SAVE_FILE_POSTFIX");
        val("SAVE_FILE_POSTFIX", postFix == null ? "_signed" : postFix);

        val("LOCALE", lang(d.getProperty("LOCALE")));
        val("SIGNLOCALE", lang(d.getProperty("SIGNLOCALE")));

        boolean marker = false, fields = false;
        String useMarker = d.getProperty("USE_MARKER");
        if (useMarker != null) {
            marker = useMarker.equalsIgnoreCase("true");
            if (marker) fields = false;
        }
        String useFields = d.getProperty("USE_FIELDS");
        if (useFields != null) {
            fields = useFields.equalsIgnoreCase("true");
            if (fields) marker = false;
        }
        val("USE_MARKER", String.valueOf(marker));
        val("USE_FIELDS", String.valueOf(fields));
        String ph = d.getProperty("USE_PLACEHOLDER_SEARCH");
        val("USE_PLACEHOLDER_SEARCH", String.valueOf(ph != null && ph.equalsIgnoreCase("true")));

        String prof = d.getProperty("SIGNATURE_PROFILE");
        List<String> profiles = Arrays.asList("SIGNATURBLOCK_SMALL", "AMTSSIGNATURBLOCK", "BASE_LOGO", "INVISIBLE");
        val("SIGNATURE_PROFILE", prof != null && profiles.contains(prof) ? prof : "SIGNATURBLOCK_SMALL");

        if (d.containsKey("SIGNATURE_NOTE")) {
            String n = d.getProperty("SIGNATURE_NOTE");
            val("SIGNATURE_NOTE", (n == null || n.trim().isEmpty()) ? "<EMPTY>" : n);
        } else {
            val("SIGNATURE_NOTE", "<MISSING>");
        }

        String compat = d.getProperty("SIGNATURE_PDFA_COMPAT");
        val("SIGNATURE_PDFA_COMPAT", String.valueOf(compat != null && compat.equalsIgnoreCase("true")));

        String pp = d.getProperty("PROXY_PORT");
        String port = "-1";
        if (pp != null && !pp.trim().isEmpty()) {
            try {
                int x = Integer.parseInt(pp);
                port = (x > 0 && x <= 0xFFFF) ? String.valueOf(x) : "<RANGE>";
            } catch (NumberFormatException e) {
                port = "<ERROR>";
            }
        }
        val("PROXY_PORT", port);

        String bku = d.getProperty("BKU");
        String b = "NONE";
        if (bku != null && Arrays.asList("LOCAL", "MOBILE", "KS", "NONE").contains(bku)) b = bku;
        val("BKU", b);

        int width = 600, height = 780;
        String size = d.getProperty("MAINWINDOW_SIZE");
        parse: {
            if (size == null) break parse;
            int pos = size.indexOf(',');
            if (pos <= 0) break parse;
            try {
                width = Integer.parseInt(size.substring(0, pos).trim());
                height = Integer.parseInt(size.substring(pos + 1).trim());
            } catch (NumberFormatException e) { /* ignoriert */ }
        }
        val("MAINWINDOW_SIZE", width + "," + height);

        String sp = d.getProperty("SIGNATURE_POSITION");
        val("SIGNATURE_POSITION", String.valueOf(sp != null && sp.trim().equals("auto")));

        String ke = d.getProperty("KEYSTORE_ENABLED");
        val("KEYSTORE_ENABLED", String.valueOf(ke != null && ke.equalsIgnoreCase("true")));
        val("KEYSTORE_FILE", blankToEmpty(d.getProperty("KEYSTORE_FILE")));
        val("KEYSTORE_TYPE", blankToEmpty(d.getProperty("KEYSTORE_TYPE")));
        val("KEYSTORE_ALIAS", blankToEmpty(d.getProperty("KEYSTORE_ALIAS")));
        String storePass = d.getProperty("KEYSTORE_STOREPASS");
        String keyPass = d.getProperty("KEYSTORE_KEYPASS");
        val("KEYSTORE_STOREPASS", storePass);
        val("KEYSTORE_KEYPASS", keyPass);
        String storeType = d.getProperty("KEYSTORE_PASS_STORE_TYPE");
        if (storeType == null) {
            if ((keyPass != null && !keyPass.trim().isEmpty()) || (storePass != null && !storePass.trim().isEmpty()))
                storeType = "disk";
            else
                storeType = "memory";
        }
        val("KEYSTORE_PASS_STORE_TYPE", "disk".equals(storeType) ? "DISK" : "memory".equals(storeType) ? "MEMORY" : "NONE");

        boolean updateCheck = true;
        String uc = d.getProperty("UPDATE_CHECK");
        if (uc != null) updateCheck = !uc.equalsIgnoreCase("false");
        val("UPDATE_CHECK", String.valueOf(updateCheck));

        val("FIDO2_DEFAULT", String.valueOf("true".equalsIgnoreCase(d.getProperty("FIDO2_DEFAULT"))));
        val("LAST_SOURCE_DIRECTORY", d.getProperty("LAST_SOURCE_DIRECTORY"));
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
