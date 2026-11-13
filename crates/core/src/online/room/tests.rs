//! Rooms over a simulated wire: the kit's way of testing without a transport.

use super::*;
use crate::lobby::NEED_READY;
use crate::online::{decode, encode, DESYNC};

fn how(client: &str, seed: u64) -> Opening {
    Opening {
        client: client.to_string(),
        name: client.to_string(),
        build: "b".to_string(),
        app_id: "royal-rubble-p2p".to_string(),
        seed,
        fast: true,
        no_net: true,
    }
}

/// Browsers joined by a wire with no lag: index 0 hosts. Each browser's
/// transport id is its client id.
struct Wire {
    rooms: Vec<Room>,
    now: i64,
}

impl Wire {
    fn new(joiners: usize) -> Wire {
        let mut host = Room::host(&how("c0", 7), 0);
        host.lobby.set_players(8);
        let code = host.code.clone();
        let mut rooms = vec![host];
        for k in 1..=joiners {
            rooms.push(Room::join(&how(&format!("c{k}"), 7 + k as u64), &code, 0));
        }
        Wire { rooms, now: 0 }
    }

    /// Introduces the host and joiner `k`.
    fn link(&mut self, k: usize) {
        let (host_id, join_id) = ("c0".to_string(), format!("c{k}"));
        self.rooms[0].event("connected", &join_id, "", false, 0);
        self.rooms[k].event("connected", &host_id, "", true, 0);
        self.settle();
    }

    /// One frame on every browser, delivering everything sent.
    fn frame(&mut self) {
        self.now += 17;
        let mut mail = Vec::new();
        for k in 0..self.rooms.len() {
            let from = format!("c{k}");
            let pump = self.rooms[k].pump(self.now);
            for m in pump.out {
                mail.push((from.clone(), m.to, m.text));
            }
        }
        for (from, to, text) in mail {
            let k: usize = to[1..].parse().unwrap();
            self.rooms[k].event("message", &from, &text, false, 0);
        }
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame();
        }
    }
}

fn seat_of(room: &Room, id: &str) -> usize {
    room.lobby.seat_of(id).unwrap()
}

#[test]
fn a_room_code_is_six_plain_letters_and_a_typed_one_is_checked() {
    let code = make_code(9);
    assert_eq!(code.len(), CODE_LEN);
    assert_eq!(clean_code(&code), Some(code.clone()));
    assert_ne!(make_code(9), make_code(10));
    assert_eq!(clean_code(" abcdef "), Some("ABCDEF".to_string()));
    for bad in ["", "ABCDE", "ABCDEFG", "ABC0EF", "ABCDE!"] {
        assert_eq!(clean_code(bad), None, "{bad:?}");
    }
}

#[test]
fn a_joiner_is_seated_by_the_host_and_sees_the_lobby() {
    let mut wire = Wire::new(1);
    assert_eq!(wire.rooms[1].status(), FINDING);
    assert!(!wire.rooms[1].heard);
    wire.link(1);
    assert!(wire.rooms[1].heard);
    assert_eq!(wire.rooms[1].status(), "");
    assert_eq!(wire.rooms[0].lobby, wire.rooms[1].lobby);
    assert_eq!(seat_of(&wire.rooms[1], "c1"), 1);
}

#[test]
fn eight_people_fill_the_room_and_a_ninth_is_told_it_is_full() {
    let mut wire = Wire::new(7);
    for k in 1..=7 {
        wire.link(k);
    }
    for k in 1..=7 {
        assert_eq!(wire.rooms[k].lobby, wire.rooms[0].lobby, "joiner {k}");
    }
    let mut late = Room::join(&how("c8", 3), &wire.rooms[0].code.clone(), 0);
    wire.rooms[0].event("connected", "c8", "", false, 0);
    late.event("connected", "c0", "", true, 0);
    let out = wire.rooms[0].pump(wire.now).out;
    let to_late: Vec<&Outgoing> = out.iter().filter(|m| m.to == "c8").collect();
    assert_eq!(to_late.len(), 1);
    late.event("message", "c0", &to_late[0].text, false, 0);
    assert_eq!(late.status(), FULL);
    assert!(late.lost());
}

#[test]
fn a_join_the_lobby_has_no_seat_for_is_turned_away() {
    let mut wire = Wire::new(1);
    for k in 1..8 {
        wire.rooms[0].lobby.add_person(&format!("p{k}"), "");
    }
    wire.link(1);
    assert_eq!(wire.rooms[1].status(), FULL);
    assert!(wire.rooms[1].lost());
    assert_eq!(wire.rooms[0].lobby.seat_of("c1"), None);
}

#[test]
fn the_ready_button_flips_a_joiners_ready() {
    let mut wire = Wire::new(1);
    wire.link(1);
    let ready = |wire: &Wire| matches!(&wire.rooms[0].lobby.seats[1], Who::Person { ready: true, .. });
    wire.rooms[1].act("lobby.ready", "").unwrap();
    wire.settle();
    assert!(ready(&wire));
    wire.rooms[1].act("lobby.ready", "").unwrap();
    wire.settle();
    assert!(!ready(&wire));
}

#[test]
fn a_joiner_asks_for_its_own_seat_and_cannot_use_the_hosts_options() {
    let mut wire = Wire::new(2);
    wire.link(1);
    wire.link(2);
    wire.rooms[1].act("lobby.name", "Morag").unwrap();
    wire.rooms[1].act("lobby.ready", "on").unwrap();
    wire.settle();
    let seat = &wire.rooms[2].lobby.seats[1];
    assert!(matches!(seat, Who::Person { name, ready: true, .. } if name == "Morag"));
    assert!(wire.rooms[1].act("lobby.players", "3").is_err());
    assert!(wire.rooms[1].act("lobby.wind", "off").is_err());
    assert!(wire.rooms[1].act("seat.3.level", "hard").is_err());
}

#[test]
fn the_hosts_options_and_bot_levels_reach_every_browser() {
    let mut wire = Wire::new(1);
    wire.link(1);
    wire.rooms[0].act("lobby.players", "4").unwrap();
    wire.rooms[0].act("lobby.wind", "off").unwrap();
    wire.rooms[0].act("seat.3.level", "hard").unwrap();
    wire.settle();
    let lobby = &wire.rooms[1].lobby;
    assert_eq!(lobby.options.players, 4);
    assert!(!lobby.options.wind);
    assert_eq!(
        lobby.seats[3],
        Who::Bot {
            level: Difficulty::Hard
        }
    );
    assert!(wire.rooms[0].act("seat.3.level", "brutal").is_err());
}

#[test]
fn a_joiner_cannot_take_another_browser_s_id() {
    let mut wire = Wire::new(2);
    wire.link(1);
    wire.rooms[0].event("connected", "c2", "", false, 0);
    let hello = encode(&Msg::Hello {
        protocol: crate::online::PROTOCOL,
        build: "b".into(),
    });
    wire.rooms[0].event("message", "c2", &hello, false, 0);
    let forged = Msg::Join {
        id: "c1".into(),
        name: "Forged".into(),
    };
    wire.rooms[0].event("message", "c2", &encode(&forged), false, 0);
    assert_eq!(wire.rooms[0].session.status, GARBLED);
    assert!(matches!(&wire.rooms[0].lobby.seats[1], Who::Person { name, .. } if name == "c1"));
}

#[test]
fn a_joiner_leaving_the_lobby_frees_its_seat_for_everyone() {
    let mut wire = Wire::new(2);
    wire.link(1);
    wire.link(2);
    wire.rooms[0].event("disconnected", "c1", "", false, 0);
    wire.settle();
    let bot = Who::Bot {
        level: Difficulty::Normal,
    };
    assert_eq!(wire.rooms[0].lobby.seats[1], bot);
    assert_eq!(wire.rooms[2].lobby.seats[1], bot);
}

#[test]
fn the_host_leaving_ends_the_room_for_a_joiner() {
    let mut wire = Wire::new(1);
    wire.link(1);
    wire.rooms[1].event("disconnected", "c0", "", false, 0);
    assert_eq!(wire.rooms[1].status(), LEFT_ROOM);
    assert!(wire.rooms[1].lost());
}

#[test]
fn a_start_reaches_every_browser_and_a_match_runs_the_same_on_each() {
    let mut wire = Wire::new(2);
    wire.link(1);
    wire.link(2);
    wire.rooms[1].act("lobby.ready", "on").unwrap();
    wire.rooms[2].act("lobby.ready", "on").unwrap();
    wire.settle();
    assert_eq!(wire.rooms[0].lobby.start_problem(), None);
    wire.rooms[0].act("lobby.start", "").unwrap();
    wire.settle();
    let start = wire.rooms[0].start.clone().expect("the host built a match");
    for k in 1..=2 {
        assert_eq!(wire.rooms[k].start.as_ref(), Some(&start), "browser {k}");
    }
    assert_eq!(start.seats.len(), 8);
    assert_eq!(start.seats.iter().filter(|s| s.bot.is_some()).count(), 5);
    // the people press Ready: the build ends early on every browser
    for t in 0..300 {
        if t == 30 {
            for room in wire.rooms.iter_mut() {
                room.queue(Input::Ready);
            }
        }
        wire.frame();
    }
    for _ in 0..60 {
        wire.frame();
    }
    for k in 0..3 {
        assert_eq!(wire.rooms[k].stage, Stage::Playing, "browser {k}");
        assert!(wire.rooms[k].session.tick > 300, "browser {k} ran");
        assert_ne!(wire.rooms[k].status(), DESYNC, "browser {k}");
        let phase = wire.rooms[k].game.as_ref().unwrap().play.rules.phase;
        assert_ne!(phase, Phase::Build, "browser {k}: Ready reached the match");
    }
    let confirmed = wire.rooms.iter().map(|r| r.session.confirmed).min().unwrap();
    assert!(confirmed > 240, "inputs crossed the host: confirmed {confirmed}");
    let sums: Vec<_> = wire.rooms.iter().map(|r| r.session.newest_sum()).collect();
    assert!(sums[0].is_some());
    assert!(sums.iter().all(|s| *s == sums[0]), "{sums:?}");
}

#[test]
fn a_start_that_cannot_stand_is_refused_and_says_why() {
    let mut wire = Wire::new(1);
    wire.link(1);
    assert_eq!(wire.rooms[0].lobby.start_problem(), Some(NEED_READY));
    wire.rooms[0].act("lobby.start", "").unwrap();
    assert_eq!(wire.rooms[0].stage, Stage::Lobby);
    assert!(wire.rooms[0].start.is_none());
}

#[test]
fn a_browser_too_late_for_the_start_is_told_so() {
    let mut wire = Wire::new(1);
    wire.link(1);
    wire.rooms[1].act("lobby.ready", "on").unwrap();
    wire.settle();
    wire.rooms[0].act("lobby.start", "").unwrap();
    wire.settle();
    wire.rooms[0].event("connected", "c9", "", false, 0);
    let out = wire.rooms[0].pump(wire.now).out;
    let bye = out.iter().find(|m| m.to == "c9").expect("told");
    assert_eq!(decode(&bye.text), Some(Msg::Bye { why: STARTED.into() }));
}

/// The joiner's Join is still on the wire as the host presses Start.
#[test]
fn a_joiner_the_start_left_out_is_told_so() {
    let mut wire = Wire::new(1);
    wire.link(1);
    let start = Lobby::new("c0", "c0").start(1).unwrap();
    wire.rooms[1].event("message", "c0", &encode(&Msg::Start { start }), false, 0);
    assert_eq!(wire.rooms[1].status(), STARTED);
    assert_eq!(wire.rooms[1].stage, Stage::Lobby);
    assert!(wire.rooms[1].lost());
}

#[test]
fn the_countdown_runs_unless_the_page_is_fast_and_then_the_ticks_start() {
    let mut slow = how("c0", 1);
    slow.fast = false;
    let mut room = Room::solo(&slow);
    room.act("lobby.start", "").unwrap();
    assert_eq!(room.counting(0), Some(3));
    room.pump(1_000);
    assert_eq!(room.counting(1_500), Some(3));
    assert_eq!(room.counting(2_500), Some(2));
    assert_eq!(room.counting(3_500), Some(1));
    room.pump(3_999);
    assert!(matches!(room.stage, Stage::Counting { .. }));
    room.pump(4_000);
    assert_eq!(room.stage, Stage::Playing);
    assert_eq!(room.counting(4_000), None);

    let mut room = Room::solo(&how("c0", 1));
    room.act("lobby.start", "").unwrap();
    room.pump(10);
    assert_eq!(room.stage, Stage::Playing, "?fast skips the wait");
}

#[test]
fn a_solo_room_opens_with_a_bot_and_can_start_at_once() {
    let mut room = Room::solo(&how("me", 1));
    assert_eq!(
        room.lobby.seats[1],
        Who::Bot {
            level: Difficulty::Normal
        }
    );
    assert_eq!(room.lobby.start_problem(), None);
    assert_eq!(room.status(), "");
    room.act("lobby.start", "").unwrap();
    room.pump(17);
    assert_eq!(room.stage, Stage::Playing, "a solo match waits for nobody");
    assert!(room.game.is_none(), "the arena runs a solo match");
}

#[test]
fn the_relays_decide_when_a_room_opens_and_a_dead_network_says_so() {
    let mut online = how("c0", 7);
    online.no_net = false;
    let mut room = Room::host(&online, 0);
    let pump = room.pump(1);
    let Request::ProbeRelays { id, .. } = pump.requests[0] else {
        panic!("a room starts with a probe: {:?}", pump.requests);
    };
    room.event(
        "relays",
        "",
        &format!(r#"{{"id":{id},"live":["wss://a"]}}"#),
        false,
        2,
    );
    let pump = room.pump(3);
    let wanted = Request::HostRoom {
        room: room_name("royal-rubble-p2p", &room.code),
        relays: vec!["wss://a".to_string()],
    };
    assert_eq!(pump.requests, vec![wanted]);

    let mut room = Room::join(&online, "ABCDEF", 0);
    let mut now = 0;
    for _ in 0..200 {
        now += 500;
        let pump = room.pump(now);
        for r in pump.requests {
            if let Request::ProbeRelays { id, .. } = r {
                room.event("relays", "", &format!(r#"{{"id":{id},"live":[]}}"#), false, now);
            }
        }
    }
    assert_eq!(room.status(), NO_RELAY);
    assert!(room.lost(), "never a silent hang");
}

#[test]
fn a_transport_error_shows_its_key_and_an_unknown_one_reads_as_a_failed_connection() {
    let mut room = Room::host(&how("c0", 1), 0);
    room.event("error", "", "online.no_path", false, 0);
    assert_eq!(room.status(), "online.no_path");
    room.event("error", "", "<script>", false, 0);
    assert_eq!(room.status(), SIGNAL_FAILED);
}

#[test]
fn a_joiner_ignores_the_other_joiners_trystero_links_it_to() {
    let mut wire = Wire::new(2);
    wire.link(1);
    wire.rooms[1].event("connected", "c2", "", false, 0);
    wire.rooms[1].event("message", "c2", "{garbage", false, 0);
    assert_eq!(wire.rooms[1].status(), "", "a joiner hears the host alone");
    assert_eq!(wire.rooms[1].session.peers.len(), 1);
}

/// How `how` reads with a transport: the pasted-code tests want its requests.
fn netted(client: &str) -> Opening {
    Opening {
        no_net: false,
        ..how(client, 1)
    }
}

#[test]
fn a_host_by_code_asks_for_an_invite_and_no_relay_and_shows_it_when_it_comes() {
    let mut host = Room::host_by_code(&netted("h"));
    assert_eq!(host.pump(0).requests, [Request::HostByCode]);
    assert_eq!(host.pump(1).requests, []);
    assert_eq!(host.pasted_code(), Some(""));
    assert_eq!(host.code, "");
    host.event("code", "", "Oinvite", false, 2);
    assert_eq!(host.pasted_code(), Some("Oinvite"));
}

#[test]
fn an_invite_is_good_for_one_joiner_and_the_next_gets_a_fresh_one() {
    let mut host = Room::host_by_code(&netted("h"));
    host.pump(0);
    host.event("code", "", "Ofirst", false, 1);
    host.event("connected", "j", "", false, 2);
    assert_eq!(host.pasted_code(), Some(""));
    assert_eq!(host.pump(3).requests, [Request::InviteByCode]);
}

#[test]
fn a_room_met_on_relays_never_asks_for_a_pasted_code() {
    let mut host = Room::host(&netted("h"), 0);
    host.pump(0);
    host.event("connected", "j", "", false, 2);
    assert_eq!(host.pasted_code(), None);
    assert!(!host.pump(3).requests.contains(&Request::InviteByCode));
}

#[test]
fn a_joiner_by_code_hands_on_the_invite_and_is_told_to_send_its_reply() {
    let mut join = Room::join_by_code(&netted("j"), "Oinvite");
    let asked = Request::JoinByCode {
        invite: "Oinvite".to_string(),
    };
    assert_eq!(join.pump(0).requests, [asked]);
    assert_eq!(join.status(), "");
    assert!(join.making_code());
    join.event("code", "", "Areply", false, 1);
    assert_eq!(join.status(), SEND_REPLY);
    assert!(!join.making_code());
}

#[test]
fn a_refused_reply_can_be_pasted_again_and_clears_the_refusal() {
    let mut host = Room::host_by_code(&netted("h"));
    host.pump(0);
    host.accept_reply("Onot_a_reply");
    let first = Request::AcceptReply {
        reply: "Onot_a_reply".to_string(),
    };
    assert_eq!(host.pump(1).requests, [first]);
    host.event("error", "", "online.wrong_code", false, 2);
    assert_eq!(host.status(), "online.wrong_code");
    host.event("error", "", "online.bad_paste", false, 3);
    assert_eq!(host.status(), "online.bad_paste");
    host.accept_reply("Areply");
    assert_eq!(host.status(), "online.waiting");
}

#[test]
fn a_refused_invite_lets_the_joiner_press_again() {
    let mut join = Room::join_by_code(&netted("j"), "nonsense");
    assert!(join.making_code());
    join.event("error", "", "online.bad_paste", false, 1);
    assert!(!join.making_code());
}

#[test]
fn with_the_net_off_a_room_by_code_asks_the_transport_for_nothing() {
    let mut host = Room::host_by_code(&how("h", 1));
    assert_eq!(host.pump(0).requests, []);
}
