//! Online play, core side: what crosses the wire, and the bookkeeping each
//! browser keeps. Part of the online kit (`kits/online/README.md`).
//!
//! The transport in `web/net/rtc.js` connects browsers and carries text. It
//! hands this module three things: a peer arrived, a peer left, and a message
//! from a peer. This module answers by leaving text in an outbox, which the
//! transport sends. It never sees a connection, so `cargo test` can play a
//! whole match between simulated browsers, with lag, in a few milliseconds.
//!
//! Rollback over up to eight seats (D23). Each browser runs the same world
//! from the same start, bots included; only inputs cross the wire.
//! A local input is used `DELAY` ticks after it is made and sent at once. A
//! browser does not wait for the other seats: it guesses that each holds its
//! last known input, keeps the world from before each tick it ran on a
//! guess, and when a real input arrives and differs, restores that world and
//! runs the ticks again. It never runs more than `MAX_AHEAD` ticks past the
//! last tick for which it holds every seat's real input. Every `SUM_EVERY`
//! confirmed ticks each browser sends a checksum of its world, and a mismatch
//! is reported.
//!
//! **Inputs go through the host.** The kit's example is two browsers, so each
//! hears the other directly. With eight, two joiners can fail to reach each
//! other while both reach the host (Ape Race's T71: STUN only, strict NATs).
//! So a joiner sends its input to the host alone and the host sends every
//! seat's inputs on, batched once a frame. That makes the host's stream the
//! one truth: when a joiner leaves mid-match, the host says from which tick
//! its seat holds no input, and no browser can have heard more of it than
//! the host did (`Msg::Gone`). The price is a joiner-to-joiner path of two
//! legs, which `MAX_AHEAD` covers.
//!
//! A session runs any `Rollback` world: `step`, `sum` and `Clone` are all this
//! module asks of one. The match is `play::online::NetMatch`; the kit's toy `World`
//! stays as the small world the session's own tests play.
//!
//! Every status is a key for a string in `data/` (`online.reload`,
//! `online.garbled`, ...), so the wording sits with the other copy and the
//! rule that chose it sits here.
//!
//! Required tests, each implemented below:
//!
//! - every message survives encoding and decoding;
//! - a browser on another protocol or another build is turned away, and both
//!   ends say why;
//! - garbage on the wire changes the status and nothing else, and a message
//!   from a peer that never said hello is ignored;
//! - a browser runs on a guess, never more than `MAX_AHEAD` ticks past its
//!   confirmed ticks;
//! - a late input that differs from the guess rolls the world back and
//!   replays it to the world of a copy fed every input on time;
//! - copies joined by a lagged wire end on the same world as a copy fed
//!   every input on time;
//! - a peer that leaves is reported, and a browser too many is told the room
//!   is full;
//! - two worlds that disagree are reported as a desync.

pub mod relays;
pub mod room;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::lobby::{Lobby, MatchStart, MAX_SEATS};

/// Keeps this game's rooms apart from every other app on the same public
/// relays. `web/net/config.js` says the same to Trystero.
pub const APP_ID: &str = "royal-rubble-p2p";

/// Bumped whenever a message changes shape or a rule changes how a tick
/// runs. The transport puts it in the room name (`room_name`), so a stale
/// tab and a fresh one still meet, and the hello turns the stale one away
/// with a reason. A room name holding the build hash would keep them apart,
/// and both would wait for nobody.
pub const PROTOCOL: u32 = 1;

/// Ticks between a local input and the tick that uses it.
pub const DELAY: u32 = 2;

/// Ticks a browser may run on guessed input past its last confirmed tick.
/// 0 is lockstep. 12 ticks is 200 ms, and the contract's budget keeps a
/// 12-tick replay under 5 ms.
pub const MAX_AHEAD: u32 = 12;

/// How far past its own tick a browser can hear an input through the host:
/// a seat's input runs `DELAY` past that seat's tick, which runs `MAX_AHEAD`
/// past what it confirmed, which needs the host's inputs (`DELAY` past the
/// host's tick, plus one); the host in turn runs `MAX_AHEAD` past what it
/// confirmed, which needs this browser's inputs (`DELAY` past this tick,
/// plus one).
pub const WINDOW: u32 = 3 * DELAY + 2 * MAX_AHEAD + 2;

/// Ticks between checksums: one a second. The match's checksum hashes debug
/// text (R1.704), so it stays off the per-tick path.
pub const SUM_EVERY: u32 = 60;

/// How many of its own checksums a browser keeps for the others' to meet.
const SUMS_KEPT: usize = 8;

/// The seats of a match.
pub const SEATS: usize = MAX_SEATS;

/// Input bits for the toy world in the session's tests.
pub const LEFT: u64 = 1;
pub const RIGHT: u64 = 2;

/// Status keys. Each has a string in `data/` under the same key.
pub const WAITING: &str = "online.waiting";
pub const PLAYING: &str = "online.playing";
pub const RELOAD: &str = "online.reload";
pub const GARBLED: &str = "online.garbled";
pub const LEFT_ROOM: &str = "online.left";
pub const FULL: &str = "online.full";
pub const STARTED: &str = "online.started";
pub const DESYNC: &str = "online.desync";

/// The reasons a `Bye` may carry. Anything else on the wire reads as `LEFT_ROOM`.
const REASONS: &[&str] = &[RELOAD, FULL, STARTED];

/// The core's number for another browser. It stays the same while
/// connections come and go under it (`room::Room` maps the transport's ids).
pub type Peer = u32;

/// What a joiner asks of the host. The host's roster shows the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "a", rename_all = "lowercase")]
pub enum Ask {
    Name { name: String },
    Ready { ready: bool },
}

/// Everything that crosses the wire. JSON text, tagged by `t`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Msg {
    /// Keep this shape fixed, so an old copy can always read it.
    Hello {
        protocol: u32,
        build: String,
    },
    /// A joiner's input, for its own seat, to the host.
    Input {
        tick: u32,
        keys: u64,
    },
    /// The host's batch to a joiner, as `[seat, tick, keys]` rows.
    Inputs {
        rows: Vec<[u64; 3]>,
    },
    /// From tick `from`, `seat` holds no input: its browser left.
    Gone {
        seat: u32,
        from: u32,
    },
    Sum {
        tick: u32,
        sum: u64,
    },
    Bye {
        why: String,
    },
    /// A joiner's client id and name, sent to the host once it says hello.
    Join {
        id: String,
        name: String,
    },
    Ask {
        ask: Ask,
    },
    /// The host's lobby.
    Roster {
        lobby: Lobby,
    },
    Start {
        start: MatchStart,
    },
}

pub fn encode(m: &Msg) -> String {
    serde_json::to_string(m).expect("a Msg always serializes")
}

pub fn decode(text: &str) -> Option<Msg> {
    serde_json::from_str(text).ok()
}

/// The room name the transport joins: the app id, the protocol and the code
/// the players typed. No build hash.
pub fn room_name(app_id: &str, code: &str) -> String {
    format!("{app_id}-p{PROTOCOL}-{code}")
}

/// What a session runs. Each seat's input for a tick is one number. `step` and
/// `sum` stay deterministic, and a clone is cheap: a session keeps one per
/// unconfirmed tick.
pub trait Rollback: Clone {
    /// The input of a seat with no person in it, from the first tick for a
    /// bot and from the agreed tick for a browser that left. It can't be
    /// anyone's real input.
    const NOBODY: u64;
    /// The guess for a seat whose input for a tick has not come in, from
    /// its last known one. Held keys hold (the default); a match whose
    /// inputs are one-off presses guesses no input.
    fn guess(last: u64) -> u64 {
        last
    }
    fn step(&mut self, keys: [u64; SEATS]);
    fn sum(&self) -> u64;
}

/// The kit's toy world: one marker a seat on a line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct World {
    pub pos: [i32; SEATS],
    pub met: u32,
}

impl Rollback for World {
    const NOBODY: u64 = 0;

    fn step(&mut self, keys: [u64; SEATS]) {
        for (seat, k) in keys.iter().enumerate() {
            if k & LEFT != 0 {
                self.pos[seat] -= 1;
            }
            if k & RIGHT != 0 {
                self.pos[seat] += 1;
            }
        }
        if self.pos[0] == self.pos[1] {
            self.met += 1;
        }
    }

    /// FNV-1a over the state, in a fixed order.
    fn sum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        for p in self.pos {
            mix(p as u32 as u64);
        }
        mix(self.met as u64);
        h
    }
}

/// One browser's side of a room: who it has met, and its rollback state.
#[derive(Debug, Clone)]
pub struct Session<W = World> {
    /// This browser hosts: it hears every joiner and sends their inputs on.
    pub host: bool,
    /// This browser's seat in the match.
    pub seat: usize,
    pub status: &'static str,
    build: String,
    /// The browsers whose hello was accepted, each with its seat once a
    /// match has begun. A joiner holds one: the host.
    pub peers: BTreeMap<Peer, Option<usize>>,
    /// Browsers that have connected and not yet said hello.
    greeting: BTreeSet<Peer>,
    /// A match has begun.
    pub begun: bool,
    /// The seats a person holds; a bot's or an empty seat has no input.
    active: [bool; SEATS],
    /// A seat whose browser left, and the tick it holds no input from.
    gone: [Option<u32>; SEATS],
    /// Each seat's real inputs by tick, from the newest confirmed one on.
    inputs: [BTreeMap<u32, u64>; SEATS],
    /// The next tick this browser will run.
    pub tick: u32,
    /// Every tick below this ran on every seat's real input.
    pub confirmed: u32,
    /// The inputs each unconfirmed tick ran on, guesses included.
    used: BTreeMap<u32, [u64; SEATS]>,
    /// The world before each unconfirmed tick.
    snapshots: BTreeMap<u32, W>,
    /// The earliest tick whose guess turned out wrong, until it is replayed.
    replay_from: Option<u32>,
    /// How many times this browser has rolled back.
    pub rollbacks: u32,
    /// This browser's newest checksums, for the others' to meet.
    sums: BTreeMap<u32, u64>,
    /// Other browsers' checksums, until this browser confirms the same tick.
    theirs: BTreeMap<u32, Vec<u64>>,
    /// Host: `[seat, tick, keys]` rows to send on, batched once a frame.
    relay: Vec<[u64; 3]>,
    /// Lobby messages for `room::Room`, which owns the lobby.
    inbox: VecDeque<(Peer, Msg)>,
    outbox: VecDeque<(Peer, String)>,
}

impl<W: Rollback> Session<W> {
    pub fn new(host: bool, build: &str) -> Session<W> {
        Session {
            host,
            seat: 0,
            status: WAITING,
            build: build.to_string(),
            peers: BTreeMap::new(),
            greeting: BTreeSet::new(),
            begun: false,
            active: [false; SEATS],
            gone: [None; SEATS],
            inputs: Default::default(),
            tick: 0,
            confirmed: 0,
            used: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            replay_from: None,
            rollbacks: 0,
            sums: BTreeMap::new(),
            theirs: BTreeMap::new(),
            relay: Vec::new(),
            inbox: VecDeque::new(),
            outbox: VecDeque::new(),
        }
    }

    /// A match begins: this browser plays `seat`, `active` says which seats
    /// a person holds, and `seats` gives each known peer's seat.
    pub fn begin(&mut self, seat: usize, active: [bool; SEATS], seats: &[(Peer, usize)]) {
        self.begun = true;
        self.seat = seat;
        self.active = active;
        self.gone = [None; SEATS];
        for (peer, s) in seats {
            if let Some(known) = self.peers.get_mut(peer) {
                *known = Some(*s);
            }
        }
        self.inputs = Default::default();
        for seat_inputs in self.inputs.iter_mut() {
            for t in 0..DELAY {
                seat_inputs.insert(t, 0);
            }
        }
        self.tick = 0;
        self.confirmed = 0;
        self.used.clear();
        self.snapshots.clear();
        self.replay_from = None;
        self.rollbacks = 0;
        self.sums.clear();
        self.theirs.clear();
        self.relay.clear();
    }

    /// Queues a message for one browser.
    pub fn send(&mut self, to: Peer, m: &Msg) {
        self.outbox.push_back((to, encode(m)));
    }

    /// Queues a message for every browser this one has met.
    pub fn send_all(&mut self, m: &Msg) {
        let peers: Vec<Peer> = self.peers.keys().copied().collect();
        for peer in peers {
            self.send(peer, m);
        }
    }

    fn the_host(&self) -> Option<Peer> {
        if self.host {
            None
        } else {
            self.peers.keys().next().copied()
        }
    }

    /// Host: each joiner gets the batch, less the rows of its own seat.
    fn flush_relay(&mut self) {
        if self.relay.is_empty() {
            return;
        }
        let rows = std::mem::take(&mut self.relay);
        let peers: Vec<(Peer, Option<usize>)> = self.peers.iter().map(|(p, s)| (*p, *s)).collect();
        for (peer, seat) in peers {
            let theirs: Vec<[u64; 3]> = rows
                .iter()
                .filter(|r| Some(r[0] as usize) != seat)
                .copied()
                .collect();
            if !theirs.is_empty() {
                self.send(peer, &Msg::Inputs { rows: theirs });
            }
        }
    }

    /// This browser's newest checksum, with its tick. Two browsers showing
    /// the same tick show the same world.
    pub fn newest_sum(&self) -> Option<(u32, u64)> {
        self.sums.last_key_value().map(|(tick, sum)| (*tick, *sum))
    }

    /// Everything to send, in order.
    pub fn take_outbox(&mut self) -> Vec<(Peer, String)> {
        self.flush_relay();
        self.outbox.drain(..).collect()
    }

    /// The lobby messages that have come in, in order.
    pub fn take_inbox(&mut self) -> Vec<(Peer, Msg)> {
        self.inbox.drain(..).collect()
    }

    /// The transport says a browser arrived.
    pub fn connected(&mut self, peer: Peer) {
        let room = self.peers.len() + self.greeting.len();
        if self.begun || room + 1 >= SEATS {
            let why = if self.begun { STARTED } else { FULL };
            self.send(peer, &Msg::Bye { why: why.to_string() });
            return;
        }
        self.greeting.insert(peer);
        let hello = Msg::Hello {
            protocol: PROTOCOL,
            build: self.build.clone(),
        };
        self.send(peer, &hello);
    }

    /// The transport says a browser left. A joiner has lost the room; the
    /// host's room goes on, and a seat left mid-match holds no more input.
    pub fn left(&mut self, peer: Peer) {
        self.greeting.remove(&peer);
        let Some(seat) = self.peers.remove(&peer) else {
            return;
        };
        if !self.host {
            self.status = LEFT_ROOM;
        } else if let (true, Some(seat)) = (self.begun, seat) {
            let from = self.inputs[seat].keys().next_back().map_or(0, |t| t + 1);
            self.seat_gone(seat, from);
            self.send_all(&Msg::Gone {
                seat: seat as u32,
                from,
            });
        }
    }

    fn seat_gone(&mut self, seat: usize, from: u32) {
        self.gone[seat] = Some(from);
        self.inputs[seat].retain(|t, _| *t < from);
        let wrong = self
            .used
            .iter()
            .find(|(t, u)| **t >= from && u[seat] != W::NOBODY);
        if let Some((t, _)) = wrong {
            let t = *t;
            self.replay_from = Some(self.replay_from.map_or(t, |r| r.min(t)));
        }
    }

    /// Text from a browser. Anything that cannot be a message, or that no
    /// honest copy would send, changes the status and nothing else.
    pub fn message(&mut self, from: Peer, text: &str) {
        let known = self.peers.contains_key(&from) || self.greeting.contains(&from);
        if !known {
            return;
        }
        let Some(msg) = decode(text) else {
            self.status = GARBLED;
            return;
        };
        match msg {
            Msg::Hello { protocol, build } => {
                if protocol != PROTOCOL || build != self.build {
                    self.send(
                        from,
                        &Msg::Bye {
                            why: RELOAD.to_string(),
                        },
                    );
                    self.greeting.remove(&from);
                    self.peers.remove(&from);
                    self.status = RELOAD;
                } else {
                    self.greeting.remove(&from);
                    self.peers.insert(from, None);
                    self.status = PLAYING;
                }
            }
            // a bye may come before a hello: the room is full, or this
            // browser's build was turned away
            Msg::Bye { why } => {
                self.status = REASONS.iter().copied().find(|r| *r == why).unwrap_or(LEFT_ROOM);
                self.peers.remove(&from);
                self.greeting.remove(&from);
            }
            _ if !self.peers.contains_key(&from) => {
                // data before a hello: the transport holds it back, so a
                // copy that sends it anyway is not one of ours
                self.status = GARBLED;
            }
            Msg::Input { tick, keys } => {
                let seat = self.peers.get(&from).copied().flatten();
                match seat {
                    Some(seat) if self.host && self.begun => {
                        if self.take_input(seat, tick, keys) {
                            self.relay.push([seat as u64, tick as u64, keys]);
                        }
                    }
                    _ => self.status = GARBLED,
                }
            }
            Msg::Inputs { rows } => {
                let fits = |r: &[u64; 3]| {
                    (r[0] as usize) < SEATS && r[0] as usize != self.seat && r[1] <= u32::MAX as u64
                };
                if self.host || !self.begun || !rows.iter().all(fits) {
                    self.status = GARBLED;
                    return;
                }
                for [seat, tick, keys] in rows {
                    self.take_input(seat as usize, tick as u32, keys);
                }
            }
            Msg::Gone { seat, from: tick } => {
                if self.host || seat as usize >= SEATS {
                    self.status = GARBLED;
                } else {
                    self.seat_gone(seat as usize, tick);
                }
            }
            Msg::Sum { tick, sum } => {
                if let Some(mine) = self.sums.get(&tick) {
                    if *mine != sum {
                        self.status = DESYNC;
                    }
                } else if tick > self.confirmed {
                    self.theirs.entry(tick).or_default().push(sum);
                }
            }
            other => self.inbox.push_back((from, other)),
        }
    }

    /// One seat's real input, heard now. False, with the status set, when no
    /// honest copy could have sent it.
    fn take_input(&mut self, seat: usize, tick: u32, keys: u64) -> bool {
        let window = self.confirmed..=self.tick + WINDOW;
        if !window.contains(&tick) || self.inputs[seat].contains_key(&tick) {
            self.status = GARBLED;
            return false;
        }
        self.inputs[seat].insert(tick, keys);
        let guessed = self.used.get(&tick).map(|u| u[seat]);
        if guessed.is_some_and(|g| g != keys) {
            self.replay_from = Some(self.replay_from.map_or(tick, |r| r.min(tick)));
        }
        true
    }

    /// This browser's input, made now. It is used `DELAY` ticks ahead and
    /// sent once. Called every display frame; a frame that runs no tick
    /// adds nothing.
    pub fn local(&mut self, keys: u64) {
        if !self.begun {
            return;
        }
        let tick = self.tick + DELAY;
        if self.inputs[self.seat].contains_key(&tick) {
            return;
        }
        self.inputs[self.seat].insert(tick, keys);
        if self.host {
            self.relay.push([self.seat as u64, tick as u64, keys]);
        } else if let Some(host) = self.the_host() {
            self.send(host, &Msg::Input { tick, keys });
        }
    }

    /// Whether a seat's input for a tick is settled: heard, or none to hear.
    fn known(&self, seat: usize, tick: u32) -> bool {
        !self.active[seat]
            || self.gone[seat].is_some_and(|g| tick >= g)
            || self.inputs[seat].contains_key(&tick)
    }

    fn all_known(&self, tick: u32) -> bool {
        (0..SEATS).all(|seat| self.known(seat, tick))
    }

    /// A seat's input for a tick: the real one if it is here, otherwise the
    /// guess that the seat still holds its last known input. A seat with no
    /// person in it holds `NOBODY`.
    fn input_or_guess(&self, seat: usize, tick: u32) -> u64 {
        if !self.active[seat] || self.gone[seat].is_some_and(|g| tick >= g) {
            return W::NOBODY;
        }
        match self.inputs[seat].range(..=tick).next_back() {
            Some((t, k)) if *t == tick => *k,
            Some((_, k)) => W::guess(*k),
            None => 0,
        }
    }

    fn run(&mut self, world: &mut W, tick: u32) {
        let mut keys = [0; SEATS];
        for (seat, k) in keys.iter_mut().enumerate() {
            *k = self.input_or_guess(seat, tick);
        }
        self.snapshots.insert(tick, world.clone());
        self.used.insert(tick, keys);
        world.step(keys);
    }

    /// Replays from the earliest wrong guess, if a real input showed one.
    /// `step` calls it first; call it on its own to correct the world
    /// without running a new tick.
    pub fn settle(&mut self, world: &mut W) {
        if let Some(from) = self.replay_from.take() {
            *world = self.snapshots[&from].clone();
            for t in from..self.tick {
                self.run(world, t);
            }
            self.rollbacks += 1;
        }
        self.confirm(world);
    }

    /// Moves `confirmed` past every tick that ran on real inputs, sends a
    /// checksum at each `SUM_EVERY`, and drops what is no longer needed.
    fn confirm(&mut self, world: &W) {
        while self.confirmed < self.tick && self.all_known(self.confirmed) {
            let c = self.confirmed;
            self.confirmed += 1;
            let k = self.confirmed;
            if k.is_multiple_of(SUM_EVERY) {
                let sum = if k == self.tick {
                    world.sum()
                } else {
                    self.snapshots[&k].sum()
                };
                if !self.peers.is_empty() {
                    if self
                        .theirs
                        .remove(&k)
                        .is_some_and(|all| all.iter().any(|s| *s != sum))
                    {
                        self.status = DESYNC;
                    }
                    self.sums.insert(k, sum);
                    while self.sums.len() > SUMS_KEPT {
                        self.sums.pop_first();
                    }
                    self.send_sum(k, sum);
                }
            }
            self.snapshots.remove(&c);
            self.used.remove(&c);
            // keep the confirmed tick's input: it is the guess for the next
            for seat_inputs in self.inputs.iter_mut() {
                seat_inputs.retain(|t, _| *t >= c);
            }
        }
    }

    /// A joiner's checksum goes to the host, the host's to everyone: every
    /// browser is compared with the host.
    fn send_sum(&mut self, tick: u32, sum: u64) {
        match self.the_host() {
            Some(host) => self.send(host, &Msg::Sum { tick, sum }),
            None => self.send_all(&Msg::Sum { tick, sum }),
        }
    }

    /// Runs the next tick, on a guess for any input not yet here. Returns
    /// whether it ran: a browser `MAX_AHEAD` ticks past its confirmed ticks
    /// waits for the others.
    pub fn step(&mut self, world: &mut W) -> bool {
        self.settle(world);
        if self.tick >= self.confirmed + MAX_AHEAD && !self.all_known(self.tick) {
            return false;
        }
        let t = self.tick;
        self.run(world, t);
        self.tick += 1;
        self.confirm(world);
        true
    }
}

#[cfg(test)]
mod tests;
