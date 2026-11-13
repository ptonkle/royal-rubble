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
    assert_eq!(
        ids(&a),
        [
            "menu.solo",
            "menu.host",
            "menu.join",
            "menu.sandbox",
            "menu.settings"
        ]
    );
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
fn play_vs_bots_opens_a_lobby_with_a_bot_and_start_shows_the_match() {
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
    // the solo room's match is a real one: the match screen's HUD
    assert_eq!(a.ui().screen, "match");
    assert_eq!(a.ui().panels[0].kind, "field-top");
    assert!(a.ui().field.is_some());
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

fn wire_frame(now: &mut i64, host: &mut App, join: &mut App) {
    *now += 17;
    for m in host.pump(*now, 0).out {
        join.net_event("message", "me", &m.text, true, *now);
    }
    for m in join.pump(*now, 0).out {
        host.net_event("message", "you", &m.text, false, *now);
    }
}

/// Two browsers' apps joined by a wire: the host's Start runs the real
/// match on both, a Ready from each reaches both, and both agree (R1.1502).
#[test]
fn online_two_apps_play_the_same_match_from_the_lobby() {
    let mut host = app();
    host.act("menu.host", "").unwrap();
    let code = host.room.as_ref().unwrap().code.clone();
    let mut join = app();
    join.act("flag.client", "you").unwrap();
    join.act("flag.room", &code).unwrap();
    host.net_event("connected", "you", "", false, 0);
    join.net_event("connected", "me", "", true, 0);
    let mut now = 0;
    for _ in 0..6 {
        wire_frame(&mut now, &mut host, &mut join);
    }
    join.act("lobby.ready", "on").unwrap();
    for _ in 0..6 {
        wire_frame(&mut now, &mut host, &mut join);
    }
    host.act("lobby.start", "").unwrap();
    for _ in 0..10 {
        wire_frame(&mut now, &mut host, &mut join);
    }
    assert_eq!(host.ui().screen, "match");
    assert_eq!(join.ui().screen, "match");
    host.act("match.ready", "").unwrap();
    join.act("match.ready", "").unwrap();
    for _ in 0..200 {
        wire_frame(&mut now, &mut host, &mut join);
    }
    let phase = |a: &App| a.arena.as_ref().unwrap().play.rules.phase;
    assert_ne!(
        phase(&host),
        crate::game::flow::Phase::Build,
        "both Readys ended the build"
    );
    assert_eq!(phase(&host), phase(&join));
    let (h, j) = (host.ui().sync, join.ui().sync);
    assert!(!h.ends_with(": 0"), "a checksum was compared: {h}");
    assert_eq!(h, j);
}

/// A solo match against one bot, running.
fn solo_match() -> App {
    let mut a = app();
    a.act("menu.solo", "").unwrap();
    a.act("lobby.start", "").unwrap();
    a.pump(17, 0);
    a.pump(1017, 0);
    a
}

#[test]
fn pause_menu_stops_a_solo_match_and_resume_skips_the_time_paused() {
    let mut a = solo_match();
    let before = a.arena.as_ref().unwrap().ticks_run();
    a.act("pause.open", "").unwrap();
    assert_eq!(ids(&a), ["pause.resume", "pause.settings", "play.leave"]);
    assert!(!a.ui().capture, "the keys are the menu's while paused");
    assert!(a.ui().field.is_some(), "the match stays drawn behind");
    a.pump(9017, 0);
    assert_eq!(a.arena.as_ref().unwrap().ticks_run(), before);
    a.act("pause.resume", "").unwrap();
    a.pump(9017, 0);
    // eight seconds paused are not caught up on resume
    assert_eq!(a.arena.as_ref().unwrap().ticks_run(), before);
    assert!(a.ui().capture);
}

#[test]
fn pause_escape_walks_pause_settings_and_back() {
    let mut a = solo_match();
    assert_eq!(a.ui().escape, "pause.open");
    a.act("pause.open", "").unwrap();
    assert_eq!(a.ui().escape, "pause.resume");
    a.act("pause.settings", "").unwrap();
    assert_eq!(ids(&a), ["settings.volume", "settings.music", "pause.back"]);
    assert_eq!(a.ui().escape, "pause.back");
    a.act("settings.music", "40").unwrap();
    assert_eq!(a.ui().music, 40);
    a.act("pause.back", "").unwrap();
    assert_eq!(a.ui().escape, "pause.resume");
    a.act("play.leave", "").unwrap();
    assert_eq!(a.ui().screen, "title");
    assert_eq!(a.ui().escape, "");
    a.act("menu.settings", "").unwrap();
    assert_eq!(a.ui().escape, "menu.back");
}
