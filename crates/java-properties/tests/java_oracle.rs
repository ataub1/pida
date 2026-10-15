//! Differentialtest gegen `java.util.Properties` (JDK im `PATH`, sonst übersprungen): Grenzfälle der
//! Zeilenlogik, Escapes, Trenner, Kodierung sowie das Format von `store`.

use std::path::Path;
use std::process::Command;

use pida_java_properties::{Properties, StoreOptions};

const CASES: &[&[u8]] = &[
    b"a=1\nb : 2\nc 3\n  d=  4  \ne\nf=\n",
    b"k = first \\\n    second \\\r\n\tthird\n",
    b"k=a\\\n#b\n",
    b"x=\\\\\n",
    b"y=a\\",
    b"y=a\\\\\\",
    b"a\\ b=c\n",
    b"\\ =v\n",
    b"=v\n",
    b":v\n",
    b"key\tvalue\n",
    b"key\x0cvalue\n",
    b"a=1\rb=2\r",
    b"a=1\r\n\r\nb=2\r\n",
    b"# comment\\\nb=2\n",
    b"a=\\u00e4\\u00C4\n",
    b"a=\\u12\n",
    b"a=\\u12g4\n",
    b"a=\\q\\z\n",
    b"a=1\\\n\nb=2\n",
    b"a=1\\\n   \nb=2\n",
    b"   \n\t\n",
    b"a=b=c\n",
    b"a==b\n",
    b"a = = b\n",
    b"a  :  b\n",
    b"a :=b\n",
    b"a\\=b=c\n",
    b"a\\:b:c\n",
    b"\xfc\xe4=\xdf\xa0\xff\n",
    b"a=x\\\r\ny\n",
    b"a=\\\n",
    b"!c\na=1\n",
    b"a=1 \\\n 2\n",
    b"  # indented comment\na=1\n",
    b"a\\\nb=c\n",
    b"key=val\\u0020\n",
    b"\\#a=1\n",
    b"a=\\t\\n\\r\\f\n",
    b"\\u0041=\\u0042\n",
    b"a=\\uD83D\\uDE00\n",
    b"a\n",
    b"a=1\\\\\nb=2\n",
    b"a=1\\\\\\\nb=2\n",
    b"my key=my value\n",
    b"b=2\na=1\nB=3\n_=4\n",
    b"k= lead\nk2=\\ lead\nk3=trail \n",
    b"z=#!=:\n",
    b"a=1\na=2\n",
    b"\\\\=\\\\\n",
    b"a=\x7f\x80\n",
];

fn java_available() -> bool {
    Command::new("java").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn unescape(s: &str) -> String {
    let mut units: Vec<u16> = Vec::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            let mut b = [0u16; 2];
            units.extend_from_slice(c.encode_utf16(&mut b));
            continue;
        }
        let mapped = match it.next() {
            Some('t') => u16::from(b'\t'),
            Some('n') => u16::from(b'\n'),
            Some('r') => u16::from(b'\r'),
            Some('u') => {
                let hex: String = it.by_ref().take(4).collect();
                u16::from_str_radix(&hex, 16).unwrap()
            }
            Some(o) => o as u16,
            None => continue,
        };
        units.push(mapped);
    }
    String::from_utf16_lossy(&units)
}

#[test]
fn matches_java_properties() {
    if !java_available() {
        eprintln!("übersprungen: kein java im PATH");
        return;
    }
    let mut input: Vec<u8> = Vec::new();
    for c in CASES {
        input.extend_from_slice(b"=====CASE=====\n");
        input.extend_from_slice(c);
    }
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pida-java-properties-cases.bin");
    std::fs::write(&file, &input).unwrap();
    let oracle = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/PropsOracle.java");
    let out = Command::new("java").arg(oracle).arg(&file).output().expect("java");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();

    let mut java: Vec<(Option<Properties>, String)> = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        match parts[0] {
            "CASE" => java.push((Some(Properties::new()), String::new())),
            "ERR" => java.last_mut().unwrap().0 = None,
            "E" => {
                let props = java.last_mut().unwrap().0.as_mut().unwrap();
                props.insert(unescape(parts[1]), unescape(parts[2]));
            }
            "STORE" => java.last_mut().unwrap().1 = unescape(parts[1]),
            _ => {}
        }
    }
    assert_eq!(java.len(), CASES.len());
    for (i, (case, (expected, stored))) in CASES.iter().zip(&java).enumerate() {
        let ours = Properties::load_latin1(case);
        match expected {
            None => assert!(ours.is_err(), "Fall {i}: Java wirft, wir nicht ({case:?})"),
            Some(exp) => {
                let ours = ours.unwrap_or_else(|e| panic!("Fall {i}: {e} ({case:?})"));
                assert_eq!(ours.len(), exp.len(), "Fall {i}: {case:?} → {ours:?} / {exp:?}");
                for (k, v) in exp.iter() {
                    assert_eq!(ours.get(k), Some(v), "Fall {i}: Key {k:?} ({case:?})");
                }
                let our_store = String::from_utf8_lossy(&ours.store_latin1(&StoreOptions::default())).into_owned();
                // Java schreibt mit System.lineSeparator() (Windows: CRLF); unser Default ist LF auf allen Plattformen.
                let stored = stored.replace("\r\n", "\n");
                assert_eq!(our_store, stored, "Fall {i}: store ({case:?})");
            }
        }
    }
}
