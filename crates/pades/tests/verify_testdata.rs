//! Prüfung vorhandener Signaturen (`verify_document`, ADR 0004 §3.17; der Prüfmodus der App ist entfernt,
//! Nachtrag 15): Testdateien `testdata/verify/*.pdf` und ihre erwarteten Ergebnisse.
//!
//! Die Dateien erzeugt dieser Test selbst (Generator des Kerns) mit den Testschlüsseln aus
//! `testdata/keystores` und dem QES-Testzertifikat aus `testdata/verify/keys` (alle NUR TEST, keine echten
//! Personendaten):
//!
//! ```text
//! testdata/verify/make-keys.sh                                  # QES-Testzertifikat (einmalig, openssl)
//! PIDA_BLESS=1 cargo test -p pida-pades --test verify_testdata  # Dateien neu erzeugen
//! ```
//!
//! Signiert wird mit der eigenen Engine (PDF-AS-kompatibel, `PdfasCompat`). Für die Altverfahren, die die
//! Engine für neue Signaturen ablehnt (SHA-1, `adbe.pkcs7.sha1`), signiert `openssl cms` als Ersatz-Signiergerät
//! über die Bytes des vorbereiteten Dokuments (wie die Fake-BKU der Ende-zu-Ende-Tests). Ohne `PIDA_BLESS`
//! prüft der Test nur die eingecheckten Dateien (kein openssl nötig).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use pida_cades::{CmsWarning, DigestAlg, SignatureAlg};
use pida_pades::{Integrity, PlanOptions, Prepared, Reserve, VerifyReport, prepare, verify_document};
use pida_pdf_cos::{Limits, Object, SourcePdf};

/// Eingabe aller Fälle (qpdf, eine Seite 612 × 792 pt).
const BASE: &str = "pdf/generated/simple-xref-table.pdf";
/// Gedrehte Seite (`/Rotate 90`, sichtbar 792 × 612 pt).
const ROTATED: &str = "pdf/pdfas/pdf-as-cli/simple_rotated_90.pdf";
/// Lage des Blocks auf der gedrehten Seite (sichtbarer Raum, `PAGE_ABS`: linke Kante, Oberkante von unten);
/// `core-api/tests/verify.rs` prüft das Rechteck der `AppearanceView` dagegen.
pub const ROTATED_POSITION: &str = "p:1;x:100;y:500";
/// QES-Testzertifikat (qcStatements QcCompliance + QcSSCD)
const QES_P12: &str = "verify/keys/test-qes-ecp256.p12";

const CN_RSA: &str = "PDF-Over NG Test RSA (NUR TEST)";
const CN_P256: &str = "PDF-Over NG Test ECDSA P-256 (NUR TEST)";
const CN_QES: &str = "PIDA Test QES P-256 (NUR TEST)";

fn dir() -> PathBuf {
    testdata().join("verify")
}

fn bless() -> bool {
    std::env::var_os("PIDA_BLESS").is_some()
}

fn open_bytes(bytes: &[u8]) -> SourcePdf {
    SourcePdf::open(bytes.to_vec(), &Limits::default()).expect("PDF lesbar")
}

/// Signiert mit der eigenen Engine (Modell A, Keystore), Selbstprüfung I1–I6.
fn sign_engine(input: &[u8], signer: &mut KeystoreSigner, profile: &str, position: Option<&str>) -> Vec<u8> {
    let src = open_bytes(input);
    let report = pida_pades::analyze(&src).unwrap();
    let mut opts = PlanOptions::new(pida_pades::local_signing_time());
    if let Some(p) = position {
        opts.position = pida_placement::TablePos::parse(p).unwrap();
    }
    let renderer = open_renderer(&src);
    let planned = plan_for(&src, &report, profile, &signer.chain.signer, &opts, renderer.as_deref()).unwrap();
    let prepared = prepare(&src, &planned.plan, Some(&signer.chain)).unwrap();
    let signed = signer.sign(prepared).unwrap();
    let backend = pdfium();
    let ctx = pida_pades::CheckContext {
        source: &src,
        expected_signer: Some(&signer.chain.signer),
        renderer: backend.as_ref().map(|b| b as &dyn pida_render::RenderBackend),
    };
    signed.self_check(&ctx).expect("Selbstprüfung");
    signed.bytes()
}

/// Bereitet mit der Engine vor (Reserve 16 KiB) – für das Ersatz-Signiergerät `openssl`.
fn prepare_for_openssl(input: &[u8], signer: &KeystoreSigner) -> (Prepared, SourcePdf) {
    let src = open_bytes(input);
    let report = pida_pades::analyze(&src).unwrap();
    let mut opts = PlanOptions::new(pida_pades::local_signing_time());
    opts.reserve = Reserve::Bytes(16 * 1024);
    let renderer = open_renderer(&src);
    let planned =
        plan_for(&src, &report, "SIGNATURBLOCK_SMALL_DE", &signer.chain.signer, &opts, renderer.as_deref()).unwrap();
    let prepared = prepare(&src, &planned.plan, Some(&signer.chain)).unwrap();
    (prepared, src)
}

/// Schlüssel und Zertifikat als PEM (openssl), Arbeitsordner.
struct OpensslSigner {
    openssl: PathBuf,
    key: PathBuf,
    cert: PathBuf,
    work: PathBuf,
}

impl OpensslSigner {
    fn new(stem: &str, work: &Path) -> OpensslSigner {
        let bku = FakeBku::from_p12(stem, work).expect("openssl ≥ 3 zum Erzeugen der Testdateien");
        OpensslSigner { openssl: bku.openssl, key: bku.key_pem, cert: bku.cert_pem, work: work.to_path_buf() }
    }

    /// `openssl cms -sign` mit SHA-1: detached CAdES (`-cades`) bzw. eingebettet über den SHA-1-Wert
    /// (`adbe.pkcs7.sha1`).
    fn sign_sha1(&self, data: &[u8], detached_cades: bool) -> Vec<u8> {
        let input = self.work.join("data.bin");
        let output = self.work.join("cms.der");
        std::fs::write(&input, data).unwrap();
        let mut cmd = Command::new(&self.openssl);
        cmd.args(["cms", "-sign", "-binary", "-outform", "DER", "-nosmimecap", "-md", "sha1"]);
        if detached_cades {
            cmd.arg("-cades");
        } else {
            cmd.arg("-nodetach");
        }
        let o = cmd
            .arg("-in")
            .arg(&input)
            .arg("-signer")
            .arg(&self.cert)
            .arg("-inkey")
            .arg(&self.key)
            .arg("-out")
            .arg(&output)
            .output()
            .unwrap();
        assert!(o.status.success(), "openssl cms: {}", String::from_utf8_lossy(&o.stderr));
        std::fs::read(&output).unwrap()
    }
}

/// Ersetzt `/SubFilter /ETSI.CAdES.detached` im Increment (gleiche Länge, mit Leerzeichen aufgefüllt).
fn patch_sub_filter(pdf: &mut [u8], from: usize, sub_filter: &str) {
    let old = b"/SubFilter /ETSI.CAdES.detached";
    let pos = from + pdf[from..].windows(old.len()).position(|w| w == old).expect("/SubFilter im Increment");
    let mut new = format!("/SubFilter /{sub_filter}").into_bytes();
    assert!(new.len() <= old.len());
    new.resize(old.len(), b' ');
    pdf[pos..pos + old.len()].copy_from_slice(&new);
}

/// CMS als Hex in `/Contents <…>` des Platzhalter-PDFs einsetzen.
fn embed(pdf: &mut [u8], prepared: &Prepared, cms: &[u8]) {
    let [_, a, b, _] = prepared.byte_range().0;
    let (start, end) = (a as usize + 1, b as usize - 1);
    let hex: String = cms.iter().map(|b| format!("{b:02X}")).collect();
    assert!(hex.len() <= end - start, "CMS passt in die Reserve");
    pdf[start..start + hex.len()].copy_from_slice(hex.as_bytes());
}

/// ByteRange-Daten eines Platzhalter-PDFs.
fn signed_data(pdf: &[u8], prepared: &Prepared) -> Vec<u8> {
    let [_, a, b, c] = prepared.byte_range().0;
    let mut data = pdf[..a as usize].to_vec();
    data.extend_from_slice(&pdf[b as usize..(b + c) as usize]);
    data
}

/// Ein Increment ohne Signatur (geänderter Katalog), angehängt an `pdf`.
fn append_unsigned_revision(pdf: &[u8]) -> Vec<u8> {
    let src = open_bytes(pdf);
    let catalog = src.catalog_id().unwrap();
    let mut dict = src.get_object(catalog).unwrap().as_dict().unwrap().clone();
    dict.set(b"PageMode".to_vec(), Object::Name(b"UseOutlines".to_vec()));
    let mut b = pida_pdf_incremental::IncrementBuilder::new(&src, pida_pdf_incremental::IncrementOptions::default());
    b.update_object(catalog, Object::Dictionary(dict)).unwrap();
    let inc = b.build().unwrap();
    inc.to_vec(src.bytes())
}

fn generate() {
    let out = dir();
    std::fs::create_dir_all(&out).unwrap();
    let base = std::fs::read(testdata().join(BASE)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let mut rsa = KeystoreSigner::open("test-rsa2048");
    let mut p256 = KeystoreSigner::open("test-ecp256");
    let write = |name: &str, bytes: &[u8]| std::fs::write(out.join(name), bytes).unwrap();

    // ohne Signatur
    write("unsigned.pdf", &base);
    // PDF-AS-kompatibel signiert (sichtbar)
    let pdfas = sign_engine(&base, &mut rsa, "SIGNATURBLOCK_SMALL_DE", None);
    write("pdfas-signed.pdf", &pdfas);
    // manipuliert: ein Byte im signierten Bereich (Binärkommentar der zweiten Zeile) geändert
    let mut tampered = pdfas.clone();
    let pos = tampered.iter().position(|b| *b == b'\n').unwrap() + 2;
    tampered[pos] ^= 0x01;
    write("tampered.pdf", &tampered);
    // unsichtbar
    write("invisible.pdf", &sign_engine(&base, &mut p256, "INVISIBLE", None));
    // zwei Signaturen: die erste wird von einer späteren Revision (der zweiten Signatur) überdeckt
    let second = sign_engine(&pdfas, &mut p256, "SIGNATURBLOCK_SMALL_EN", Some("p:1;x:300;y:300"));
    write("two-signatures.pdf", &second);
    // nach der letzten Signatur ergänzt (Increment ohne Signatur)
    write("modified-after-signing.pdf", &append_unsigned_revision(&pdfas));
    // QES-Testzertifikat (qcStatements)
    let mut qes = KeystoreSigner::open_p12(&testdata().join(QES_P12));
    write("qes.pdf", &sign_engine(&base, &mut qes, "SIGNATURBLOCK_SMALL_DE", None));
    // gedrehte Seite, Block an fester Lage im sichtbaren Raum
    let rotated = std::fs::read(testdata().join(ROTATED)).unwrap();
    write("rotated-90.pdf", &sign_engine(&rotated, &mut rsa, "SIGNATURBLOCK_SMALL_DE", Some(ROTATED_POSITION)));

    // Altverfahren über openssl
    let ossl = OpensslSigner::new("test-rsa2048", tmp.path());
    // CAdES mit SHA-1 (ESS signing-certificate v1)
    let (prepared, src) = prepare_for_openssl(&base, &rsa);
    let mut pdf = prepared.placeholder_pdf();
    let cms = ossl.sign_sha1(&signed_data(&pdf, &prepared), true);
    embed(&mut pdf, &prepared, &cms);
    write("cades-sha1.pdf", &pdf);
    // adbe.pkcs7.sha1: eingebettetes CMS über den SHA-1-Wert der ByteRange-Daten
    let mut pdf = prepared.placeholder_pdf();
    patch_sub_filter(&mut pdf, src.bytes().len(), "adbe.pkcs7.sha1");
    let digest = DigestAlg::Sha1.digest(&signed_data(&pdf, &prepared)).value;
    let cms = ossl.sign_sha1(&digest, false);
    embed(&mut pdf, &prepared, &cms);
    write("pkcs7-sha1.pdf", &pdf);
}

fn report(name: &str) -> VerifyReport {
    let path = dir().join(name);
    let src = SourcePdf::open_file(&path, &Limits::default()).unwrap_or_else(|e| {
        panic!("{name}: {e} – Testdateien erzeugen: PIDA_BLESS=1 cargo test -p pida-pades --test verify_testdata")
    });
    verify_document(&src).unwrap()
}

fn cn(v: &pida_pades::SignatureVerification) -> Option<String> {
    v.signer.as_ref().and_then(pida_cades::Certificate::subject_cn)
}

#[test]
fn verify_testdata_matches_the_table() {
    if bless() {
        generate();
    }

    // ohne Signatur
    let r = report("unsigned.pdf");
    assert!(r.signatures.is_empty() && !r.modified_after_last_signature());

    // PDF-AS-kompatibel signiert: intakt, ganze Datei, Unterzeichner, Zeit, Verfahren, sichtbar
    let r = report("pdfas-signed.pdf");
    assert_eq!(r.revisions, 2);
    let [s] = r.signatures.as_slice() else { panic!("eine Signatur: {r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert!(s.covers_whole_file && s.later_revisions == 0 && s.later_signatures == 0);
    assert_eq!(cn(s).as_deref(), Some(CN_RSA));
    assert!(s.signing_time.is_some(), "signing-time (PdfasCompat)");
    assert_eq!(
        (s.digest_alg, s.signature_alg),
        (Some(DigestAlg::Sha256), Some(SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256)))
    );
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    assert_eq!(s.sub_filter.as_deref(), Some("ETSI.CAdES.detached"));
    let (page, rect) = s.widget.expect("sichtbar");
    assert_eq!(page, 0);
    assert!(rect.width() > 100.0 && rect.height() > 20.0, "{rect:?}");
    assert!(!r.modified_after_last_signature());

    // manipuliert → verändert (message-digest)
    let r = report("tampered.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Broken { code: "cms-digest-mismatch".into() });
    assert_eq!(cn(s).as_deref(), Some(CN_RSA), "Unterzeichner auch bei veränderter Signatur");
    assert!(s.signing_time.is_some());

    // adbe.pkcs7.sha1 → nicht prüfbar
    let r = report("pkcs7-sha1.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::NotVerifiable { code: "pdf-signature-subfilter".into() });
    assert_eq!(s.sub_filter.as_deref(), Some("adbe.pkcs7.sha1"));
    assert_eq!(cn(s).as_deref(), Some(CN_RSA));

    // CAdES mit SHA-1 → intakt mit SHA-1-Hinweis
    let r = report("cades-sha1.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert_eq!(s.digest_alg, Some(DigestAlg::Sha1));
    assert!(s.warnings.contains(&CmsWarning::Sha1Digest), "{:?}", s.warnings);
    assert!(s.warnings.contains(&CmsWarning::SigningCertificateV1), "{:?}", s.warnings);

    // zwei Signaturen: die erste deckt eine frühere Revision ab
    let r = report("two-signatures.pdf");
    assert_eq!(r.revisions, 3);
    let [first, second] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!((cn(first).as_deref(), cn(second).as_deref()), (Some(CN_RSA), Some(CN_P256)));
    assert!(first.integrity == Integrity::Intact && second.integrity == Integrity::Intact);
    assert!(!first.covers_whole_file && first.later_revisions == 1 && first.later_signatures == 1, "{first:?}");
    assert!(second.covers_whole_file && second.later_revisions == 0 && second.later_signatures == 0);
    assert_eq!(second.signature_alg, Some(SignatureAlg::Ecdsa(DigestAlg::Sha256)));
    assert!(!r.modified_after_last_signature());

    // nach der letzten Signatur ergänzt
    let r = report("modified-after-signing.pdf");
    assert_eq!(r.revisions, 3);
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert!(!s.covers_whole_file && s.later_revisions == 1 && s.later_signatures == 0, "{s:?}");
    assert!(r.modified_after_last_signature());

    // unsichtbar
    let r = report("invisible.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert_eq!(s.widget, None);
    assert_eq!(cn(s).as_deref(), Some(CN_P256));

    // QES-Testzertifikat: qcStatements laut Zertifikat
    let r = report("qes.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert_eq!(cn(s).as_deref(), Some(CN_QES));
    let qc = pida_cades::qc_statements(s.signer.as_ref().unwrap());
    assert!(qc.compliance && qc.sscd, "{qc:?}");

    // gedrehte Seite: Widget auf Seite 0 (Rechteck im Benutzerraum; PAGE_ABS prüft core-api)
    let r = report("rotated-90.pdf");
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Intact);
    assert_eq!(s.widget.map(|w| w.0), Some(0));
}

/// Signature Wrapping: ungeschützte Bytes in der Lücke der `/ByteRange` (der Hex-String `/Contents` wird um
/// das Padding gekürzt, dahinter ein weiterer Schlüssel des Signaturdictionarys). Kryptografisch passt die
/// Signatur weiter (CMS und signierte Bytes unverändert), die Datei ist aber nicht mehr nur das Signierte –
/// „verändert“ (`pdf-signature-byterange`), wie pyHanko („coverage unclear“).
#[test]
fn bytes_hidden_in_the_byte_range_gap_are_broken() {
    let original = std::fs::read(dir().join("pdfas-signed.pdf")).unwrap();
    let r = verify_document(&open_bytes(&original)).unwrap();
    assert_eq!(r.signatures[0].integrity, Integrity::Intact);
    let [_, _, gap_end, _] = byte_range_of(&original).expect("ByteRange");
    let gap_end = gap_end as usize;
    let mut pdf = original.clone();
    let inject = b">/X 1";
    assert!(pdf[gap_end - inject.len()..gap_end - 1].iter().all(|b| *b == b'0'), "Padding am Ende der Lücke");
    pdf[gap_end - inject.len()..gap_end].copy_from_slice(inject);
    let r = verify_document(&open_bytes(&pdf)).unwrap();
    let [s] = r.signatures.as_slice() else { panic!("{r:?}") };
    assert_eq!(s.integrity, Integrity::Broken { code: pida_pades::CODE_SIGNATURE_BYTERANGE.into() });
    assert_eq!(cn(s).as_deref(), Some(CN_RSA), "Unterzeichner trotzdem genannt");
    // dieselbe Regel für Vorsignaturen (Merge E4): Inspektor im Arbeitsbereich (`analyze`) und I4 sagen nicht
    // „intakt“, wo `verify_document` „verändert“ sagt – vor dem Gegenzeichnen folgt die Analysewarnung
    let prior = |pdf: &[u8]| {
        let report = pida_pades::analyze(&open_bytes(pdf)).unwrap();
        let [e] = report.signatures.as_slice() else { panic!("{:?}", report.signatures.len()) };
        let warned = report.findings.iter().any(|f| f.code == pida_pades::codes::PRIOR_SIGNATURE_INVALID);
        (e.status.clone(), e.status_code.clone(), e.signer_cn.clone(), warned)
    };
    assert_eq!(prior(&original), (pida_pades::PriorStatus::Valid, None, Some(CN_RSA.into()), false));
    assert_eq!(
        prior(&pdf),
        (
            pida_pades::PriorStatus::Invalid,
            Some(pida_pades::CODE_SIGNATURE_BYTERANGE.into()),
            Some(CN_RSA.into()),
            true
        )
    );
}

/// Gegenprobe mit echter PDF-AS-Ausgabe: Alle von der offiziellen PDF-AS-4.4.2-CLI signierten Golden-Fälle
/// (`testdata/golden/pdfas-4.4.2/cases/*/signed.pdf`, Testzertifikat) sind intakt und decken die ganze Datei
/// ab (wie pyHanko: „intact“, „entire file“); die mehrfach signierten PDF-AS-Testdateien nennen die späteren
/// Revisionen.
#[test]
fn pdfas_signed_reference_files_are_intact() {
    let cases = testdata().join("golden/pdfas-4.4.2/cases");
    let mut checked = 0;
    for entry in std::fs::read_dir(&cases).unwrap() {
        let path = entry.unwrap().path().join("signed.pdf");
        if !path.is_file() {
            continue;
        }
        let src = SourcePdf::open_file(&path, &Limits::default()).unwrap();
        let r = verify_document(&src).unwrap();
        let [s] = r.signatures.as_slice() else { panic!("{}: {r:?}", path.display()) };
        assert_eq!(s.integrity, Integrity::Intact, "{}", path.display());
        assert!(s.covers_whole_file && s.later_revisions == 0, "{}: {s:?}", path.display());
        assert_eq!(cn(s).as_deref(), Some(CN_RSA));
        checked += 1;
    }
    assert!(checked >= 80, "nur {checked} PDF-AS-Fälle");
    // dreifach von PDF-AS signiert: die ersten beiden von späteren Revisionen überdeckt
    let path = testdata().join("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf");
    let r = verify_document(&SourcePdf::open_file(&path, &Limits::default()).unwrap()).unwrap();
    let later: Vec<(bool, usize, usize)> = r
        .signatures
        .iter()
        .map(|s| (s.integrity == Integrity::Intact, s.later_revisions, s.later_signatures))
        .collect();
    assert_eq!(later, vec![(true, 2, 2), (true, 1, 1), (true, 0, 0)], "{r:?}");
    assert!(r.signatures[2].covers_whole_file && !r.modified_after_last_signature());
}

/// `/ByteRange [a b c d]` der (einzigen) Signatur, roh aus der Datei.
fn byte_range_of(pdf: &[u8]) -> Option<[u64; 4]> {
    let key = b"/ByteRange [";
    let pos = pdf.windows(key.len()).rposition(|w| w == key)? + key.len();
    let end = pos + pdf[pos..].iter().position(|b| *b == b']')?;
    let nums: Vec<u64> =
        std::str::from_utf8(&pdf[pos..end]).ok()?.split_whitespace().map(|n| n.parse().ok()).collect::<Option<_>>()?;
    nums.try_into().ok()
}

/// Personendaten im `Debug` (Regel 9): Feldname und Zertifikat nur redigiert.
#[test]
fn verification_debug_is_redacted() {
    let r = report("pdfas-signed.pdf");
    let d = format!("{r:?}");
    assert!(!d.contains("NUR TEST") && !d.contains("PDF-AS Signatur"), "{d}");
}
