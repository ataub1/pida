//! Regressionstests der Sicherheitskonfiguration (Plan §7.7): minimale Capabilities, strenge CSP,
//! keine Remote-Inhalte, eingefrorener Prototyp, kein globales `__TAURI__`.

use std::collections::BTreeSet;

const CAPABILITIES: &str = include_str!("../capabilities/default.json");
const TAURI_CONF: &str = include_str!("../tauri.conf.json");

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("gültiges JSON")
}

fn permission_ids(caps: &serde_json::Value) -> BTreeSet<String> {
    caps["permissions"]
        .as_array()
        .expect("permissions")
        .iter()
        .map(|p| match p {
            serde_json::Value::String(s) => s.clone(),
            other => other["identifier"].as_str().expect("identifier").to_owned(),
        })
        .collect()
}

#[test]
fn capabilities_are_minimal() {
    let caps = json(CAPABILITIES);
    assert_eq!(caps["windows"], serde_json::json!(["main"]), "nur das Hauptfenster");
    assert!(caps.get("remote").is_none(), "keine Remote-URLs mit IPC-Zugriff");
    let ids = permission_ids(&caps);
    let expected: BTreeSet<String> = [
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:webview:allow-internal-toggle-devtools",
        "allow-core-view",
        "allow-core-dispatch",
        "allow-app-info",
        "allow-open-log-dir",
        "allow-reveal-output",
        "dialog:allow-open",
        "dialog:allow-save",
        "opener:allow-open-url",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(ids, expected, "Berechtigungen nur erweitern, wenn die Oberfläche sie braucht");
    for id in &ids {
        assert!(!id.ends_with(":default"), "keine Sammelberechtigung {id}");
    }
    // Links nur https/mailto (kein file:, kein http:, keine Programme)
    let opener = caps["permissions"]
        .as_array()
        .and_then(|list| list.iter().find(|p| p["identifier"] == "opener:allow-open-url"))
        .expect("opener-Eintrag");
    assert_eq!(opener["allow"], serde_json::json!([{ "url": "https://*" }, { "url": "mailto:*" }]));
}

#[test]
fn csp_has_no_remote_sources() {
    let conf = json(TAURI_CONF);
    let security = &conf["app"]["security"];
    assert_eq!(security["freezePrototype"], true);
    assert_eq!(conf["app"]["withGlobalTauri"], false);
    let csp = security["csp"].as_str().expect("CSP gesetzt");
    let directives: Vec<(&str, Vec<&str>)> = csp
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut parts = d.split_whitespace();
            (parts.next().unwrap_or_default(), parts.collect())
        })
        .collect();
    let get = |name: &str| directives.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone());
    assert_eq!(get("default-src"), Some(vec!["'self'"]));
    assert_eq!(get("script-src"), Some(vec!["'self'"]), "keine Inline-/Remote-Skripte");
    assert_eq!(get("object-src"), Some(vec!["'none'"]));
    assert_eq!(get("frame-src"), Some(vec!["'none'"]));
    assert_eq!(get("base-uri"), Some(vec!["'none'"]));
    assert_eq!(get("form-action"), Some(vec!["'none'"]));
    for (name, sources) in &directives {
        for source in sources {
            let remote = source.starts_with("http://") || source.starts_with("https://");
            if remote {
                assert!(
                    source.ends_with(".localhost"),
                    "{name}: nur lokale Tauri-Ursprünge (*.localhost), gefunden {source}"
                );
            }
            assert_ne!(*source, "*", "{name}: keine Wildcard");
            assert_ne!(*source, "'unsafe-eval'", "{name}: kein eval");
            if *name != "style-src" {
                assert_ne!(*source, "'unsafe-inline'", "{name}: kein unsafe-inline");
            }
        }
    }
}
