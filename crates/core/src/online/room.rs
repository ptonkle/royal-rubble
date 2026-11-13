//! One browser's room: the lobby, the rollback session under it, and the
//! finder before them. `app.rs` holds one `Room` while the player is in a
//! lobby or a match. The transport's events come in through `event`, the
//! player's through `act`, and `pump` runs once a frame and says what to
//! send. Solo is a room with no one to meet, so a solo match and an online
//! one share every rule of the lobby.
//!
//! The host's lobby is the truth. A joiner asks (`Msg::Ask`) and sees the
//! answer in the next roster; the host checks that each ask is about the
//! joiner's own seat.
//!
//! A room can also meet by pasted codes, with no relay: [`Pasted`] stands in
//! for the finder, and the host and joiner swap an invite code and a reply
//! code by hand. From the channel down the two kinds of room are the same.
//!
//! `Room::begin` builds the match every browser runs from `MatchStart`
//! (`play::online::NetMatch`), and `Room::pump` runs it through the session
//! with this browser's queued inputs, one a tick (R1.403). A solo room
//! leaves the match to `app::arena`, which steps it alone.
//! Ported from Fleeced's `online/room.rs`.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use super::relays::{Finder, RelayStatus, Side};
use super::{
    room_name, Ask, Msg, Peer, Session, DESYNC, FULL, GARBLED, LEFT_ROOM, PLAYING, RELOAD, SEATS, STARTED,
    WAITING,
};
use crate::game::flow::Phase;
use crate::lobby::{clean_name, Difficulty, Lobby, MatchStart, Who, MAX_SEATS};
use crate::play::online::{pack, NetMatch};
use crate::play::Input;
use crate::rng::Rng;
use crate::ticks::Clock;

/// Letters in a room code.
pub const CODE_LEN: usize = 6;
/// What a room code is made of: no 0, O, 1 or I, so a code read aloud
/// survives.
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// The countdown before a match, in milliseconds. `?fast` skips it.
pub const COUNTDOWN_MS: i64 = 3_000;

/// Status keys this module adds to the session's. Each has a string in `data/`.
pub const FINDING: &str = "online.finding";
const NO_RELAY: &str = "online.no_relay";
/// The status of a joiner holding a reply code the host has yet to see.
pub const SEND_REPLY: &str = "online.send_reply";
/// The transport's error keys; anything else reads as a failed connection.
const ERRORS: &[&str] = &[
    NO_RELAY,
    "online.no_bundle",
    "online.no_path",
    "online.signal_failed",
    "online.wrong_code",
    "online.bad_paste",
];
const SIGNAL_FAILED: &str = "online.signal_failed";

/// A room code from a seed.
pub fn make_code(seed: u64) -> String {
    let mut rng = Rng::new(seed);
    (0..CODE_LEN)
        .map(|_| CODE_LETTERS[rng.below(CODE_LETTERS.len() as u32) as usize] as char)
        .collect()
}

/// A typed or linked code as the room knows it, or None. Case and spaces at
/// the ends do not matter.
pub fn clean_code(text: &str) -> Option<String> {
    let code = text.trim().to_ascii_uppercase();
    let fits = code.len() == CODE_LEN && code.bytes().all(|b| CODE_LETTERS.contains(&b));
    fits.then_some(code)
}

/// What the page asks of the transport (`web/online.js` `request`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Try every pinned relay at once: open it, send `req`, and count it
    /// live if EOSE comes back within `timeout_ms`. The answer is a `relays`
    /// event whose text is `{"id", "live": [urls]}`.
    ProbeRelays {
        id: u32,
        timeout_ms: i64,
        req: String,
    },
    /// Open the room on these relays, leaving any room already open.
    HostRoom {
        room: String,
        relays: Vec<String>,
    },
    JoinRoom {
        room: String,
        relays: Vec<String>,
    },
    /// Host by pasted code: the answer is a `code` event holding the invite.
    HostByCode,
    /// Host: a fresh invite for the next joiner, as a `code` event.
    InviteByCode,
    /// Join by the host's invite: the answer is a `code` event holding the
    /// reply.
    JoinByCode {
        invite: String,
    },
    /// Host: the joiner's reply to the current invite.
    AcceptReply {
        reply: String,
    },
    Leave,
}

/// Text for one browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Outgoing {
    pub to: String,
    pub text: String,
}

/// One frame's work for the page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Pump {
    pub out: Vec<Outgoing>,
    pub requests: Vec<Request>,
}

/// A `relays` event's text.
#[derive(Deserialize)]
struct Probed {
    id: u32,
    live: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Solo,
    Host,
    Join,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Lobby,
    /// A match is built and counting down; `ends` is set by the next pump.
    Counting {
        ends: Option<i64>,
    },
    Playing,
}

/// A room met by pasted codes.
#[derive(Debug, Clone, Default)]
struct Pasted {
    /// The code to send: the host's invite, or the joiner's reply. Empty
    /// while the browser is still making it.
    code: String,
    /// What the transport has yet to hear.
    requests: Vec<Request>,
}

/// How a room is opened: the page's facts, handed in by `app.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// this browser's id, stable for its tab
    pub client: String,
    pub name: String,
    pub build: String,
    pub app_id: String,
    pub seed: u64,
    pub fast: bool,
    /// `?net=off`: the room opens with no transport at all, for checks
    pub no_net: bool,
}

/// One browser's room.
#[derive(Debug, Clone)]
pub struct Room {
    pub kind: Kind,
    pub code: String,
    pub client: String,
    pub stage: Stage,
    pub session: Session<NetMatch>,
    pub lobby: Lobby,
    /// A joiner has seen the host's lobby. A host and a solo room always have.
    pub heard: bool,
    pub start: Option<MatchStart>,
    /// The match, from `begin` on; none in a solo room.
    pub game: Option<NetMatch>,
    /// This browser's inputs, packed, waiting for a tick each.
    queued: VecDeque<u64>,
    /// The session tick whose input went out last.
    sent_for: Option<u32>,
    name: String,
    finder: Option<Finder>,
    pasted: Option<Pasted>,
    /// The transport's ids and the core's numbers for them.
    numbers: BTreeMap<String, Peer>,
    ids: BTreeMap<Peer, String>,
    /// Host: each joiner's client id.
    clients: BTreeMap<Peer, String>,
    /// Joiner: the host, and whether this browser has introduced itself.
    host_peer: Option<Peer>,
    joined: bool,
    /// Host: the lobby the joiners last saw.
    told: Option<Lobby>,
    rng: Rng,
    clock: Clock,
    fast: bool,
    /// A transport error, as a key.
    error: Option<&'static str>,
}

impl Room {
    fn new(kind: Kind, code: String, how: &Opening, finder: Option<Finder>) -> Room {
        let host = kind != Kind::Join;
        let lobby = if host {
            Lobby::new(&how.client, &how.name)
        } else {
            Lobby::new("", "")
        };
        Room {
            kind,
            code,
            client: how.client.clone(),
            stage: Stage::Lobby,
            session: Session::new(host, &how.build),
            lobby,
            heard: host,
            start: None,
            game: None,
            queued: VecDeque::new(),
            sent_for: None,
            name: how.name.clone(),
            finder,
            pasted: None,
            numbers: BTreeMap::new(),
            ids: BTreeMap::new(),
            clients: BTreeMap::new(),
            host_peer: None,
            joined: false,
            told: None,
            rng: Rng::new(how.seed),
            clock: Clock::default(),
            fast: how.fast,
            error: None,
        }
    }

    /// A match against bots: the lobby's open seats past the host's are bots.
    pub fn solo(how: &Opening) -> Room {
        Room::new(Kind::Solo, String::new(), how, None)
    }

    pub fn host(how: &Opening, now_ms: i64) -> Room {
        let code = make_code(how.seed);
        let finder = (!how.no_net).then(|| Finder::new(Side::Host, &room_name(&how.app_id, &code), now_ms));
        Room::new(Kind::Host, code, how, finder)
    }

    /// `code` is already clean (`clean_code`).
    pub fn join(how: &Opening, code: &str, now_ms: i64) -> Room {
        let finder = (!how.no_net).then(|| Finder::new(Side::Join, &room_name(&how.app_id, code), now_ms));
        Room::new(Kind::Join, code.to_string(), how, finder)
    }

    fn by_pasted_code(kind: Kind, how: &Opening, first: Request) -> Room {
        let mut room = Room::new(kind, String::new(), how, None);
        room.pasted = Some(Pasted {
            code: String::new(),
            requests: if how.no_net { Vec::new() } else { vec![first] },
        });
        room
    }

    /// Hosts with an invite code, on no relay.
    pub fn host_by_code(how: &Opening) -> Room {
        Room::by_pasted_code(Kind::Host, how, Request::HostByCode)
    }

    /// Joins by the host's invite code, on no relay.
    pub fn join_by_code(how: &Opening, invite: &str) -> Room {
        let invite = invite.to_string();
        Room::by_pasted_code(Kind::Join, how, Request::JoinByCode { invite })
    }

    /// The code this browser has to send, if the room met by pasted codes:
    /// empty while it is being made.
    pub fn pasted_code(&self) -> Option<&str> {
        self.pasted.as_ref().map(|p| p.code.as_str())
    }

    /// A pasted code is on its way from the transport, and asking again
    /// would talk over it.
    pub fn making_code(&self) -> bool {
        self.error.is_none() && self.pasted_code() == Some("")
    }

    /// Host: the reply code pasted in. A wrong one is the transport's to
    /// refuse, and the invite stands for another try.
    pub fn accept_reply(&mut self, reply: &str) {
        self.error = None;
        if let Some(p) = &mut self.pasted {
            p.requests.push(Request::AcceptReply {
                reply: reply.to_string(),
            });
        }
    }

    pub fn is_host(&self) -> bool {
        self.kind != Kind::Join
    }

    /// The seconds left on the countdown, for the screen.
    pub fn counting(&self, now_ms: i64) -> Option<u32> {
        match self.stage {
            Stage::Counting { ends } => {
                let left = ends.map_or(COUNTDOWN_MS, |e| (e - now_ms).max(0));
                Some(((left + 999) / 1_000) as u32)
            }
            _ => None,
        }
    }

    /// What to say about the room now, as a key, or nothing.
    pub fn status(&self) -> &'static str {
        if let Some(key) = self.error {
            return key;
        }
        if let Some(RelayStatus::Failed { error }) = self.finder.as_ref().map(Finder::status) {
            return ERRORS
                .iter()
                .copied()
                .chain(["online.timeout"])
                .find(|k| *k == error)
                .unwrap_or(SIGNAL_FAILED);
        }
        let reply_out = self.pasted_code().is_some_and(|c| !c.is_empty());
        match (self.session.status, self.kind) {
            (WAITING, Kind::Join) if self.pasted.is_some() => {
                if reply_out {
                    SEND_REPLY
                } else {
                    ""
                }
            }
            (PLAYING, _) | (WAITING, Kind::Solo) => "",
            (WAITING, Kind::Join) => FINDING,
            (other, _) => other,
        }
    }

    /// The transport's number for a peer, made on first sight.
    fn number(&mut self, id: &str) -> Peer {
        if let Some(n) = self.numbers.get(id) {
            return *n;
        }
        let n = self.numbers.len() as Peer + 1;
        self.numbers.insert(id.to_string(), n);
        self.ids.insert(n, id.to_string());
        n
    }

    /// A transport event. A key in `text` on `error`, and `{"id", "live"}`
    /// on `relays`; a bad one is ignored.
    pub fn event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now_ms: i64) {
        match kind {
            "relays" => {
                if let (Ok(p), Some(f)) = (serde_json::from_str::<Probed>(text), &mut self.finder) {
                    f.probed(p.id, p.live, now_ms);
                }
            }
            "code" => {
                if let Some(p) = &mut self.pasted {
                    p.code = text.to_string();
                }
            }
            "connected" => self.connected(peer, host),
            "message" => {
                if let Some(n) = self.numbers.get(peer).copied() {
                    self.session.message(n, text);
                    self.after_message();
                }
            }
            "disconnected" => self.disconnected(peer),
            "error" if text == NO_RELAY => {
                if let Some(f) = &mut self.finder {
                    f.no_relay(now_ms);
                }
            }
            "error" => {
                let key = ERRORS.iter().copied().find(|k| *k == text);
                self.error = Some(key.unwrap_or(SIGNAL_FAILED));
            }
            _ => {}
        }
    }

    fn connected(&mut self, id: &str, host: bool) {
        if let Some(f) = &mut self.finder {
            f.connected(host);
        }
        // Trystero makes a full mesh, and a joiner talks only to the host
        if self.kind == Kind::Join && !host {
            return;
        }
        let n = self.number(id);
        if self.kind == Kind::Join {
            self.host_peer = Some(n);
        }
        self.session.connected(n);
        // an invite works once: the next joiner needs a fresh one
        if let (Kind::Host, Some(p)) = (self.kind, &mut self.pasted) {
            p.code.clear();
            p.requests.push(Request::InviteByCode);
        }
    }

    fn disconnected(&mut self, id: &str) {
        let Some(n) = self.numbers.get(id).copied() else {
            return;
        };
        self.session.left(n);
        if let Some(client) = self.clients.remove(&n) {
            if self.stage == Stage::Lobby {
                self.lobby.remove_person(&client);
            }
        }
    }

    /// What a message left behind: the joiner introduces itself once the
    /// host's hello is in, and lobby messages are for this module.
    fn after_message(&mut self) {
        if let (false, Some(host)) = (self.joined, self.host_peer) {
            if self.session.peers.contains_key(&host) {
                self.joined = true;
                let join = Msg::Join {
                    id: self.client.clone(),
                    name: self.name.clone(),
                };
                self.session.send(host, &join);
            }
        }
        for (from, msg) in self.session.take_inbox() {
            self.heard_from(from, msg);
        }
    }

    fn heard_from(&mut self, from: Peer, msg: Msg) {
        match (self.kind, msg) {
            (Kind::Host, Msg::Join { id, name }) => {
                let taken = id == self.client || self.clients.iter().any(|(p, c)| *c == id && *p != from);
                if id.is_empty() || taken {
                    self.session.status = GARBLED;
                } else if self.lobby.add_person(&id, &name).is_some() {
                    self.clients.insert(from, id);
                    self.told = None;
                } else {
                    self.session.send(
                        from,
                        &Msg::Bye {
                            why: FULL.to_string(),
                        },
                    );
                }
            }
            (Kind::Host, Msg::Ask { ask }) => {
                let Some(id) = self.clients.get(&from).cloned() else {
                    self.session.status = GARBLED;
                    return;
                };
                match ask {
                    Ask::Name { name } => self.lobby.set_name(&id, &name),
                    Ask::Ready { ready } => self.lobby.set_ready(&id, ready),
                }
            }
            (Kind::Join, Msg::Roster { lobby }) if lobby.is_whole() => {
                self.lobby = lobby;
                self.heard = true;
            }
            (Kind::Join, Msg::Start { start }) if self.stage == Stage::Lobby || self.over() => {
                self.begin(start)
            }
            _ => self.session.status = GARBLED,
        }
    }

    /// Builds the match every browser builds from `start`.
    pub(crate) fn begin(&mut self, start: MatchStart) {
        if start.seats.len() > SEATS || !start.options.valid() {
            self.session.status = GARBLED;
            return;
        }
        let host_id = self.lobby.host.clone();
        let mut active = [false; SEATS];
        let mut mine = None;
        let mut map: Vec<(Peer, usize)> = Vec::new();
        for s in start.seats.iter().filter(|s| !s.person.is_empty()) {
            let seat = s.seat as usize;
            if seat >= SEATS {
                self.session.status = GARBLED;
                return;
            }
            active[seat] = true;
            if s.person == self.client {
                mine = Some(seat);
            }
            for (peer, client) in &self.clients {
                if *client == s.person {
                    map.push((*peer, seat));
                }
            }
            if let (Some(host), true) = (self.host_peer, s.person == host_id) {
                map.push((host, seat));
            }
        }
        let Some(seat) = mine else {
            self.session.status = STARTED;
            return;
        };
        self.game = None;
        if self.kind != Kind::Solo {
            match NetMatch::new(&start) {
                Ok(game) => self.game = Some(game),
                Err(_) => {
                    self.session.status = GARBLED;
                    return;
                }
            }
        }
        self.session.begin(seat, active, &map);
        self.queued.clear();
        self.sent_for = None;
        self.clock = Clock::default();
        self.start = Some(start);
        self.stage = Stage::Counting { ends: None };
    }

    /// The match here has ended.
    pub fn over(&self) -> bool {
        self.out_of_sync()
            || self
                .game
                .as_ref()
                .is_some_and(|g| g.play.rules.phase == Phase::Results)
    }

    /// The browsers' matches disagree: this one stops, and says so.
    pub fn out_of_sync(&self) -> bool {
        self.session.status == DESYNC
    }

    /// One of this browser's inputs, for the session to send on a tick.
    /// An input that cannot be packed is dropped: the match would refuse it.
    /// When the session's last tick began, microseconds (R1.2103).
    pub fn last_tick_us(&self) -> Option<i64> {
        self.clock.last_tick_us()
    }

    pub fn queue(&mut self, input: Input) {
        if let Some(k) = pack(input) {
            self.queued.push_back(k);
        }
    }

    /// From the winner's screen back to the lobby this match came from. A
    /// match under way isn't left this way.
    pub fn back_to_lobby(&mut self) {
        if self.over() {
            self.game = None;
            self.start = None;
            self.stage = Stage::Lobby;
        }
    }

    /// The host presses Start. A lobby that cannot start says so in its
    /// panel, so a press that slips through changes nothing.
    fn press_start(&mut self) {
        let seed = self.rng.next_u64();
        let can_start = self.is_host() && self.stage == Stage::Lobby;
        if let (true, Ok(start)) = (can_start, self.lobby.start(seed)) {
            self.tell_joiners();
            self.begin(start.clone());
            self.session.send_all(&Msg::Start { start });
        }
    }

    /// Host: every joiner sees the lobby as it now stands.
    fn tell_joiners(&mut self) {
        if self.kind == Kind::Host && self.told.as_ref() != Some(&self.lobby) {
            self.told = Some(self.lobby.clone());
            let lobby = self.lobby.clone();
            self.session.send_all(&Msg::Roster { lobby });
        }
    }

    fn ask(&mut self, ask: Ask) {
        if let Some(host) = self.host_peer {
            self.session.send(host, &Msg::Ask { ask });
        }
    }

    /// Whether this browser's seat has pressed Ready.
    pub fn am_ready(&self) -> bool {
        self.lobby
            .seat_of(&self.client)
            .is_some_and(|n| matches!(&self.lobby.seats[n], Who::Person { ready: true, .. }))
    }

    fn host_only(&self, name: &str) -> Result<(), String> {
        if self.is_host() {
            Ok(())
        } else {
            Err(format!("Only the host can use \"{name}\"."))
        }
    }

    /// A lobby action. The seat action is `seat.<n>.level`.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        let number = || {
            value
                .parse::<u8>()
                .map_err(|_| format!("\"{value}\" is not a number."))
        };
        match name {
            "lobby.name" => {
                self.name = clean_name(value);
                if self.is_host() {
                    let client = self.client.clone();
                    self.lobby.set_name(&client, value);
                } else {
                    self.ask(Ask::Name {
                        name: value.to_string(),
                    });
                }
            }
            // the Ready button sends nothing and flips it; "on" and "off" set it
            "lobby.ready" => {
                let ready = match value {
                    "on" => true,
                    "off" => false,
                    _ => !self.am_ready(),
                };
                if self.is_host() {
                    let client = self.client.clone();
                    self.lobby.set_ready(&client, ready);
                } else {
                    self.ask(Ask::Ready { ready });
                }
            }
            "lobby.start" => self.press_start(),
            "play.lobby" => self.back_to_lobby(),
            "lobby.players" => {
                self.host_only(name)?;
                self.lobby.set_players(number()? as u32);
            }
            "lobby.wind" => {
                self.host_only(name)?;
                self.lobby.set_wind(value == "on");
            }
            "lobby.flat" => {
                self.host_only(name)?;
                self.lobby.set_flat(value == "on");
            }
            other => return self.seat_act(other, value),
        }
        Ok(())
    }

    fn seat_act(&mut self, name: &str, value: &str) -> Result<(), String> {
        let refused = || format!("Unknown action \"{name}\".");
        let mut parts = name.split('.');
        let (Some("seat"), Some(seat), Some(what), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(refused());
        };
        let seat: usize = seat.parse().map_err(|_| refused())?;
        if seat >= MAX_SEATS {
            return Err(refused());
        }
        match what {
            "level" => {
                self.host_only(name)?;
                let level =
                    Difficulty::parse(value).ok_or_else(|| format!("\"{value}\" is not a difficulty."))?;
                self.lobby.set_level(seat, level);
            }
            _ => return Err(refused()),
        }
        Ok(())
    }

    /// One display frame: the finder's timeouts, the countdown, the ticks,
    /// and what to send.
    pub fn pump(&mut self, now_ms: i64) -> Pump {
        let mut requests = Vec::new();
        if let Some(f) = &mut self.finder {
            f.update(now_ms);
            requests = f.take_requests();
        }
        if let Some(p) = &mut self.pasted {
            requests.append(&mut p.requests);
        }
        self.tell_joiners();
        if let Stage::Counting { ends } = self.stage {
            let ends = ends.unwrap_or(now_ms + if self.fast { 0 } else { COUNTDOWN_MS });
            self.stage = if now_ms >= ends {
                Stage::Playing
            } else {
                Stage::Counting { ends: Some(ends) }
            };
        }
        let over = self.over();
        if let (Stage::Playing, Some(game), false) = (self.stage, &mut self.game, over) {
            for _ in self.clock.frame(now_ms * 1_000, 0) {
                // one input a tick; a tick still waiting has its input already
                if self.sent_for != Some(self.session.tick) {
                    self.sent_for = Some(self.session.tick);
                    self.session.local(self.queued.pop_front().unwrap_or(0));
                }
                self.session.step(game);
            }
        }
        let mut out = Vec::new();
        for (peer, text) in self.session.take_outbox() {
            if let Some(to) = self.ids.get(&peer) {
                out.push(Outgoing { to: to.clone(), text });
            }
        }
        Pump { out, requests }
    }

    /// Whether this browser has lost the room: the host left, this browser
    /// was turned away, or the search failed.
    pub fn lost(&self) -> bool {
        let gone = [LEFT_ROOM, FULL, STARTED, RELOAD].contains(&self.session.status);
        let failed = matches!(
            self.finder.as_ref().map(Finder::status),
            Some(RelayStatus::Failed { .. })
        );
        gone || failed
    }
}

#[cfg(test)]
mod tests;
