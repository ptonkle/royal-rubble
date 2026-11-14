//! The app: which screen shows, the player's settings, and what every action
//! does. `api.rs` holds one `App` for the page and forwards to it; `ui` is
//! everything the page draws. No clock and no randomness: the page hands in
//! a seed and the time. Ported from Fleeced's `app.rs` (after Crabble's).

pub mod arena;
mod bot_names;
pub mod demo;
pub mod field;
mod guide;
pub mod hud_icon;
mod lobby_view;
pub mod sandbox;

use std::collections::BTreeMap;

use crate::lobby::clean_name;
use crate::online::room::{clean_code, Kind, Opening, Pump, Request, Room, Stage};
use crate::online::APP_ID;
use crate::rng::Rng;
use crate::save::{self, Loaded, Save};
use crate::strings::Strings;
use crate::view::{Hint, Item, NoteEntry, Panel, Ui};
use crate::{glossary, icon, patch_notes};

/// The build's version: the version button's text, and the top row of
/// `data/patch-notes.json`. `scripts/bump.sh` changes the two together.
pub const VERSION: &str = "0.2.16";

/// The sound sliders' step, percent.
const VOLUME_STEP: u32 = 5;
/// The sound sliders' top, percent.
const VOLUME_MAX: u32 = 100;

/// The menu theme, on every screen outside a match (R1.3802).
const MENU_MUSIC: &str = "music_menu";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Settings,
    Notes,
    /// Join with code: the code field.
    Online,
    /// A joiner's pasted-code form, until the host's lobby is heard.
    Paste,
    Lobby,
    Play,
    /// The playfield with no rules: build, test and throw.
    Sandbox,
    /// How to Play, a tab at a time.
    How,
    /// Every upgrade with its icon, filtered by tier.
    Upgrades,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Screen::Title => "title",
            Screen::Settings => "settings",
            Screen::Notes => "notes",
            Screen::Online => "online",
            Screen::Paste => "paste",
            Screen::Lobby => "lobby",
            Screen::Play => "play",
            Screen::Sandbox => "sandbox",
            Screen::How => "how",
            Screen::Upgrades => "upgrades",
        }
    }
}

/// The in-match pause menu (Esc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pause {
    Off,
    Menu,
    Settings,
}

/// The page's one app.
#[derive(Debug, Clone)]
pub struct App {
    strings: Strings,
    notes: Vec<patch_notes::Entry>,
    hints: Vec<Hint>,
    screen: Screen,
    /// How to Play's open tab, a value from `guide::HOW_TABS`.
    how_tab: &'static str,
    /// The upgrade list's tier filter, a value from `guide::TIER_FILTERS`.
    tier_filter: &'static str,
    /// Where Back goes from Settings and What's New.
    back: Screen,
    save: Save,
    /// A save this build refused, until the page has taken it.
    keep_refused: String,
    /// The status key that says why, for as long as Settings can offer it.
    refused_why: &'static str,
    /// Settings' button asked for the refused save as a file.
    offer_old_file: bool,
    /// `?fast`: the browser checks skip what only makes a person wait.
    fast: bool,
    /// `?codes`: show the invite-code buttons on the Join screen.
    codes: bool,
    /// `?net=off`: rooms open with no transport, for the checks.
    no_net: bool,
    /// This browser's id, stable for its tab, from the page.
    client: String,
    /// The build's stamp: a different one is turned away at the door.
    build: String,
    /// The page's own address, for the invite link.
    page_url: String,
    rng: Rng,
    /// The room this browser is in.
    room: Option<Room>,
    /// A room just left: the transport still has to hear it.
    leaving: Vec<Request>,
    /// The Join screen's code field, and whether Join refused it.
    typed_code: String,
    bad_code: bool,
    /// The pasted-code forms' fields: a joiner's invite, a host's reply.
    typed_invite: String,
    typed_reply: String,
    /// The latest time the page handed in.
    now_ms: i64,
    /// The sandbox, while it shows.
    sandbox: Option<sandbox::Sandbox>,
    /// The match against bots, from Start to the results.
    arena: Option<arena::Arena>,
    /// The upgrade cards' names and texts (data/upgrades.json).
    cards: arena::CardText,
    /// The pause menu over a match.
    pause: Pause,
    /// When a solo match was paused: its clock skips the time paused.
    paused_at: Option<i64>,
    /// The siege behind the menus, while one of them shows.
    demo: Option<demo::Demo>,
    /// Volume slider moves so far: each plays the sample cue.
    samples: u32,
}

fn percent(value: &str) -> Result<u32, String> {
    let v: u32 = value
        .parse()
        .map_err(|_| format!("\"{value}\" is not a sound level."))?;
    Ok(v.min(VOLUME_MAX))
}

/// A gain in thousandths as the decimal the page hands to Web Audio.
fn decimal(milli: u32) -> String {
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

impl App {
    /// Checks the strings file, the patch notes and the glossary and opens
    /// on the Title.
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Result<App, String> {
        let strings = Strings::parse(strings_json).map_err(|e| e.message())?;
        let notes = patch_notes::parse(notes_json)?;
        if notes[0].version != VERSION {
            return Err(format!(
                "The patch notes start at {}, and this build is {VERSION}.",
                notes[0].version
            ));
        }
        let hints = glossary::parse(glossary_md)
            .map_err(|e| e.message())?
            .into_iter()
            .map(|e| Hint {
                term: e.term,
                definition: e.definition,
            })
            .collect();
        Ok(App {
            strings,
            notes,
            hints,
            screen: Screen::Title,
            how_tab: guide::HOW_TABS[0].0,
            tier_filter: guide::TIER_FILTERS[0],
            back: Screen::Title,
            save: save::new_game(),
            keep_refused: String::new(),
            refused_why: "",
            offer_old_file: false,
            fast: false,
            codes: false,
            no_net: false,
            client: String::new(),
            build: String::new(),
            page_url: String::new(),
            rng: Rng::new(0),
            room: None,
            leaving: Vec::new(),
            typed_code: String::new(),
            bad_code: false,
            typed_invite: String::new(),
            typed_reply: String::new(),
            now_ms: 0,
            sandbox: None,
            arena: None,
            cards: arena::CardText::default(),
            pause: Pause::Off,
            paused_at: None,
            demo: None,
            samples: 0,
        })
    }

    /// Opens a screen that Back returns from.
    fn open(&mut self, screen: Screen) {
        self.back = self.screen;
        self.screen = screen;
    }

    /// The stored text, read once as the page loads.
    fn load_save(&mut self, text: &str) {
        match save::load(text) {
            Loaded::Fresh => {}
            Loaded::Ready(s) => self.save = s,
            Loaded::Refused { why, kept } => {
                self.keep_refused = kept;
                self.refused_why = why;
            }
        }
    }

    /// What a new room needs from the page.
    fn opening(&mut self) -> Opening {
        Opening {
            client: self.client.clone(),
            name: self.save.name.clone(),
            build: self.build.clone(),
            app_id: APP_ID.to_string(),
            seed: self.rng.next_u64(),
            fast: self.fast,
            no_net: self.no_net,
        }
    }

    fn enter(&mut self, room: Room) {
        self.room = Some(room);
        self.bad_code = false;
        self.back = Screen::Title;
        self.screen = Screen::Lobby;
    }

    /// Joins by a typed or linked code, or says it is wrong.
    fn join(&mut self, code: &str) {
        match clean_code(code) {
            Some(code) => {
                let how = self.opening();
                self.enter(Room::join(&how, &code, self.now_ms));
            }
            None => {
                self.typed_code = code.to_string();
                self.bad_code = true;
                if self.screen != Screen::Online {
                    self.open(Screen::Online);
                }
            }
        }
    }

    /// Opens the pause menu over the match. A solo match stops its clock.
    fn pause(&mut self) {
        if self.pause == Pause::Off && self.arena.as_ref().is_some_and(|a| !a.is_online()) {
            self.paused_at = Some(self.now_ms);
        }
        self.pause = Pause::Menu;
    }

    /// Closes the pause menu; a solo match picks up where it stopped.
    fn resume(&mut self) {
        if let (Some(at), Some(a)) = (self.paused_at.take(), &mut self.arena) {
            a.hold(self.now_ms - at);
        }
        self.pause = Pause::Off;
    }

    fn leave(&mut self) {
        self.pause = Pause::Off;
        self.paused_at = None;
        self.arena = None;
        if self.room.take().is_some_and(|r| r.kind != Kind::Solo) {
            self.leaving.push(Request::Leave);
        }
        self.typed_invite.clear();
        self.typed_reply.clear();
        self.back = Screen::Title;
        self.screen = Screen::Title;
    }

    /// Applies one named action. A refused action leaves the app as it was.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "menu.solo" => {
                let how = self.opening();
                self.enter(Room::solo(&how));
            }
            "menu.host" => {
                let how = self.opening();
                self.enter(Room::host(&how, self.now_ms));
            }
            "menu.join" => self.open(Screen::Online),
            "menu.sandbox" => {
                self.sandbox = Some(sandbox::Sandbox::new(self.rng.next_u64()));
                self.back = Screen::Title;
                self.screen = Screen::Sandbox;
            }
            "sandbox.leave" => {
                self.sandbox = None;
                self.screen = Screen::Title;
            }
            "field.size" | "field.down" | "field.move" | "field.up" | "field.wheel" | "sandbox.tool"
            | "sandbox.ammo" | "sandbox.air" | "sandbox.test" | "sandbox.mode"
                if self.arena.is_none() || name.starts_with("sandbox.") =>
            {
                let Some(s) = &mut self.sandbox else {
                    return Err(format!("Unknown action \"{name}\"."));
                };
                match name {
                    "field.size" => s.set_size(value)?,
                    "field.down" => s.down(value)?,
                    "field.move" => s.moved(value)?,
                    "field.up" => s.up(value)?,
                    // the sandbox keeps its fixed view
                    "field.wheel" => {}
                    "sandbox.tool" => s.set_tool(value)?,
                    "sandbox.ammo" => s.set_ammo(value)?,
                    "sandbox.air" => s.set_air(value)?,
                    "sandbox.test" => s.run_test(),
                    _ => s.toggle_mode(),
                }
            }
            "data.upgrades" => self.cards = arena::CardText::parse(value)?,
            "pause.open" if self.arena.is_some() && self.screen == Screen::Play => self.pause(),
            "pause.resume" => self.resume(),
            "pause.settings" if self.pause != Pause::Off => self.pause = Pause::Settings,
            "pause.back" if self.pause != Pause::Off => self.pause = Pause::Menu,
            "match.again" => {
                let seed = self.rng.next_u64();
                let next = match &self.arena {
                    Some(a) if !a.is_online() => a.again(seed, &self.strings)?,
                    _ => return Err(format!("Unknown action \"{name}\".")),
                };
                self.arena = Some(next);
            }
            n if n.starts_with("match.") || (self.arena.is_some() && n.starts_with("field.")) => {
                let Some(a) = &mut self.arena else {
                    return Err(format!("Unknown action \"{name}\"."));
                };
                match n {
                    "field.size" => a.set_size(value)?,
                    "field.down" => a.down(value)?,
                    "field.move" => a.moved(value)?,
                    "field.up" => a.up(value)?,
                    "field.wheel" => a.wheel(value)?,
                    _ => a.act(n, value)?,
                }
            }
            "menu.settings" => self.open(Screen::Settings),
            "menu.back" => {
                self.screen = self.back;
                // How to Play is a level below the title, whichever screen
                // came back to it
                if self.screen == Screen::How {
                    self.back = Screen::Title;
                }
            }
            "menu.how" => {
                self.how_tab = guide::HOW_TABS[0].0;
                self.open(Screen::How);
            }
            "how.tab" => {
                self.how_tab = guide::HOW_TABS
                    .iter()
                    .map(|t| t.0)
                    .find(|t| *t == value)
                    .ok_or_else(|| format!("\"{value}\" is not a How to Play tab."))?;
            }
            "menu.upgrades" => {
                self.tier_filter = guide::TIER_FILTERS[0];
                self.open(Screen::Upgrades);
            }
            "upgrades.tier" => {
                self.tier_filter = guide::TIER_FILTERS
                    .iter()
                    .copied()
                    .find(|t| *t == value)
                    .ok_or_else(|| format!("\"{value}\" is not a tier."))?;
            }
            "version" => {
                if self.screen != Screen::Notes {
                    self.open(Screen::Notes);
                }
            }
            "settings.volume" => {
                self.save.volume = percent(value)?;
                self.samples += 1;
            }
            "settings.music" => {
                self.save.music = percent(value)?;
                self.samples += 1;
            }
            "save.load" => self.load_save(value),
            // the page has the refused text now, and Settings still offers it
            "save.kept" => self.keep_refused.clear(),
            "save.old_file" => self.offer_old_file = true,
            "save.offered" => self.offer_old_file = false,
            "online.code" => {
                self.typed_code = value.to_string();
                self.bad_code = false;
            }
            "online.join" => self.join(&self.typed_code.clone()),
            "online.paste_host" => {
                let how = self.opening();
                self.enter(Room::host_by_code(&how));
            }
            "online.paste_join" => self.open(Screen::Paste),
            "online.offer_in" => self.typed_invite = value.to_string(),
            // the form stays up, so a refused invite can be pasted again
            "online.make_answer" => {
                let how = self.opening();
                self.room = Some(Room::join_by_code(&how, &self.typed_invite));
            }
            "online.answer_in" => self.typed_reply = value.to_string(),
            "online.accept_answer" => {
                if let Some(room) = &mut self.room {
                    room.accept_reply(&self.typed_reply);
                }
                self.typed_reply.clear();
            }
            "online.paste_back" => {
                self.leave();
                self.screen = Screen::Online;
            }
            "lobby.leave" | "play.leave" => self.leave(),
            "flag.fast" => self.fast = true,
            "flag.codes" => self.codes = true,
            "flag.net_off" => self.no_net = true,
            "flag.client" => self.client = value.to_string(),
            "flag.build" => self.build = value.to_string(),
            "flag.page_url" => self.page_url = value.to_string(),
            "flag.seed" => {
                let seed: u64 = value.parse().map_err(|_| format!("\"{value}\" is not a seed."))?;
                self.rng = Rng::new(seed);
            }
            "flag.room" => self.join(value),
            other => {
                let Some(room) = &mut self.room else {
                    return Err(format!("Unknown action \"{other}\"."));
                };
                room.act(other, value)?;
                if other == "lobby.name" {
                    self.save.name = clean_name(value);
                }
            }
        }
        Ok(())
    }

    /// One event from the transport. With no room open there is no one to
    /// hear it: a room just left can still send a last word.
    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now_ms: i64) {
        if let Some(room) = &mut self.room {
            room.event(kind, peer, text, host, now_ms);
        }
    }

    /// One display frame: the room's work, and the screen following it.
    pub fn pump(&mut self, now_ms: i64, keys: u32) -> Pump {
        self.pump_pressed(now_ms, keys, None)
    }

    /// As `pump`, with when the latest key went down (microseconds, the
    /// page's clock), so a release lands between ticks (R1.2103).
    pub fn pump_pressed(&mut self, now_ms: i64, keys: u32, press_us: Option<i64>) -> Pump {
        self.now_ms = now_ms;
        if self.menu_backdrop() {
            if self.demo.is_none() {
                self.demo = Some(demo::Demo::new(self.rng.next_u64()));
            }
        } else {
            self.demo = None;
        }
        if let Some(d) = &mut self.demo {
            d.pump(now_ms);
        }
        if let Some(s) = &mut self.sandbox {
            s.pump(now_ms, keys, press_us);
        }
        self.start_arena();
        if self.arena.is_none() {
            self.pause = Pause::Off;
            self.paused_at = None;
        }
        if let Some(a) = &mut self.arena {
            let net_tick = self.room.as_ref().and_then(|r| r.last_tick_us());
            match (self.pause, self.paused_at) {
                (Pause::Off, _) => a.pump(now_ms, keys, press_us, net_tick),
                // a solo match waits; an online one plays on with no keys
                (_, Some(_)) => {}
                (_, None) => a.pump(now_ms, 0, None, net_tick),
            }
        }
        if let (Some(a), Some(r)) = (&mut self.arena, &mut self.room) {
            if a.is_online() {
                for i in a.take_inputs() {
                    r.queue(i);
                }
            }
        }
        let mut pump = self.room.as_mut().map_or_else(Pump::default, |r| r.pump(now_ms));
        if let (Some(a), Some(r)) = (&mut self.arena, &self.room) {
            match &r.game {
                Some(g) if a.is_online() => a.follow(g, r.session.tick),
                None if a.is_online() => self.arena = None,
                _ => {}
            }
        }
        pump.requests.append(&mut self.leaving);
        if self.screen == Screen::Paste && self.room.as_ref().is_some_and(|r| r.heard) {
            self.screen = Screen::Lobby;
        }
        let started = self.room.as_ref().is_some_and(|r| r.stage != Stage::Lobby);
        if started && self.screen == Screen::Lobby {
            self.screen = Screen::Play;
        }
        if !started && self.screen == Screen::Play {
            self.screen = Screen::Lobby;
        }
        pump
    }

    /// A room that has started gets its match screen: solo, the arena runs
    /// the match; online, it follows the room's once the countdown ends.
    fn start_arena(&mut self) {
        if self.arena.is_some() {
            return;
        }
        let Some(room) = &self.room else { return };
        let Some(start) = &room.start else { return };
        let made = match (&room.game, room.stage) {
            (_, Stage::Lobby) => return,
            (None, _) if room.kind == Kind::Solo => {
                arena::Arena::new(start, &self.client, self.fast, &self.strings)
            }
            (Some(g), Stage::Playing) => {
                arena::Arena::online(start, &self.client, &self.strings, g, room.session.tick)
            }
            _ => return,
        };
        if let Ok(a) = made {
            self.arena = Some(a);
        }
    }

    /// The match stopped because the browsers disagree.
    fn out_of_sync_panels(&self) -> Vec<Panel> {
        vec![Panel::new(
            "play",
            self.strings.get("online.desync"),
            vec![self.quiet("play.leave", "play.leave")],
        )]
    }

    fn button(&self, id: &str, key: &str) -> Item {
        Item::Button {
            tip: String::new(),
            icon: String::new(),
            cost: String::new(),
            id: id.into(),
            text: self.strings.get(key),
            quiet: false,
            disabled: false,
        }
    }

    fn quiet(&self, id: &str, key: &str) -> Item {
        Item::Button {
            tip: String::new(),
            icon: String::new(),
            cost: String::new(),
            id: id.into(),
            text: self.strings.get(key),
            quiet: true,
            disabled: false,
        }
    }

    fn title_panels(&self) -> Vec<Panel> {
        vec![Panel::new(
            "title",
            self.strings.get("title"),
            vec![
                self.button("menu.solo", "menu.solo"),
                self.button("menu.host", "menu.host"),
                self.button("menu.join", "menu.join"),
                self.button("menu.sandbox", "menu.sandbox"),
                self.quiet("menu.how", "menu.how"),
                self.quiet("menu.upgrades", "menu.upgrades"),
                self.quiet("menu.settings", "menu.settings"),
            ],
        )]
    }

    fn slider(&self, id: &str, key: &str, value: u32) -> Item {
        Item::Slider {
            id: id.into(),
            label: self.strings.get(key),
            min: 0,
            max: VOLUME_MAX,
            step: VOLUME_STEP,
            value,
            text: format!("{value}%"),
        }
    }

    /// The pause menu over the match: Resume, Settings, Leave.
    fn pause_panels(&self) -> Vec<Panel> {
        match self.pause {
            Pause::Settings => {
                let mut p = self.settings_panels();
                for panel in &mut p {
                    panel.kind = "pause".into();
                    for item in &mut panel.items {
                        if let Item::Button { id, .. } = item {
                            if id == "menu.back" {
                                *id = "pause.back".into();
                            }
                        }
                    }
                }
                p
            }
            _ => vec![Panel::new(
                "pause",
                self.strings.get("pause.heading"),
                vec![
                    self.button("pause.resume", "pause.resume"),
                    self.quiet("pause.settings", "menu.settings"),
                    self.quiet("play.leave", "pause.leave"),
                ],
            )],
        }
    }

    /// The menus that show the siege behind them.
    fn menu_backdrop(&self) -> bool {
        matches!(
            self.screen,
            Screen::Title
                | Screen::Settings
                | Screen::Notes
                | Screen::Online
                | Screen::How
                | Screen::Upgrades
        ) && self.room.is_none()
    }

    /// What Esc does on this screen, as an action name, or empty.
    fn escape(&self) -> &'static str {
        match (self.screen, self.pause) {
            // Esc first cancels a segment's pending start (R1.3300)
            (Screen::Play, Pause::Off) if self.arena.as_ref().is_some_and(|a| !a.start_pending()) => {
                "pause.open"
            }
            (Screen::Play, Pause::Menu) => "pause.resume",
            (Screen::Play, Pause::Settings) => "pause.back",
            (Screen::Settings | Screen::Notes | Screen::Online | Screen::How | Screen::Upgrades, _) => {
                "menu.back"
            }
            _ => "",
        }
    }

    fn settings_panels(&self) -> Vec<Panel> {
        let mut items = vec![
            self.slider("settings.volume", "settings.sound", self.save.volume),
            self.slider("settings.music", "settings.music", self.save.music),
        ];
        if !self.refused_why.is_empty() {
            items.push(Item::Lines {
                id: String::new(),
                lines: vec![self.strings.get(self.refused_why)],
            });
            items.push(self.quiet("save.old_file", "save.old_file"));
        }
        items.push(self.quiet("menu.back", "menu.back"));
        vec![Panel::new(
            "settings",
            self.strings.get("settings.heading"),
            items,
        )]
    }

    fn notes_panels(&self) -> Vec<Panel> {
        let entries = self
            .notes
            .iter()
            .map(|e| NoteEntry {
                version: e.version.clone(),
                date: e.date.clone(),
                minor: e.minor(),
                lines: e.notes.clone(),
            })
            .collect();
        vec![Panel::new(
            "notes",
            self.strings.get("notes.heading"),
            vec![
                Item::Notes {
                    id: "notes.list".into(),
                    entries,
                },
                self.quiet("menu.back", "menu.back"),
            ],
        )]
    }

    fn online_panels(&self) -> Vec<Panel> {
        let mut items = vec![
            Item::Text {
                id: "online.code".into(),
                label: self.strings.get("online.code"),
                value: self.typed_code.clone(),
                max: crate::online::room::CODE_LEN as u32,
                placeholder: String::new(),
            },
            self.button("online.join", "online.join"),
        ];
        if self.codes {
            items.push(self.quiet("online.paste_host", "online.paste_host"));
            items.push(self.quiet("online.paste_join", "online.paste_join"));
        }
        items.push(self.quiet("menu.back", "menu.back"));
        vec![Panel::new("online", self.strings.get("online.heading"), items)]
    }

    /// Everything the page draws now.
    pub fn ui(&self) -> Ui {
        let room = self.room.as_ref();
        let panels = match (self.screen, room) {
            (Screen::Settings, _) => self.settings_panels(),
            (Screen::Notes, _) => self.notes_panels(),
            (Screen::Online, _) => self.online_panels(),
            (Screen::How, _) => guide::how_panels(&self.strings, self.how_tab),
            (Screen::Upgrades, _) => guide::upgrades_panels(&self.strings, &self.cards, self.tier_filter),
            (Screen::Paste, r) => self.paste_panels(r),
            (Screen::Lobby, Some(r)) => self.lobby_panels(r),
            (Screen::Play, Some(r)) if self.arena.is_some() && r.out_of_sync() => self.out_of_sync_panels(),
            (Screen::Play, Some(_)) if self.arena.is_some() && self.pause != Pause::Off => {
                self.pause_panels()
            }
            (Screen::Play, Some(_)) if self.arena.is_some() => self
                .arena
                .as_ref()
                .map_or_else(Vec::new, |a| a.panels(&self.strings, &self.cards)),
            (Screen::Play, Some(r)) => self.play_panels(r),
            (Screen::Sandbox, _) => self
                .sandbox
                .as_ref()
                .map_or_else(|| self.title_panels(), |s| s.panels(&self.strings)),
            (Screen::Title | Screen::Lobby | Screen::Play, _) => self.title_panels(),
        };
        let sandbox = self.sandbox.as_ref().filter(|_| self.screen == Screen::Sandbox);
        let arena = self.arena.as_ref().filter(|_| self.screen == Screen::Play);
        let playing = (self.screen == Screen::Play && room.is_some_and(|r| r.stage == Stage::Playing))
            || sandbox.is_some();
        let playing = playing && self.pause == Pause::Off;
        let status = match room {
            Some(r) => r.status(),
            None if self.bad_code => "online.bad_code",
            None => "",
        };
        Ui {
            screen: if arena.is_some() {
                "match".into()
            } else {
                self.screen.name().into()
            },
            title: self.strings.get("title"),
            version: VERSION.into(),
            version_opens: !matches!(self.screen, Screen::Notes | Screen::Play | Screen::Sandbox),
            version_shown: arena.is_none() && !matches!(self.screen, Screen::Play | Screen::Sandbox),
            panels,
            hints: self.hints.clone(),
            icon: icon::data_url(),
            save: save::store(&self.save),
            keep_refused: self.keep_refused.clone(),
            offer_old_file: self.offer_old_file,
            volume: self.save.volume,
            music: self.save.music,
            master_gain: decimal(save::master_gain(self.save.volume)),
            music_gain: decimal(save::music_gain(self.save.music)),
            sound_share: decimal(self.save.volume * 10),
            music_share: decimal(self.save.music * 10),
            live: room.is_some() || sandbox.is_some() || self.menu_backdrop(),
            capture: playing,
            keys: if arena.is_some() {
                arena::keys()
            } else if sandbox.is_some() {
                sandbox::keys()
            } else {
                BTreeMap::new()
            },
            hash: room
                .filter(|r| !r.code.is_empty())
                .map_or_else(String::new, |r| format!("#room={}", r.code)),
            status: if status.is_empty() {
                String::new()
            } else {
                self.strings.get(status)
            },
            menu_music: if matches!(self.screen, Screen::Play | Screen::Sandbox) {
                String::new()
            } else {
                MENU_MUSIC.into()
            },
            field: arena
                .map(|a| a.frame())
                .or_else(|| sandbox.map(|s| s.frame()))
                .or_else(|| {
                    self.demo
                        .as_ref()
                        .filter(|_| self.menu_backdrop())
                        .map(|d| d.frame())
                }),
            samples: self.samples,
            sample_cue: "sample".into(),
            escape: self.escape().into(),
            sync: room
                .filter(|r| r.game.is_some())
                .map_or_else(String::new, lobby_view::state_line),
        }
    }
}

#[cfg(test)]
mod tests;
