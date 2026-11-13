//! The online kit's fence (`kits/online/README.md`). Copy this file to
//! `crates/core/tests/online_fence.rs` when the project takes the kit.
//!
//! One change to the kit's text, recorded in SECOND-ORDER.md (R1.306): the
//! saves kit's `web/save/store.js` is page JavaScript in a subdirectory. The
//! fence names that one path; boundary.rs scans it at any depth, so nothing
//! goes unread.
//!
//! The online feature is the one place a Builder project reaches another
//! origin, so it is kept in one directory and its addresses in one file:
//!
//! - `web/net/` holds the transport and nothing else, file by file;
//! - one page file (`TRANSPORT_DOOR`) imports the transport, and no other;
//! - `web/net/config.js` is the only file outside the vendored bundle that
//!   names a relay;
//! - every page's CSP `connect-src` is `'self'` plus exactly the relays in
//!   `config.js`, and no other directive names an address;
//! - the protocol tag in `config.js` is the core's `PROTOCOL`.
//!
//! The template's page scan (`boundary.rs`) reads only the top level of
//! `web/`, so it never reads `web/net/`. That is deliberate: the transport
//! names `RTCPeerConnection` and holds timings. This file is what watches it.
//!
//! When the project names its transport files differently, edit the
//! constants below, and record the change in SECOND-ORDER.md.

use std::fs;
use std::path::{Path, PathBuf};

/// Every file under `web/`, as a path relative to `web/`, that the
/// transport may consist of. A new file here is a decision, not a fix.
const NET_FILES: &[&str] = &[
    "net/config.js",
    "net/rtc.js",
    "net/vendor/README.md",
    "net/vendor/SHA256SUMS",
    "net/vendor/trystero-nostr-0.25.4.js",
];

/// The transport module that the page imports.
const TRANSPORT: &str = "net/rtc.js";

/// The one top-level page file allowed to import it.
const TRANSPORT_DOOR: &str = "online.js";

/// The only file outside the vendored bundle that may name a relay.
const CONFIG: &str = "net/config.js";

/// The saves kit's storage door: page JavaScript in a subdirectory, which
/// boundary.rs and the storage fence both read.
const STORE: &str = "save/store.js";

/// The vendored bundle's directory. Its own default relay list is not used
/// (the transport passes the list from `CONFIG`), and the file is pinned by
/// hash, so it is not read here.
const VENDOR: &str = "net/vendor/";

fn web_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web")
}

/// Every file under `dir`, relative to `dir`, with `/` separators, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap_or_else(|e| panic!("read {}: {e}", d.display())) {
            let p = entry.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).expect("under dir");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// The content of the first Content-Security-Policy meta element.
fn csp_of(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let at = lower.find("http-equiv=\"content-security-policy\"")?;
    let start = lower[..at].rfind('<')?;
    let end = at + lower[at..].find('>')?;
    let tag = &html[start..end];
    let from = tag.find("content=\"")? + "content=\"".len();
    let rest = &tag[from..];
    Some(rest[..rest.find('"')?].to_string())
}

/// The policy's directives as (name, sources).
fn directives(csp: &str) -> Vec<(String, Vec<String>)> {
    csp.split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut words = d.split_whitespace().map(str::to_string);
            let name = words.next().unwrap_or_default();
            (name, words.collect())
        })
        .collect()
}

/// Every quoted `wss://` address in a JavaScript text, in order.
fn relays_in(js: &str) -> Vec<String> {
    let mut out = Vec::new();
    for quote in ['"', '\''] {
        for (k, piece) in js.split(quote).enumerate() {
            if k % 2 == 1 && piece.starts_with("wss://") {
                out.push(piece.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The number after `protocol:` in the config, if there is exactly one.
fn protocol_in(js: &str) -> Option<u32> {
    let mut found = js.lines().filter_map(|line| {
        let rest = line.trim().strip_prefix("protocol:")?;
        rest.trim().trim_end_matches(',').trim().parse::<u32>().ok()
    });
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn read(rel: &str) -> String {
    let p = web_dir().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn web_net_holds_the_transport_and_nothing_else() {
    let all = files_under(&web_dir());
    let net: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|p| p.starts_with("net/"))
        .collect();
    assert_eq!(net, NET_FILES, "web/net holds the transport and nothing else");
    let elsewhere: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|p| p.contains('/') && !p.starts_with("net/"))
        .filter(|p| p.ends_with(".js") && *p != STORE)
        .collect();
    assert!(
        elsewhere.is_empty(),
        "page JavaScript in a subdirectory the page scan does not read: {elsewhere:?}"
    );
}

#[test]
fn only_one_page_file_imports_the_transport() {
    let mut importers = Vec::new();
    for rel in files_under(&web_dir()) {
        if rel.starts_with("net/") {
            continue;
        }
        let text = read(&rel);
        if text.contains(TRANSPORT) || text.contains(VENDOR) {
            importers.push(rel);
        }
    }
    assert_eq!(
        importers,
        [TRANSPORT_DOOR],
        "only {TRANSPORT_DOOR} may reach {TRANSPORT} or the vendored bundle"
    );
}

#[test]
fn only_the_config_names_a_relay() {
    let mut naming = Vec::new();
    for rel in files_under(&web_dir()) {
        if rel.starts_with(VENDOR) || rel == CONFIG || rel.ends_with(".html") {
            continue;
        }
        let text = read(&rel);
        if text.contains("://") {
            naming.push(rel);
        }
    }
    assert!(
        naming.is_empty(),
        "an address outside {CONFIG} (pages are checked by their CSP): {naming:?}"
    );
}

#[test]
fn every_page_connects_only_to_self_and_the_configured_relays() {
    let relays = relays_in(&read(CONFIG));
    assert!(
        !relays.is_empty(),
        "{CONFIG} names no wss:// relay; an empty list would make this test pass on nothing"
    );
    let mut wanted: Vec<String> = vec!["'self'".to_string()];
    wanted.extend(relays.iter().cloned());
    wanted.sort();

    let mut pages = 0;
    for rel in files_under(&web_dir()) {
        if !rel.ends_with(".html") {
            continue;
        }
        pages += 1;
        let csp = csp_of(&read(&rel)).unwrap_or_else(|| panic!("{rel}: no CSP"));
        let mut connect = None;
        for (name, mut sources) in directives(&csp) {
            if name == "connect-src" {
                sources.sort();
                connect = Some(sources);
            } else {
                let loose: Vec<&String> = sources
                    .iter()
                    .filter(|s| s.contains(':') && !s.starts_with('\''))
                    .filter(|s| *s != "data:" && *s != "blob:")
                    .collect();
                assert!(loose.is_empty(), "{rel}: {name} names {loose:?}");
            }
        }
        let connect = connect.unwrap_or_else(|| panic!("{rel}: CSP has no connect-src"));
        assert_eq!(
            connect, wanted,
            "{rel}: connect-src must be 'self' plus exactly the relays in {CONFIG}"
        );
    }
    assert!(pages > 0, "no HTML page found in web/");
}

#[test]
fn the_protocol_tag_is_the_core_s() {
    let tag = protocol_in(&read(CONFIG))
        .unwrap_or_else(|| panic!("{CONFIG} needs exactly one `protocol: <number>,` line"));
    assert_eq!(
        tag,
        tool_core::online::PROTOCOL,
        "the room name and the hello must agree"
    );
}

// ---- The readers are tests too: each must catch what it claims to. ----

#[test]
fn the_relay_reader_finds_each_quoted_address_and_nothing_else() {
    let js = "const a = [\"wss://one.example\", 'wss://two.example'];\n\
              // wss://in.a.comment is not quoted\n\
              const s = \"stun:stun.example:3478\"; const h = \"https://x.example\";";
    assert_eq!(
        relays_in(js),
        vec!["wss://one.example".to_string(), "wss://two.example".to_string()]
    );
    assert!(relays_in("export const CONFIG = { relays: [] };").is_empty());
}

#[test]
fn the_csp_reader_sees_an_extra_or_a_missing_relay() {
    let html = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self'; connect-src 'self' wss://one.example wss://extra.example; img-src 'self' https://cdn.example\">";
    let csp = csp_of(html).expect("csp");
    let d = directives(&csp);
    let connect = &d.iter().find(|(n, _)| n == "connect-src").expect("connect").1;
    assert_eq!(connect.len(), 3, "an extra relay is visible to the comparison");
    let img = &d.iter().find(|(n, _)| n == "img-src").expect("img").1;
    assert!(
        img.iter().any(|s| s.contains("://")),
        "an address in another directive is visible"
    );
    assert_eq!(csp_of("<meta charset=\"utf-8\">"), None);
}

#[test]
fn the_protocol_reader_wants_exactly_one_tag() {
    assert_eq!(protocol_in("  protocol: 3,\n"), Some(3));
    assert_eq!(protocol_in("  protocol: 3,\n  protocol: 4,\n"), None);
    assert_eq!(protocol_in("  appId: \"x\",\n"), None);
}
