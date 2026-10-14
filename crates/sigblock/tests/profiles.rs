//! Profil-Engine: eingebettete PDF-AS-Konfiguration, Vererbung, Includes, Overlays
//! (docs/analyse/04 §2.1–§2.9).

use std::path::Path;
use std::sync::Arc;

use pdfover_java_properties::Properties;
use pdfover_sigblock::{
    BuiltinResources, CellSpec, HAlign, PDFOVER_PROFILE_IDS, ProfileOverrides, ProfileStore, SigBlockError, Std14,
    VAlign, fonts::FontSpec,
};

fn store() -> ProfileStore {
    ProfileStore::builtin().expect("eingebettete Konfiguration")
}

#[test]
fn all_pdfover_profile_ids_exist() {
    let s = store();
    for id in PDFOVER_PROFILE_IDS {
        let p = s.get(id).unwrap_or_else(|| panic!("{id} fehlt"));
        assert_eq!(p.id, id);
        assert_eq!(p.visible, id != "INVISIBLE", "{id}");
    }
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    assert_eq!(s.global("sig_obj.type.default"), Some("SIGNATURBLOCK_SMALL_DE_NOTE"));
    assert!(matches!(s.profile("GIBT_ES_NICHT"), Err(SigBlockError::UnknownProfile(_))));
    // 20 Profildateien, alle aktiv
    assert_eq!(s.ids().count(), 20);
}

#[test]
fn small_profiles_effective_content() {
    let s = store();
    for id in PDFOVER_PROFILE_IDS.iter().filter(|i| i.starts_with("SIGNATURBLOCK_SMALL")) {
        let p = s.get(id).unwrap();
        assert_eq!(p.pos.as_deref(), Some("f:80;w:230"), "{id}");
        let main = p.main_table().unwrap();
        assert_eq!(main.rows[0].0, vec![CellSpec::Image("SIG_LABEL".into()), CellSpec::SubTable("info".into())]);
        let note = id.contains("NOTE");
        assert_eq!(main.rows.len(), if note { 2 } else { 1 }, "{id}");
        if note {
            assert_eq!(main.rows[1].0, vec![CellSpec::CaptionValue("SIG_NOTE".into())]);
        }
        let st = main.style.as_ref().unwrap();
        assert_eq!(st.bgcolor, Some([255, 255, 255]));
        assert_eq!(st.padding, Some(4.0));
        assert_eq!(st.border, Some(0.1));
        assert_eq!(st.halign, Some(HAlign::Right));
        assert_eq!(st.valign, Some(VAlign::Top));
        assert_eq!(st.valuehalign, Some(HAlign::Left));
        assert_eq!(st.valuevalign, None);
        assert_eq!(st.imagevalign, Some(VAlign::Middle));
        assert_eq!(st.imagehalign, None);
        let pdfa = id.ends_with("PDFA");
        let font = st.font.clone().unwrap();
        if pdfa {
            assert_eq!(font, FontSpec::TrueType { file: "DejaVuSansCondensed-Bold.ttf".into(), size: 5.0 });
            assert!(p.flags.pdfa1b_valid && p.is_pdfa());
        } else {
            assert_eq!(font, FontSpec::Standard { face: Std14::HelveticaBold, size: 5.0 });
            assert!(!p.is_pdfa());
        }
        assert_eq!(st.valuefont, Some(FontSpec::TrueType { file: "DejaVuSansMono.ttf".into(), size: 5.0 }));
        let info = &p.tables["info"];
        assert_eq!(info.rows.len(), 3);
        assert!(info.style.is_none(), "info erbt den Stil von main");
        let (main_cols, info_cols): (Vec<f32>, Vec<f32>) = match *id {
            "SIGNATURBLOCK_SMALL_EN_NOTE" => (vec![1.0, 4.5], vec![1.0, 3.2]),
            "SIGNATURBLOCK_SMALL_EN_PDFA" => (vec![1.0, 5.5], vec![1.0, 4.5]),
            _ => (vec![1.0, 4.5], vec![1.0, 2.7]),
        };
        assert_eq!(main.cols_width.as_deref(), Some(&main_cols[..]), "{id}");
        assert_eq!(info.cols_width.as_deref(), Some(&info_cols[..]), "{id}");
        let en = id.contains("_EN");
        assert_eq!(p.captions["SIG_SUBJECT"], if en { "Signatory" } else { "Unterzeichner" });
        assert_eq!(p.captions["SIG_DATE"], if en { "Date/Time-UTC" } else { "Datum/Zeit-UTC" });
        assert_eq!(p.captions["SIG_META"], if en { "Verification" } else { "Prüfinformation" });
        assert_eq!(
            p.values["SIG_LABEL"],
            if en { "./images/signatur-logo_en.png" } else { "./images/signatur-logo_de.png" }
        );
        assert_eq!(p.values["SIG_SUBJECT"], "${subject.title != null ? (subject.title + \" \") : \"\"}${subject.CN}");
        // SIG_DATE: kein Wert im Profil ⇒ eingebautes Default-Muster
        assert_eq!(p.values["SIG_DATE"], "yyyy-MM-dd'T'HH:mm:ssXXX");
        assert!(p.reason.as_deref().unwrap().contains("signatur"));
        assert_eq!(p.timezone, None);
    }
}

#[test]
fn amts_and_logo_and_invisible() {
    let s = store();
    for id in ["AMTSSIGNATURBLOCK_DE_RECOMMENDED", "AMTSSIGNATURBLOCK_EN_RECOMMENDED"] {
        let p = s.get(id).unwrap();
        assert_eq!(p.pos.as_deref(), Some("f:80"));
        let main = p.main_table().unwrap();
        assert_eq!(main.cols_width.as_deref(), Some(&[1.0, 6.0][..]));
        let st = main.style.as_ref().unwrap();
        assert_eq!(st.halign, Some(HAlign::Center));
        assert_eq!(st.font, Some(FontSpec::Standard { face: Std14::HelveticaBold, size: 8.0 }));
        assert_eq!(p.values["SIG_LABEL"], "./images/amtsignatur-logo_demo.png");
        let subj = &p.captions["SIG_SUBJECT"];
        assert!(subj.contains('\n'), "echter Zeilenumbruch in der Caption: {subj:?}");
    }
    let logo = s.get("BASE_LOGO").unwrap();
    assert_eq!(logo.pos.as_deref(), Some("w:230.0;f:80"));
    let st = logo.main_table().unwrap().style.as_ref().unwrap();
    assert_eq!(st.border, Some(0.0));
    assert_eq!(st.bgcolor, None);
    assert_eq!(st.imagescaletofit, Some((65.0, 65.0)));
    assert_eq!(st.imagevalign, None);
    let inv = s.get("INVISIBLE").unwrap();
    assert!(!inv.visible);
    assert_eq!(inv.placeholder_search, Some(false));
    assert!(inv.main_table().is_none());
    assert!(!inv.values.contains_key("SIG_SUBJECT"), "kein /Name bei INVISIBLE");
}

#[test]
fn overrides_only_touch_one_profile() {
    let s = store();
    let p = s.get("BASE_LOGO").unwrap();
    let ov = ProfileOverrides::base_logo(None, (326, 342), 23.0);
    let p2 = p.with_overrides(&ov);
    assert_eq!(p2.pos.as_deref(), Some("w:62.117933723196884;f:0"));
    let st = p2.main_table().unwrap().style.as_ref().unwrap();
    assert_eq!(st.padding, Some(0.0));
    let (w, h) = st.imagescaletofit.unwrap();
    assert!((w - 62.118).abs() < 1e-3 && (h - 65.167).abs() < 1e-3);
    // Original unverändert
    assert_eq!(s.get("BASE_LOGO").unwrap().pos.as_deref(), Some("w:230.0;f:80"));
    let note = ProfileOverrides { sig_note: Some("Mein Hinweis".into()), ..Default::default() };
    let n = s.get("SIGNATURBLOCK_SMALL_DE_NOTE").unwrap().with_overrides(&note);
    assert_eq!(n.values["SIG_NOTE"], "Mein Hinweis");
}

fn props(pairs: &[(&str, &str)]) -> Properties {
    let mut p = Properties::new();
    for (k, v) in pairs {
        p.insert(*k, *v);
    }
    p
}

#[test]
fn inheritance_keywise_with_augments_and_defaults() {
    let p = props(&[
        ("sig_obj.types.A", "on"),
        ("sig_obj.types.B", "on"),
        ("sig_obj.types.C", "on"),
        ("sig_obj.types.X", "off"),
        ("sig_obj.A.table.main.1", "SIG_SUBJECT-cv"),
        ("sig_obj.A.table.main.Style.font", "HELVETICA,5,BOLD"),
        ("sig_obj.A.table.main.Style.valuefont", "COURIER,5,NORMAL"),
        ("sig_obj.A.table.main.Style.padding", "4"),
        ("sig_obj.A.key.SIG_SUBJECT", "Wer"),
        ("sig_obj.B.parent", "A"),
        ("sig_obj.B.table.main.Style.font", "TTF:DejaVuSansMono.ttf,6"),
        ("sig_obj.B.augments.1", "C"),
        ("sig_obj.C.key.SIG_SUBJECT", "Augment gewinnt"),
        ("sig_obj.C.table.main.Style.padding", "7"),
        ("sig_obj.X.key.SIG_SUBJECT", "inaktiv"),
        ("default.timezone", "Europe/Vienna"),
        ("default.SIG_DATE", "dd.MM.yyyy"),
    ]);
    let s = ProfileStore::from_properties(&p, Arc::new(BuiltinResources::default())).unwrap();
    assert!(s.get("X").is_none());
    let b = s.get("B").unwrap();
    // schlüsselweise: eigener Font bleibt, Rest kommt aus Augment (vor Parent)
    let st = b.main_table().unwrap().style.as_ref().unwrap();
    assert_eq!(st.font, Some(FontSpec::TrueType { file: "DejaVuSansMono.ttf".into(), size: 6.0 }));
    assert_eq!(st.padding, Some(7.0));
    assert_eq!(st.valuefont, Some(FontSpec::Standard { face: Std14::Courier, size: 5.0 }));
    assert_eq!(b.captions["SIG_SUBJECT"], "Augment gewinnt");
    // Default-Wert für SIG_SUBJECT (DefaultSignatureProfileSettings) und default.timezone
    assert_eq!(b.values["SIG_SUBJECT"], "${subject.T != null ? (subject.T + \" \") : \"\"}${subject.CN}");
    assert_eq!(b.timezone.as_deref(), Some("Europe/Vienna"));
    let a = s.get("A").unwrap();
    assert_eq!(a.main_table().unwrap().style.as_ref().unwrap().padding, Some(4.0));
}

#[test]
fn cycles_and_broken_tables_are_reported() {
    let p = props(&[
        ("sig_obj.types.A", "on"),
        ("sig_obj.types.B", "on"),
        ("sig_obj.A.parent", "B"),
        ("sig_obj.B.parent", "A"),
        ("sig_obj.types.C", "on"),
        ("sig_obj.C.table.main.1", "SIG_LABEL"),
        ("sig_obj.types.D", "on"),
        ("sig_obj.D.table.main.2", "SIG_SUBJECT-cv"),
        ("sig_obj.types.E", "on"),
        ("sig_obj.E.table.main.Style.padding", "abc"),
        ("sig_obj.E.table.main.1", "SIG_SUBJECT-cv"),
    ]);
    let s = ProfileStore::from_properties(&p, Arc::new(BuiltinResources::default())).unwrap();
    let probs = s.problems().join("\n");
    assert!(probs.contains("Profil A"), "{probs}");
    assert!(probs.contains("Profil B"), "{probs}");
    assert!(probs.contains("C: table.main.1"), "{probs}");
    assert!(probs.contains("lückenlos"), "{probs}");
    assert!(probs.contains("E: table.main.Style"), "{probs}");
    // verworfene Tabelle: Profil bleibt sichtbar, aber ohne main
    assert!(s.get("C").unwrap().main_table().is_none());
    assert!(s.get("C").unwrap().visible);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

#[test]
fn load_dir_with_wildcard_includes_matches_builtin() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&assets.join("cfg"), &tmp.path().join("cfg"));
    let s = ProfileStore::load_dir(&tmp.path().join("cfg")).unwrap();
    let b = store();
    for id in PDFOVER_PROFILE_IDS {
        assert_eq!(s.get(id), b.get(id), "{id}");
    }
    // eigenes Profil per zusätzlichem Include, Wildcard mit '?'
    std::fs::write(
        tmp.path().join("cfg/profiles/EIGEN_X1.properties"),
        "sig_obj.types.EIGEN_X1=on\nsig_obj.EIGEN_X1.parent=SIGNATURBLOCK_SMALL_DE\nsig_obj.EIGEN_X1.key.SIG_META=Pr\\u00fcfung\n",
    )
    .unwrap();
    std::fs::write(tmp.path().join("cfg/profiles/EIGEN_X1.properties.bak"), "sig_obj.types.BAK=on\n").unwrap();
    let s2 = ProfileStore::load_dir(&tmp.path().join("cfg")).unwrap();
    let e = s2.get("EIGEN_X1").unwrap();
    assert_eq!(e.captions["SIG_META"], "Prüfung");
    assert_eq!(e.captions["SIG_SUBJECT"], "Unterzeichner");
    assert!(s2.get("BAK").is_none(), "*.properties passt nicht auf .bak");
    // DirResources: Bild aus <workdir>/images fehlt ⇒ Rückfall auf eingebettete Logos
    let img = s2.resources().image("./images/signatur-logo_de.png").unwrap();
    assert_eq!((img.width, img.height), (326, 342));
}

#[test]
fn missing_config_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(matches!(ProfileStore::load_dir(tmp.path()), Err(SigBlockError::MissingResource(_))));
}
