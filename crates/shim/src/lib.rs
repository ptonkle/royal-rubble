//! tool_shim: forwards strings between the page and `tool_core::api`.
//!
//! Builder requirement: the shim decides nothing. Each method is one call
//! into the core and a conversion of its arguments. The page's float clock
//! (milliseconds) becomes whole milliseconds. `crates/core/tests/boundary.rs`
//! fails if a branch, a loop or the `?` operator appears here; if a feature
//! seems to need one, the rule belongs in the core. Ported from Fleeced.

use wasm_bindgen::prelude::*;

/// The page's one session with the core.
#[wasm_bindgen]
pub struct Game {
    g: tool_core::api::Game,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Game {
        Game {
            g: tool_core::api::Game::new(strings_json, notes_json, glossary_md),
        }
    }

    pub fn ui(&self) -> String {
        self.g.ui()
    }

    pub fn act(&mut self, name: &str, value: &str) -> String {
        self.g.act(name, value)
    }

    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now: f64) {
        self.g.net_event(kind, peer, text, host, now as i64);
    }

    /// `press`: the latest keydown's timeStamp, milliseconds, passed on in
    /// microseconds.
    pub fn net_pump(&mut self, now: f64, keys: u32, press: Option<f64>) -> String {
        self.g
            .net_pump(now as i64, keys, press.map(|p| (p * 1000.0) as i64))
    }
}
