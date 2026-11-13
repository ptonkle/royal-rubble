//! The door into the core. The shim holds one `Game` and forwards its calls;
//! each returns a JSON string, `{"ok": ...}` on success or `{"error": "..."}`
//! with a message the page can show as it is.
//!
//! The core describes the screen as a view model (`view::Ui`): the screen's
//! name, its panels and their items, and the glossary's definitions. The page
//! builds the DOM from that description without knowing what a panel means.
//! Buttons, settings, flags and screens all come back through one function,
//! `act`, with a dotted name such as `menu.settings`, so a new option needs a
//! new `match` arm in `app.rs` and no new shim function. (After Peter's
//! Mandible; the session held here rather than a state round-trip through the
//! page is Honey Hole's.)

use serde::Serialize;

use crate::app::App;

fn ok<T: Serialize>(value: &T) -> String {
    serde_json::json!({ "ok": value }).to_string()
}

fn error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// The app for one page, or why it could not start.
#[derive(Debug)]
pub struct Game {
    app: Result<App, String>,
}

impl Game {
    /// Checks the strings file, the patch notes and the glossary and opens
    /// on the Title.
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Game {
        Game {
            app: App::new(strings_json, notes_json, glossary_md),
        }
    }

    /// What the page draws now.
    pub fn ui(&self) -> String {
        match &self.app {
            Ok(app) => ok(&app.ui()),
            Err(e) => error(e),
        }
    }

    /// Applies one named action and returns what the page draws next. A
    /// refused action leaves the app as it was.
    pub fn act(&mut self, name: &str, value: &str) -> String {
        match &mut self.app {
            Ok(app) => match app.act(name, value) {
                Ok(()) => ok(&app.ui()),
                Err(e) => error(&e),
            },
            Err(e) => error(e),
        }
    }

    /// One event from the transport (`web/online.js`).
    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now_ms: i64) {
        if let Ok(app) = &mut self.app {
            app.net_event(kind, peer, text, host, now_ms);
        }
    }

    /// One display frame: what to send, and what to ask of the transport.
    pub fn net_pump(&mut self, now_ms: i64, keys: u32) -> String {
        match &mut self.app {
            Ok(app) => ok(&app.pump(now_ms, keys)),
            Err(e) => error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::tests::sample;

    fn notes() -> String {
        let version = crate::app::VERSION;
        format!(r#"[{{"version": "{version}", "date": "2026-11-06", "notes": ["A."]}}]"#)
    }
    const GLOSSARY: &str = "### Keep\n\nThe tallest tower.\n";

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_new_game_opens_on_the_title_with_its_hints() {
        let v = json(&Game::new(&sample(), &notes(), GLOSSARY).ui());
        assert_eq!(v["ok"]["screen"], "title");
        assert_eq!(v["ok"]["hints"][0]["term"], "Keep");
        assert_eq!(v["ok"]["hints"][0]["definition"], "The tallest tower.");
    }

    #[test]
    fn a_bad_strings_file_comes_back_in_error() {
        let v = json(&Game::new("{}", &notes(), "").ui());
        assert_eq!(v["error"], "The strings file has no \"title\".");
    }

    #[test]
    fn a_bad_glossary_comes_back_in_error() {
        let v = json(&Game::new(&sample(), &notes(), "### Keep\n").ui());
        assert_eq!(v["error"], "The glossary entry \"Keep\" has no definition.");
    }

    #[test]
    fn act_goes_through_the_one_door() {
        let mut g = Game::new(&sample(), &notes(), "");
        let v = json(&g.act("menu.settings", ""));
        assert_eq!(v["ok"]["screen"], "settings");
    }

    #[test]
    fn act_refuses_an_unknown_action_in_error() {
        let mut g = Game::new(&sample(), &notes(), "");
        let v = json(&g.act("screen.jump", "start"));
        assert_eq!(v["error"], "Unknown action \"screen.jump\".");
    }
}
