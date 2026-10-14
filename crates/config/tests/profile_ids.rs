//! Profil-ID-Matrix wie `PdfAs4SignatureParameter.getPdfAsSignatureProfileId()` (docs/analyse/05 §1.6):
//! alle 4 Profile × 2 Sprachen × Hinweis × PDF/A ergeben genau die 12 PDF-Over-IDs, und jede ID ist
//! ein ausgeliefertes PDF-AS-Profil.

use std::collections::BTreeSet;

use pdfover_config::migrate::SHIPPED_PDFAS_FILES;
use pdfover_config::{Effective, NoteSetting, Overlay, Settings, SignatureProfile};
use pdfover_i18n::Lang;

fn expected(profile: SignatureProfile, lang: Lang, note: bool, pdfa: bool) -> String {
    let l = if lang == Lang::En { "EN" } else { "DE" };
    match profile {
        SignatureProfile::Invisible => "INVISIBLE".into(),
        SignatureProfile::BaseLogo => "BASE_LOGO".into(),
        SignatureProfile::Amtssignaturblock => format!("AMTSSIGNATURBLOCK_{l}_RECOMMENDED"),
        SignatureProfile::SignaturblockSmall => {
            format!("SIGNATURBLOCK_SMALL_{l}{}{}", if note { "_NOTE" } else { "" }, if pdfa { "_PDFA" } else { "" })
        }
    }
}

#[test]
fn full_matrix() {
    let mut ids = BTreeSet::new();
    for profile in SignatureProfile::ALL {
        for lang in Lang::ALL {
            for note in [false, true] {
                for pdfa in [false, true] {
                    let id = profile.pdfas_profile_id(lang, note, pdfa);
                    assert_eq!(id, expected(profile, lang, note, pdfa), "{profile:?} {lang:?} {note} {pdfa}");
                    ids.insert(id);
                }
            }
        }
    }
    assert_eq!(ids.len(), 12);
    let shipped: BTreeSet<String> = SHIPPED_PDFAS_FILES
        .iter()
        .filter_map(|(name, _)| name.strip_prefix("profiles/")?.strip_suffix(".properties").map(str::to_owned))
        .collect();
    for id in &ids {
        assert!(shipped.contains(id), "{id} ist kein ausgeliefertes Profil");
    }
}

#[test]
fn matrix_from_settings_note_rules() {
    // Hinweis: Default (nur Small/Amts haben Text), None, leerer Custom, eigener Text.
    let cases = [
        (NoteSetting::Default, SignatureProfile::SignaturblockSmall, "SIGNATURBLOCK_SMALL_DE_NOTE"),
        (NoteSetting::None, SignatureProfile::SignaturblockSmall, "SIGNATURBLOCK_SMALL_DE"),
        (NoteSetting::Custom("   ".into()), SignatureProfile::SignaturblockSmall, "SIGNATURBLOCK_SMALL_DE"),
        (NoteSetting::Custom("x".into()), SignatureProfile::SignaturblockSmall, "SIGNATURBLOCK_SMALL_DE_NOTE"),
        (NoteSetting::Default, SignatureProfile::Amtssignaturblock, "AMTSSIGNATURBLOCK_DE_RECOMMENDED"),
        (NoteSetting::Default, SignatureProfile::BaseLogo, "BASE_LOGO"),
    ];
    for (note, profile, id) in cases {
        let mut s = Settings::default();
        s.signature.note = note.clone();
        s.signature.profile = profile;
        assert_eq!(s.pdfas_profile_id(Lang::De, false), id, "{note:?} {profile:?}");
    }
    // Systemsprache wirkt nur ohne SIGNLOCALE
    let s = Settings::default();
    assert_eq!(s.pdfas_profile_id(Lang::En, false), "SIGNATURBLOCK_SMALL_EN_NOTE");
    // erkannte PDF/A-Kennung schaltet _PDFA zu (Plan §6.9)
    assert_eq!(s.pdfas_profile_id(Lang::De, true), "SIGNATURBLOCK_SMALL_DE_NOTE_PDFA");
}

#[test]
fn cli_v_forces_invisible() {
    let s = Settings::default();
    let o = Overlay { invisible: Some(true), ..Default::default() };
    let e = Effective::new(&s, &o, None);
    assert_eq!(e.profile().pdfas_profile_id(Lang::De, true, true), "INVISIBLE");
    let o = Overlay { invisible: Some(false), ..Default::default() };
    assert_eq!(Effective::new(&s, &o, None).profile(), SignatureProfile::SignaturblockSmall);
}
