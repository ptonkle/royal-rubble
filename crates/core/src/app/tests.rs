use super::*;
use crate::strings::tests::sample;

fn notes() -> String {
    format!(r#"[{{"version": "{VERSION}", "date": "2026-11-12", "notes": ["A."]}}]"#)
}
const GLOSSARY: &str = "### Keep\n\nThe tallest tower.\n";

fn app() -> App {
    let mut a = App::new(&sample(), &notes(), GLOSSARY).unwrap();
    a.act("flag.net_off", "").unwrap();
    a.act("flag.fast", "").unwrap();
    a.act("flag.client", "me").unwrap();
    a.act("flag.page_url", "https://example.test/rr/").unwrap();
    a
}

/// Every control id on the screen, in order, by walking the panels' JSON.
fn ids(app: &App) -> Vec<String> {
    let v = serde_json::to_value(app.ui()).unwrap();
    let mut out = Vec::new();
    for panel in v["panels"].as_array().unwrap() {
        for item in panel["items"].as_array().unwrap() {
            if let Some(id) = item["id"].as_str() {
                out.push(id.to_string());
            }
            for c in item["controls"].as_array().into_iter().flatten() {
                out.push(c["id"].as_str().unwrap().to_string());
            }
        }
    }
    out
}

#[test]
fn the_title_offers_bots_host_join_and_settings_and_the_version_opens_whats_new() {
    let mut a = app();
    assert_eq!(a.ui().screen, "title");
    assert_eq!(ids(&a), ["menu.solo", "menu.host", "menu.join", "menu.settings"]);
    assert_eq!(a.ui().version, VERSION);
    a.act("version", "").unwrap();
    assert_eq!(a.ui().screen, "notes");
    assert!(!a.ui().version_opens);
    a.act("menu.back", "").unwrap();
    assert_eq!(a.ui().screen, "title");
}

#[test]
fn the_build_refuses_patch_notes_that_do_not_start_at_its_version() {
    let old = r#"[{"version": "0.0.9", "date": "2026-11-06", "notes": ["A."]}]"#;
    assert_eq!(
        App::new(&sample(), old, "").unwrap_err(),
        format!("The patch notes start at 0.0.9, and this build is {VERSION}.")
    );
}

#[test]
fn sound_and_music_reach_the_save_and_a_reload_brings_them_back() {
    let mut a = app();
    a.act("menu.settings", "").unwrap();
    assert_eq!(ids(&a), ["settings.volume", "settings.music", "menu.back"]);
    a.act("settings.volume", "100").unwrap();
    a.act("settings.music", "30").unwrap();
    let text = a.ui().save;
    let mut b = app();
    b.act("save.load", &text).unwrap();
    let ui = b.ui();
    assert_eq!((ui.volume, ui.music), (100, 30));
    // worked by hand: 100*100*6/100 = 600; 30*30/10 = 90
    assert_eq!(ui.master_gain, "0.600");
    assert_eq!(ui.music_gain, "0.090");
    assert!(b.act("settings.music", "loud").is_err());
}

#[test]
fn the_tab_icon_comes_from_the_core() {
    assert!(app().ui().icon.starts_with("data:image/svg+xml,"));
}

#[test]
fn play_vs_bots_opens_a_lobby_with_a_bot_and_start_shows_the_placeholder() {
    let mut a = app();
    a.act("menu.solo", "").unwrap();
    assert_eq!(a.ui().screen, "lobby");
    let ids = ids(&a);
    assert!(ids.contains(&"seat.1.level".to_string()), "{ids:?}");
    assert!(!ids.contains(&"lobby.copy_code".to_string()), "no code offline");
    a.act("lobby.players", "4").unwrap();
    a.act("seat.3.level", "hard").unwrap();
    a.act("lobby.start", "").unwrap();
    a.pump(17, 0);
    assert_eq!(a.ui().screen, "play");
    assert_eq!(a.ui().panels[0].heading, "play.placeholder");
    a.act("play.leave", "").unwrap();
    assert_eq!(a.ui().screen, "title");
}

#[test]
fn hosting_shows_the_code_its_link_and_the_lobby_options() {
    let mut a = app();
    a.act("menu.host", "").unwrap();
    let ui = a.ui();
    assert_eq!(ui.screen, "lobby");
    let code = ui.hash.strip_prefix("#room=").unwrap().to_string();
    assert_eq!(code.len(), crate::online::room::CODE_LEN);
    let v = serde_json::to_value(&ui).unwrap();
    let items = v["panels"][0]["items"].as_array().unwrap();
    assert_eq!(items[0]["copy"], code.as_str());
    assert_eq!(items[1]["copy"], format!("https://example.test/rr/#room={code}"));
    let ids = ids(&a);
    for id in ["lobby.players", "lobby.wind", "lobby.start"] {
        assert!(ids.contains(&id.to_string()), "{id} in {ids:?}");
    }
}

#[test]
fn an_invite_link_joins_and_a_bad_code_says_so() {
    let mut a = app();
    a.act("flag.room", "abcdef").unwrap();
    assert_eq!(a.ui().screen, "lobby");
    assert_eq!(a.ui().hash, "#room=ABCDEF");
    a.act("lobby.leave", "").unwrap();
    a.act("menu.join", "").unwrap();
    a.act("online.code", "nope").unwrap();
    a.act("online.join", "").unwrap();
    assert_eq!(a.ui().screen, "online");
    assert_eq!(a.ui().status, "online.bad_code");
}
