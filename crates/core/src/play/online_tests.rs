//! The real match over the online session: two browsers on a lagged,
//! reordered wire, two people and two bots, against a copy fed every input
//! on time (Builder's FL-1 test, R1.1501).

use std::collections::BTreeMap;

use super::*;
use crate::lobby::{Difficulty as Level, Options, StartSeat};
use crate::online::{Peer, Session, DELAY, SUM_EVERY};

fn start(people: usize, bots: usize, seed: u64) -> MatchStart {
    let mut seats = Vec::new();
    for k in 0..people + bots {
        let person = k < people;
        seats.push(StartSeat {
            seat: k as u32,
            person: if person { format!("c{k}") } else { String::new() },
            name: String::new(),
            bot: (!person).then_some(Level::Normal),
        });
    }
    MatchStart {
        seed,
        seats,
        options: Options::default(),
    }
}

/// What person `seat` presses on the tick an input lands on: a fixed script
/// of real moves (a royal, a wall, swings, ready), so a true run can be
/// worked out alone. Refused moves are part of the script too.
fn script(seat: usize, tick: u32) -> u64 {
    if tick < DELAY {
        return 0;
    }
    let t = tick + seat as u32 * 7;
    let input = match t % 240 {
        20 => Input::PlaceRoyal {
            index: 0,
            at: GridPt::new(4 * 40, 4 * 40),
        },
        30 => Input::PlaceSegment {
            piece: Piece::Segment(Material::Wood),
            a: GridPt::new(4 * 38, 4 * 41),
            b: GridPt::new(4 * 40, 4 * 41),
        },
        40 => Input::SelectAmmo(0),
        50 => Input::Swing,
        50..=59 if t % 3 == seat as u32 % 3 => Input::Release,
        100 => Input::Pick(1),
        130 => Input::Air(0),
        200 => Input::Ready,
        _ => return 0,
    };
    pack(input).expect("every scripted input packs")
}

fn keys_for(people: usize, tick: u32) -> [u64; SEATS] {
    let mut keys = [0; SEATS];
    for (seat, k) in keys.iter_mut().enumerate().take(people) {
        *k = script(seat, tick);
    }
    keys
}

/// Two browsers: the host (seat 0) and a joiner (seat 1). Each message
/// takes 0..=5 frames by a fixed pattern, so the wire reorders them.
struct Pair {
    browsers: Vec<Session<NetMatch>>,
    games: Vec<NetMatch>,
    wire: Vec<(usize, Peer, usize, String)>,
    frame: usize,
    sent: usize,
    sums: Vec<BTreeMap<u32, u64>>,
}

const HOST_PEER: Peer = 100;

impl Pair {
    fn new(m: &MatchStart) -> Pair {
        let mut host = Session::new(true, "b");
        let mut join = Session::new(false, "b");
        host.connected(1);
        join.connected(HOST_PEER);
        for (_, text) in host.take_outbox() {
            join.message(HOST_PEER, &text);
        }
        for (_, text) in join.take_outbox() {
            host.message(1, &text);
        }
        let mut active = [false; SEATS];
        active[0] = true;
        active[1] = true;
        host.begin(0, active, &[(1, 1)]);
        join.begin(1, active, &[(HOST_PEER, 0)]);
        let game = NetMatch::new(m).expect("the match starts");
        Pair {
            browsers: vec![host, join],
            games: vec![game.clone(), game],
            wire: Vec::new(),
            frame: 0,
            sent: 0,
            sums: vec![BTreeMap::new(), BTreeMap::new()],
        }
    }

    fn post(&mut self, k: usize) {
        for (_, text) in self.browsers[k].take_outbox() {
            self.sent += 1;
            let lag = (self.sent * 7 + k * 3) % 6;
            let (from, to) = if k == 0 { (HOST_PEER, 1) } else { (1, 0) };
            self.wire.push((self.frame + lag, from, to, text));
        }
    }

    fn deliver(&mut self) {
        let frame = self.frame;
        let (now, later): (Vec<_>, Vec<_>) = self.wire.drain(..).partition(|m| m.0 <= frame);
        self.wire = later;
        for (_, from, to, text) in now {
            self.browsers[to].message(from, &text);
        }
    }

    fn note_sums(&mut self) {
        for k in 0..2 {
            if let Some((t, s)) = self.browsers[k].newest_sum() {
                self.sums[k].insert(t, s);
            }
        }
    }

    fn play(&mut self, ticks: u32) {
        while self.browsers.iter().any(|s| s.tick < ticks) && self.frame < 40_000 {
            for k in 0..2 {
                let keys = script(k, self.browsers[k].tick + DELAY);
                self.browsers[k].local(keys);
                self.post(k);
            }
            self.deliver();
            for k in 0..2 {
                if self.browsers[k].tick < ticks {
                    let (s, g) = (&mut self.browsers[k], &mut self.games[k]);
                    s.step(g);
                }
            }
            self.note_sums();
            self.frame += 1;
        }
        // the last inputs and checksums arrive
        for _ in 0..12 {
            for k in 0..2 {
                self.post(k);
            }
            self.frame += 1;
            self.deliver();
            for k in 0..2 {
                let (s, g) = (&mut self.browsers[k], &mut self.games[k]);
                s.settle(g);
            }
            self.note_sums();
        }
    }
}

fn truth(m: &MatchStart, people: usize, ticks: u32) -> NetMatch {
    let mut g = NetMatch::new(m).unwrap();
    for t in 0..ticks {
        g.step(keys_for(people, t));
    }
    g
}

#[test]
fn online_two_lagged_browsers_play_a_whole_match_and_agree_every_second() {
    let m = start(2, 2, 41);
    // the whole match: this script plays to the results in 21,290 ticks
    let ticks = 21_400;
    let mut pair = Pair::new(&m);
    pair.play(ticks);
    let want = truth(&m, 2, ticks);
    for k in 0..2 {
        let b = &pair.browsers[k];
        assert_eq!(b.tick, ticks, "browser {k} stalled");
        assert_eq!(b.confirmed, ticks, "browser {k} confirmed every tick");
        assert_eq!(b.status, crate::online::PLAYING, "browser {k}: no desync");
        assert_eq!(pair.games[k].sum(), want.sum(), "browser {k} ends on the true match");
    }
    assert!(pair.browsers[1].rollbacks > 0, "the wire made the joiner guess wrong");
    // a checksum every second, the same on both
    let seconds = (ticks / SUM_EVERY) as usize;
    assert!(pair.sums[0].len() >= seconds - 1, "sums {:?}", pair.sums[0].len());
    assert_eq!(pair.sums[0], pair.sums[1]);
    // the same results everywhere
    assert_eq!(want.play.rules.phase, Phase::Results);
    let standings = want.play.rules.standings();
    assert_eq!(standings.len(), 4);
    for k in 0..2 {
        assert_eq!(pair.games[k].play.rules.standings(), standings, "browser {k}");
    }
}

#[test]
fn online_inputs_pack_and_unpack_and_a_far_point_does_not_pack() {
    let all = [
        Input::Pick(2),
        Input::Reroll,
        Input::PlaceSegment {
            piece: Piece::Rope,
            a: GridPt::new(8191, 0),
            b: GridPt::new(3, 8000),
        },
        Input::PlaceSegment {
            piece: Piece::Segment(Material::Metal),
            a: GridPt::new(1, 2),
            b: GridPt::new(3, 4),
        },
        Input::PlaceBuilding {
            kind: BuildingKind::Armory,
            at: GridPt::new(500, 300),
        },
        Input::Sell(u32::MAX),
        Input::PlaceRoyal {
            index: 11,
            at: GridPt::new(160, 170),
        },
        Input::BuyXp,
        Input::BuyShot,
        Input::SelectAmmo(4),
        Input::Swing,
        Input::Release,
        Input::Air(3),
        Input::Ready,
    ];
    for i in all {
        let k = pack(i).unwrap();
        assert_ne!(k, 0);
        assert_ne!(k, NetMatch::NOBODY);
        assert_eq!(unpack(k), Some(i));
    }
    assert_eq!(unpack(0), None);
    let far = Input::PlaceRoyal {
        index: 0,
        at: GridPt::new(-1, 3),
    };
    assert_eq!(pack(far), None);
}

#[test]
fn online_a_person_who_leaves_is_played_by_a_bot_from_then_on() {
    let m = start(2, 1, 5);
    let mut g = NetMatch::new(&m).unwrap();
    g.step([0; SEATS]);
    assert_eq!(g.play.rules.players[1].seat, Seat::Human);
    let mut keys = [0; SEATS];
    keys[1] = NetMatch::NOBODY;
    g.step(keys);
    assert_eq!(g.play.rules.players[1].seat, Seat::Bot(Difficulty::Normal));
    assert_eq!(g.play.rules.players[0].seat, Seat::Human);
}

#[test]
fn online_the_replay_holds_a_persons_siege_inputs_at_their_ticks() {
    let m = start(2, 0, 9);
    let mut g = NetMatch::new(&m).unwrap();
    // both ready through the build
    let mut t = 0;
    while g.play.rules.phase != Phase::Siege && t < 10_000 {
        let mut keys = [0; SEATS];
        keys[0] = pack(Input::Ready).unwrap();
        keys[1] = keys[0];
        g.step(keys);
        t += 1;
    }
    assert_eq!(g.play.rules.phase, Phase::Siege);
    let mut keys = [0; SEATS];
    keys[1] = pack(Input::Swing).unwrap();
    g.step(keys);
    g.step([0; SEATS]);
    g.step([0; SEATS]);
    keys[1] = pack(Input::Release).unwrap();
    g.step(keys);
    let (field, inputs) = g.log.replay_of(0).expect("player 0's castle was attacked");
    assert_eq!(field.attacker, 1);
    let got: Vec<(u32, Input)> = inputs.iter().map(|l| (l.tick, l.input)).collect();
    assert_eq!(got, [(0, Input::Swing), (3, Input::Release)]);
}

