//! Drittes Orakel: pyHanko (`async_validate_detached_cms`) prüft unsere CMS (Integrität,
//! Signaturwert, Vertrauen gegen das Testzertifikat als Anker). Übersprungen, wenn kein `python3`
//! mit pyHanko im `PATH` ist (`source tools/env.sh` nimmt die `.venv` des Hauptcheckouts auf).

mod common;

use std::process::Command;

use pdfover_cades::{CmsProfile, RawSigner, SigningTime, sign_detached};
use time::OffsetDateTime;

use common::*;

const SCRIPT: &str = r#"
import sys, json, asyncio
from asn1crypto import cms, x509
from pyhanko.sign.validation.generic_cms import async_validate_detached_cms
from pyhanko_certvalidator import ValidationContext
data = open(sys.argv[1], 'rb').read()
sd = cms.ContentInfo.load(open(sys.argv[2], 'rb').read())['content']
anchor = x509.Certificate.load(open(sys.argv[3], 'rb').read())
vc = ValidationContext(trust_roots=[anchor], allow_fetching=False)
st = asyncio.run(async_validate_detached_cms(data, sd, signer_validation_context=vc))
print(json.dumps({"intact": bool(st.intact), "valid": bool(st.valid), "trusted": bool(st.trusted)}))
"#;

fn pyhanko_available() -> bool {
    Command::new("python3")
        .args(["-I", "-c", "import pyhanko.sign.validation.generic_cms, pyhanko_certvalidator"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn pyhanko_check(dir: &std::path::Path, content: &[u8], cms: &[u8], anchor: &[u8]) -> String {
    let c = dir.join("content.bin");
    let s = dir.join("sig.cms.der");
    let a = dir.join("anchor.der");
    std::fs::write(&c, content).unwrap();
    std::fs::write(&s, cms).unwrap();
    std::fs::write(&a, anchor).unwrap();
    let out = Command::new("python3").args(["-I", "-c", SCRIPT]).arg(&c).arg(&s).arg(&a).output().unwrap();
    assert!(out.status.success(), "pyHanko: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn our_cms_validates_with_pyhanko() {
    if !pyhanko_available() {
        eprintln!("python3 mit pyHanko nicht gefunden – Orakeltest übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let content = content();
    for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
        for s in [rsa2048(), ecp256(), ecp384(), p521(), rsa3072()] {
            let md = s.algorithm().digest().digest(&content);
            let cms = sign_detached(&s, &md, profile, SigningTime(OffsetDateTime::now_utc())).unwrap();
            // Endzertifikat als Anker (Testzertifikate sind selbstsigniert bzw. von der Test-CA)
            let anchor = s.chain().signer.as_der().to_vec();
            let res = pyhanko_check(dir.path(), &content, &cms, &anchor);
            assert!(
                res.contains(r#""intact": true"#) && res.contains(r#""valid": true"#),
                "{profile:?}/{:?}: {res}",
                s.algorithm()
            );
            let mut bad = content.clone();
            bad[3] ^= 1;
            let res = pyhanko_check(dir.path(), &bad, &cms, &anchor);
            assert!(res.contains(r#""intact": false"#), "Manipulation nicht erkannt: {res}");
        }
    }
}
