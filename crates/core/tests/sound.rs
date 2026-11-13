//! The compiled sounds (data/sound-programs.json, from tools/sound/compile.py)
//! keep the volume low and name only sounds that exist. These tests read the
//! file directly; the page plays it as it is.

use serde_json::Value;
use std::fs;
use std::path::Path;

fn programs() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/sound-programs.json");
    let text = fs::read_to_string(&path).expect("read data/sound-programs.json");
    serde_json::from_str(&text).expect("parse data/sound-programs.json")
}

/// The low-volume rule from every game: a full slider plays at most 60% of
/// the level the sounds are written at. Compared as text so no float is used.
#[test]
fn a_full_slider_plays_at_most_sixty_percent() {
    let p = programs();
    let scale = p["setup"]["master_scale"].to_string();
    let allowed = ["0.6", "0.55", "0.5", "0.45", "0.4"];
    assert!(
        allowed.contains(&scale.as_str()),
        "master_scale is {scale}, want 0.6 or lower"
    );
}

/// Every group, tier, section and the urgent layer names a compiled sound,
/// so play(name) never finds nothing.
#[test]
fn every_named_sound_is_compiled() {
    let p = programs();
    let ids: Vec<&str> = p["programs"]
        .as_array()
        .expect("programs")
        .iter()
        .map(|x| x["id"].as_str().expect("id"))
        .collect();
    let mut named: Vec<String> = Vec::new();
    for (_, members) in p["groups"].as_object().expect("groups") {
        for m in members.as_array().expect("group list") {
            named.push(m.as_str().expect("group member").to_string());
        }
    }
    for (_, tiers) in p["tiers"].as_object().expect("tiers") {
        for t in tiers.as_array().expect("tier list") {
            named.push(t[1].as_str().expect("tier id").to_string());
        }
    }
    for s in p["sections"].as_array().expect("sections") {
        for id in s["ids"].as_array().expect("section ids") {
            named.push(id.as_str().expect("section id").to_string());
        }
    }
    named.push(
        p["setup"]["urgent_track"]
            .as_str()
            .expect("urgent_track")
            .to_string(),
    );
    for n in &named {
        assert!(ids.contains(&n.as_str()), "{n} is named but not compiled");
        assert!(p["labels"][n.as_str()].is_string(), "{n} has no label");
    }
}
