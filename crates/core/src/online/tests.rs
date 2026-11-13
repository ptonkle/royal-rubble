//! The kit's required tests, played on the eight-seat session, and the tests
//! the eight seats add: the host sending inputs on, and a seat left mid-match.

use super::*;

const HOST: Peer = 1;
const JOINER: Peer = 2;

/// The keys each seat presses for a tick: a fixed script, so a run can be
/// compared with a copy fed every input on time.
fn script(seat: usize, tick: u32) -> u64 {
    if tick < DELAY {
        return 0;
    }
    match (tick * (seat as u32 + 3)) % 5 {
        0 | 1 => LEFT,
        2 => RIGHT,
        3 => LEFT | RIGHT,
        _ => 0,
    }
}

fn truth(ticks: u32, seats: usize) -> World {
    let mut w = World::default();
    for t in 0..ticks {
        let mut keys = [0; SEATS];
        for (seat, k) in keys.iter_mut().enumerate().take(seats) {
            *k = script(seat, t);
        }
        w.step(keys);
    }
    w
}

fn active(seats: usize) -> [bool; SEATS] {
    let mut a = [false; SEATS];
    a[..seats].fill(true);
    a
}

/// A host and a joiner that have exchanged hellos, no match begun.
fn met(build_host: &str, build_join: &str) -> (Session, Session) {
    let mut host = Session::new(true, build_host);
    let mut join = Session::new(false, build_join);
    host.connected(JOINER);
    join.connected(HOST);
    for (_, text) in host.take_outbox() {
        join.message(HOST, &text);
    }
    for (_, text) in join.take_outbox() {
        host.message(JOINER, &text);
    }
    (host, join)
}

/// A host and a joiner in a two-seat match.
fn pair(build_host: &str, build_join: &str) -> (Session, Session) {
    let (mut host, mut join) = met(build_host, build_join);
    host.begin(0, active(2), &[(JOINER, 1)]);
    join.begin(1, active(2), &[(HOST, 0)]);
    (host, join)
}

#[test]
fn every_message_round_trips() {
    let lobby = Lobby::new("h", "Host");
    let start = lobby.clone();
    let mut with_bot = start.clone();
    with_bot.set_players(3);
    let all = [
        Msg::Hello {
            protocol: PROTOCOL,
            build: "abc123".into(),
        },
        Msg::Input { tick: 7, keys: 3 },
        Msg::Inputs {
            rows: vec![[1, 7, 3], [2, 7, 0]],
        },
        Msg::Gone { seat: 3, from: 90 },
        Msg::Sum {
            tick: 30,
            sum: u64::MAX,
        },
        Msg::Bye { why: FULL.into() },
        Msg::Join {
            id: "c".into(),
            name: "Morag".into(),
        },
        Msg::Ask {
            ask: Ask::Name { name: "M".into() },
        },
        Msg::Ask {
            ask: Ask::Ready { ready: true },
        },
        Msg::Roster { lobby },
        Msg::Start {
            start: with_bot.start(5).unwrap(),
        },
    ];
    for m in all {
        assert_eq!(decode(&encode(&m)), Some(m.clone()), "{m:?}");
    }
}

#[test]
fn the_room_name_carries_the_protocol_and_not_the_build() {
    let name = room_name("my-game", "ABCDE");
    assert_eq!(name, format!("my-game-p{PROTOCOL}-ABCDE"));
}

#[test]
fn another_build_or_protocol_is_turned_away_with_a_reason() {
    let (host, join) = met("b1", "b2");
    assert_eq!(host.status, RELOAD);
    assert_eq!(join.status, RELOAD);
    assert!(host.peers.is_empty() && join.peers.is_empty());

    let mut host: Session = Session::new(true, "b1");
    host.connected(JOINER);
    host.take_outbox();
    let old = encode(&Msg::Hello {
        protocol: PROTOCOL + 1,
        build: "b1".into(),
    });
    host.message(JOINER, &old);
    assert_eq!(host.status, RELOAD);
    let out = host.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!(decode(&out[0].1), Some(Msg::Bye { why: RELOAD.into() }));

    // the far end reads the reason as well as the departure
    let mut other: Session = Session::new(false, "b9");
    other.connected(HOST);
    other.message(HOST, &out[0].1);
    assert_eq!(other.status, RELOAD);
}

#[test]
fn garbage_on_the_wire_changes_only_the_status() {
    let (mut host, _join) = pair("b", "b");
    let mut world = World::default();
    host.local(LEFT);
    host.take_outbox();
    let before = (host.tick, host.inputs.clone(), host.peers.clone());
    for junk in [
        "{nope",
        "",
        "{}",
        "[1,2,3]",
        r#"{"t":"input","tick":"x","keys":1}"#,
        r#"{"t":"input","tick":4000000000,"keys":1}"#,
        r#"{"t":"teleport","to":9}"#,
    ] {
        host.status = PLAYING;
        host.message(JOINER, junk);
        assert_eq!(host.status, GARBLED, "{junk:?}");
        assert_eq!((host.tick, host.inputs.clone(), host.peers.clone()), before);
        assert!(host.take_outbox().is_empty(), "{junk:?} drew a reply");
    }
    assert!(host.step(&mut world), "the prefilled first tick still runs");

    // a second input for the same tick is garbage too
    host.status = PLAYING;
    host.message(JOINER, &encode(&Msg::Input { tick: 2, keys: 1 }));
    assert_eq!(host.status, PLAYING);
    host.message(JOINER, &encode(&Msg::Input { tick: 2, keys: 2 }));
    assert_eq!(host.status, GARBLED);

    // a browser that never said hello is not heard at all
    host.status = PLAYING;
    host.message(9, "{nope");
    host.message(9, &encode(&Msg::Input { tick: 3, keys: 1 }));
    assert_eq!(host.status, PLAYING);

    // an unknown reason on a bye reads as a plain departure
    host.message(
        JOINER,
        &encode(&Msg::Bye {
            why: "<b>x</b>".into(),
        }),
    );
    assert_eq!(host.status, LEFT_ROOM);
}

#[test]
fn a_joiner_hears_inputs_from_the_host_alone_and_only_for_other_seats() {
    let (_host, mut join) = pair("b", "b");
    let keep = join.inputs.clone();
    for junk in [
        // its own seat, and a seat past the last
        Msg::Inputs {
            rows: vec![[1, 5, 1]],
        },
        Msg::Inputs {
            rows: vec![[SEATS as u64, 5, 1]],
        },
        // the shape a joiner sends, which no host sends
        Msg::Input { tick: 5, keys: 1 },
        Msg::Gone {
            seat: SEATS as u32,
            from: 5,
        },
    ] {
        join.status = PLAYING;
        join.message(HOST, &encode(&junk));
        assert_eq!(join.status, GARBLED, "{junk:?}");
        assert_eq!(join.inputs, keep, "{junk:?}");
    }
}

#[test]
fn a_copy_runs_on_a_guess_and_never_more_than_its_limit_ahead() {
    let (mut host, _join) = pair("b", "b");
    let mut world = World::default();
    let mut ran = 0;
    for _ in 0..(DELAY + MAX_AHEAD + 5) {
        host.local(RIGHT);
        if host.step(&mut world) {
            ran += 1;
        }
    }
    // the prefilled ticks confirm on their own; past them the host may
    // run MAX_AHEAD ticks on guesses for the joiner, and no more
    assert_eq!(host.confirmed, DELAY);
    assert_eq!(ran, DELAY + MAX_AHEAD);
    assert_eq!(host.tick, DELAY + MAX_AHEAD);
    host.message(JOINER, &encode(&Msg::Input { tick: DELAY, keys: 0 }));
    assert!(host.step(&mut world), "one real input frees one tick");
}

#[test]
fn a_late_input_that_differs_rolls_back_to_the_world_of_a_copy_fed_on_time() {
    let (mut host, _join) = pair("b", "b");
    let mut world = World::default();
    let ticks = DELAY + 6;
    while host.tick < ticks {
        host.local(script(0, host.tick + DELAY));
        assert!(host.step(&mut world));
    }
    assert_eq!(host.rollbacks, 0, "nothing has contradicted a guess yet");
    // the joiner's inputs arrive late, and the guess (still 0) was wrong
    for t in DELAY..ticks {
        host.message(
            JOINER,
            &encode(&Msg::Input {
                tick: t,
                keys: script(1, t),
            }),
        );
    }
    host.settle(&mut world);
    assert_eq!(host.rollbacks, 1, "one replay covers every late input");
    // the host's own inputs for ticks under DELAY are the prefilled zeros,
    // which script() also gives, so the true run is truth(ticks)
    assert_eq!(world, truth(ticks, 2));
    assert_eq!(host.confirmed, ticks);
}

/// Everything a star of `seats` browsers does in one frame, with `lag` frames
/// on each leg of the wire. Index 0 is the host.
struct Star {
    browsers: Vec<Session>,
    worlds: Vec<World>,
    /// (arrives at frame, from peer, to browser, text)
    wire: VecDeque<(usize, Peer, usize, String)>,
    lag: usize,
    frame: usize,
}

impl Star {
    fn new(seats: usize, lag: usize) -> Star {
        let mut browsers = vec![Session::new(true, "b")];
        for _ in 1..seats {
            browsers.push(Session::new(false, "b"));
        }
        // a joiner's peer number on the host is its seat, and the host is
        // peer 100 on each joiner
        for j in 1..seats {
            browsers[0].connected(j as Peer);
            browsers[j].connected(100);
            let to_join = browsers[0].take_outbox();
            let to_host = browsers[j].take_outbox();
            for (_, text) in to_join {
                browsers[j].message(100, &text);
            }
            for (_, text) in to_host {
                browsers[0].message(j as Peer, &text);
            }
            browsers[j].begin(j, active(seats), &[(100, 0)]);
        }
        let map: Vec<(Peer, usize)> = (1..seats).map(|j| (j as Peer, j)).collect();
        browsers[0].begin(0, active(seats), &map);
        Star {
            worlds: vec![World::default(); seats],
            browsers,
            wire: VecDeque::new(),
            lag,
            frame: 0,
        }
    }

    fn post(&mut self, k: usize) {
        for (to, text) in self.browsers[k].take_outbox() {
            let arrive = self.frame + self.lag;
            if k == 0 {
                self.wire.push_back((arrive, 100, to as usize, text));
            } else {
                self.wire.push_back((arrive, k as Peer, 0, text));
            }
        }
    }

    fn deliver(&mut self) {
        let frame = self.frame;
        let mut later = VecDeque::new();
        while let Some((at, from, to, text)) = self.wire.pop_front() {
            if at > frame {
                later.push_back((at, from, to, text));
            } else {
                self.browsers[to].message(from, &text);
            }
        }
        self.wire = later;
    }

    fn drain(&mut self) {
        loop {
            for k in 0..self.browsers.len() {
                self.post(k);
            }
            if self.wire.is_empty() {
                break;
            }
            self.frame += 1;
            self.deliver();
        }
        for k in 0..self.browsers.len() {
            let (s, w) = (&mut self.browsers[k], &mut self.worlds[k]);
            s.settle(w);
        }
    }

    /// Plays `ticks` ticks on every browser. The script is keyed to the
    /// tick an input lands on, so a true run can be worked out alone.
    fn play(&mut self, ticks: u32) {
        while self.browsers.iter().any(|s| s.tick < ticks) && self.frame < 20_000 {
            for k in 0..self.browsers.len() {
                let keys = script(k, self.browsers[k].tick + DELAY);
                self.browsers[k].local(keys);
                self.post(k);
            }
            self.deliver();
            for k in 0..self.browsers.len() {
                if self.browsers[k].tick < ticks {
                    let (s, w) = (&mut self.browsers[k], &mut self.worlds[k]);
                    s.step(w);
                }
            }
            self.frame += 1;
        }
        self.drain();
    }
}

#[test]
fn two_lagged_copies_reach_the_world_of_a_copy_fed_on_time() {
    let ticks = 300;
    for lag in [0usize, 1, 3, 5] {
        let mut star = Star::new(2, lag);
        star.play(ticks);
        for k in 0..2 {
            assert_eq!(star.browsers[k].tick, ticks, "lag {lag}: browser {k} stalled");
            assert_eq!(star.worlds[k], truth(ticks, 2), "lag {lag}: browser {k}");
            assert_eq!(star.browsers[k].status, PLAYING, "lag {lag}: browser {k}");
        }
    }
}

#[test]
fn eight_lagged_copies_reach_the_world_of_a_copy_fed_on_time_through_the_host() {
    let ticks = 300;
    for lag in [0usize, 1, 3, 5] {
        let mut star = Star::new(8, lag);
        star.play(ticks);
        for k in 0..8 {
            assert_eq!(star.browsers[k].tick, ticks, "lag {lag}: browser {k} stalled");
            assert_eq!(star.worlds[k], truth(ticks, 8), "lag {lag}: browser {k}");
            assert_eq!(star.browsers[k].status, PLAYING, "lag {lag}: browser {k}");
        }
    }
}

#[test]
fn a_seat_a_bot_holds_is_never_waited_for() {
    // the host and one joiner in seats 0 and 1, bots in the other six
    let mut star = Star::new(2, 2);
    star.play(100);
    assert_eq!(star.browsers[0].tick, 100);
    assert_eq!(star.worlds[0], truth(100, 2));
}

#[test]
fn a_joiner_leaving_mid_match_frees_the_others_and_every_copy_agrees() {
    let mut star = Star::new(3, 1);
    star.play(60);
    let host_saw = star.browsers[0].inputs[2].keys().next_back().copied().unwrap();
    // the third browser is gone, with some of its inputs still on the wire
    star.browsers.pop();
    star.worlds.pop();
    star.wire.retain(|m| m.2 != 2);
    star.browsers[0].left(2);
    star.drain();
    let from = star.browsers[0].gone[2].unwrap();
    assert_eq!(from, host_saw + 1, "the seat ends where the host's stream ends");
    assert_eq!(star.browsers[1].gone[2], Some(from), "the host said so");
    star.play(200);
    assert_eq!(star.browsers[0].tick, 200, "the host did not stall");
    assert_eq!(star.browsers[1].tick, 200, "the other joiner did not stall");
    assert_eq!(star.worlds[0], star.worlds[1]);
    assert_eq!(star.browsers[0].status, PLAYING);
    assert_eq!(star.browsers[1].status, PLAYING);
}

#[test]
fn a_peer_that_leaves_is_reported_and_a_browser_too_many_is_told_the_room_is_full() {
    // the host and seven joiners fill eight seats
    let mut host: Session = Session::new(true, "b");
    for k in 1..SEATS as Peer {
        host.connected(k);
    }
    host.take_outbox();
    let hello = encode(&Msg::Hello {
        protocol: PROTOCOL,
        build: "b".into(),
    });
    for k in 1..SEATS as Peer {
        host.message(k, &hello);
    }
    assert_eq!(host.peers.len(), SEATS - 1);
    host.connected(50);
    let out = host.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, 50);
    assert_eq!(decode(&out[0].1), Some(Msg::Bye { why: FULL.into() }));
    assert!(!host.peers.contains_key(&50), "the ninth browser took no seat");

    // a stranger leaving changes nothing; a joiner hears the host go
    let (mut host, mut join) = pair("b", "b");
    host.left(3);
    assert_eq!(host.status, PLAYING, "a stranger leaving changes nothing");
    join.left(HOST);
    assert_eq!(join.status, LEFT_ROOM);
    assert!(join.peers.is_empty());
}

#[test]
fn a_browser_that_arrives_after_the_start_is_told_so() {
    let (mut host, _join) = pair("b", "b");
    host.take_outbox();
    host.connected(7);
    let out = host.take_outbox();
    assert_eq!(decode(&out[0].1), Some(Msg::Bye { why: STARTED.into() }));
}

#[test]
fn worlds_that_disagree_are_reported_as_a_desync() {
    let (mut host, _join) = pair("b", "b");
    let mut world = World::default();
    while host.tick < SUM_EVERY {
        host.local(0);
        let t = host.tick;
        if t >= DELAY {
            host.message(JOINER, &encode(&Msg::Input { tick: t, keys: 0 }));
        }
        assert!(host.step(&mut world));
    }
    let mine = world.sum();
    host.message(
        JOINER,
        &encode(&Msg::Sum {
            tick: SUM_EVERY,
            sum: mine ^ 1,
        }),
    );
    assert_eq!(host.status, DESYNC);
}

#[test]
fn a_checksum_that_beats_the_tick_it_covers_is_still_compared() {
    let (mut host, _join) = pair("b", "b");
    let mut world = World::default();
    host.message(
        JOINER,
        &encode(&Msg::Sum {
            tick: SUM_EVERY,
            sum: 1,
        }),
    );
    while host.tick < SUM_EVERY {
        host.local(0);
        let t = host.tick;
        if t >= DELAY {
            host.message(JOINER, &encode(&Msg::Input { tick: t, keys: 0 }));
        }
        host.step(&mut world);
    }
    assert_eq!(host.status, DESYNC);
}
