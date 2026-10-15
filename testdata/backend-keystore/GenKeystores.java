// PIDA, Paket B – erzeugt Java-spezifische Test-Keystores (nur von make-testdata.sh aufgerufen).
// NUR FÜR TESTS: alle Schlüssel stammen aus testdata/keystores bzw. testdata/cades, Passwort "test".
//
//   java [-Dkeystore.pkcs12.…] GenKeystores.java <testdata-Verzeichnis> <Ausgabeverzeichnis> <modus>
//
// modus = main (getrennte Passwörter, zwei Schlüssel, falscher Schlüssel, JKS, JCEKS) oder
// export:<datei>:<alias> (test-rsa2048 als PKCS#12 mit den Voreinstellungen der JVM bzw. -D…).
// keytool verlangt Passwörter ≥ 6 Zeichen, die KeyStore-API nicht – daher dieses Programm.
import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.security.cert.*;

public class GenKeystores {
    static KeyStore.PrivateKeyEntry load(Path p12, String alias) throws Exception {
        KeyStore ks = KeyStore.getInstance("PKCS12");
        try (InputStream in = Files.newInputStream(p12)) {
            ks.load(in, "test".toCharArray());
        }
        return (KeyStore.PrivateKeyEntry) ks.getEntry(alias, new KeyStore.PasswordProtection("test".toCharArray()));
    }

    static void store(KeyStore ks, Path out, String storePass) throws Exception {
        try (OutputStream o = Files.newOutputStream(out)) {
            ks.store(o, storePass.toCharArray());
        }
    }

    public static void main(String[] args) throws Exception {
        Path td = Paths.get(args[0]);
        Path out = Paths.get(args[1]);
        KeyStore.PrivateKeyEntry rsa = load(td.resolve("keystores/test-rsa2048.p12"), "test-rsa2048");
        KeyStore.PrivateKeyEntry ec = load(td.resolve("keystores/test-ecp256.p12"), "test-ecp256");
        if (args[2].startsWith("export:")) {
            String[] parts = args[2].split(":", 3);
            KeyStore ks = KeyStore.getInstance("PKCS12");
            ks.load(null, null);
            ks.setKeyEntry(parts[2], rsa.getPrivateKey(), "test".toCharArray(), rsa.getCertificateChain());
            store(ks, out.resolve(parts[1]), "test");
            return;
        }

        // 1. Getrennte Passwörter: Store "test", Schlüssel "schluessel" (Java kann das, p12-keystore nicht)
        KeyStore sep = KeyStore.getInstance("PKCS12");
        sep.load(null, null);
        sep.setKeyEntry("getrennt", rsa.getPrivateKey(), "schluessel".toCharArray(), rsa.getCertificateChain());
        store(sep, out.resolve("java-separate-keypass.p12"), "test");

        // 2. Zwei Schlüsseleinträge + ein vertrauenswürdiges Zertifikat
        KeyStore two = KeyStore.getInstance("PKCS12");
        two.load(null, null);
        two.setKeyEntry("erster", rsa.getPrivateKey(), "test".toCharArray(), rsa.getCertificateChain());
        two.setKeyEntry("zweiter", ec.getPrivateKey(), "test".toCharArray(), ec.getCertificateChain());
        CertificateFactory cf = CertificateFactory.getInstance("X.509");
        java.security.cert.Certificate root;
        try (InputStream in = Files.newInputStream(td.resolve("cades/ca-root.crt.der"))) {
            root = cf.generateCertificate(in);
        }
        two.setCertificateEntry("vertrauensanker", root);
        store(two, out.resolve("java-two-keys.p12"), "test");

        // 3. Schlüssel passt nicht zum Zertifikat (RSA-2048-Schlüssel, RSA-3072-Zertifikat)
        java.security.cert.Certificate other;
        try (InputStream in = Files.newInputStream(td.resolve("cades/leaf-rsa3072.crt.der"))) {
            other = cf.generateCertificate(in);
        }
        KeyStore mism = KeyStore.getInstance("PKCS12");
        mism.load(null, null);
        mism.setKeyEntry("falsch", rsa.getPrivateKey(), "test".toCharArray(), new java.security.cert.Certificate[] { other });
        store(mism, out.resolve("java-key-mismatch.p12"), "test");

        // 4. JKS und JCEKS (Java-proprietär → Konvertierungshinweis)
        for (String type : new String[] { "JKS", "JCEKS" }) {
            KeyStore ks = KeyStore.getInstance(type);
            ks.load(null, null);
            ks.setKeyEntry("test-rsa2048", rsa.getPrivateKey(), "test".toCharArray(), rsa.getCertificateChain());
            store(ks, out.resolve("java-rsa2048." + type.toLowerCase()), "test");
        }
    }
}
