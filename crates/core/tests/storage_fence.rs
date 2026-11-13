//! Saves kit: D17 changed. Only web/save/store.js may name browser storage,
//! and only the save the core writes goes through it. This reads
//! every other file under web/ (vendor/ aside) and fails if one names a browser
//! storage API. Replaces the template's no_storage.rs.

use std::fs;
use std::path::{Path, PathBuf};

const STORAGE_NAMES: &[&str] = &[
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "IDBFactory",
    "document.cookie",
    "caches",
    "CacheStorage",
    "serviceWorker",
    "navigator.storage",
    "openDatabase",
];

fn storage_names_in(text: &str) -> Vec<&'static str> {
    STORAGE_NAMES
        .iter()
        .copied()
        .filter(|name| text.contains(name))
        .collect()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else {
            continue;
        };
        for entry in read {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if path.file_name().map(|n| n != "vendor").unwrap_or(true) {
                    stack.push(path);
                }
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn only_the_save_store_names_browser_storage() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let mut found = Vec::new();
    for path in files_under(&web) {
        if path.ends_with("save/store.js") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for name in storage_names_in(&text) {
            found.push(format!("{}: {name}", path.display()));
        }
    }
    assert!(
        found.is_empty(),
        "browser storage outside web/save/store.js (saves kit, D17):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_store_uses_local_storage_and_nothing_else() {
    let store = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/save/store.js");
    let text = fs::read_to_string(store).expect("web/save/store.js, from kits/saves");
    assert_eq!(storage_names_in(&text), vec!["localStorage"]);
}

#[test]
fn the_scanner_sees_storage() {
    assert_eq!(
        storage_names_in("window.localStorage.setItem('n', name)"),
        vec!["localStorage"]
    );
    assert_eq!(storage_names_in("indexedDB.open('x')"), vec!["indexedDB"]);
    assert!(storage_names_in("el(\"note\").value").is_empty());
}
