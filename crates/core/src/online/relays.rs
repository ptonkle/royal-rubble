//! Finding the other browsers over the pinned Nostr relays, and recovering
//! when relays go down. Ported from Chromatic, whose lobbies had failed on
//! dead relays (its R1.41 and R1.57, Crabble's F26, Ape Race's T114): a dead
//! relay alone does not stop a room, but a room fails when the peers' live
//! relays do not overlap or all fail, so here nobody opens a room blind.
//!
//! A pure state machine, like [`super::Session`]: the page reports what
//! happened (a probe's answer, a peer arriving, a transport error) and the
//! time, and [`Finder::take_requests`] says what to ask of the transport.
//!
//! 1. **Probe.** Before opening a room, the page tries every pinned relay at
//!    once (`web/net/rtc.js` `probeRelays`): a WebSocket open, then
//!    [`probe_req`] answered by EOSE within [`PROBE_TIMEOUT_MS`]. Only the
//!    relays that answer are handed to Trystero, all of them, so the two
//!    browsers (which probe the same pinned list) share as many as they can.
//! 2. **Wait.** A joiner that has not met the host within
//!    [`FIND_TIMEOUT_MS`] probes again and reopens the room on whatever is
//!    live now, up to [`ATTEMPTS`] rounds in all, then gives up with
//!    `online.timeout`. A host has no one to wait for, so it never times out;
//!    while alone it re-probes every [`RECHECK_MS`] and reopens the room only
//!    if the live set has changed, so a joiner always finds it on the relays
//!    that are up.
//! 3. **No relay.** A probe that finds no live relay, or a transport report
//!    of `online.no_relay`, tries again after [`RETRY_DELAY_MS`]; after
//!    [`ATTEMPTS`] rounds in a row with no relay it gives up with
//!    `online.no_relay`. Either failure closes the room: never a silent hang.
//!
//! Once this browser has met someone (a joiner: the host), the relays have
//! done their job and the finder stops.

use serde::Serialize;

use super::room::Request;

/// How long each relay has to open its socket and answer the probe's REQ
/// with EOSE.
pub const PROBE_TIMEOUT_MS: i64 = 4_000;
/// The page's answer to a probe is given up on this long after the probe's
/// own timeout (a page that lost the request).
pub const PROBE_GRACE_MS: i64 = 2_000;
/// How long a joiner waits to meet the host before it probes again and
/// reopens the room.
pub const FIND_TIMEOUT_MS: i64 = 20_000;
/// How often a host still alone re-probes the relays.
pub const RECHECK_MS: i64 = 30_000;
/// The pause before probing again after finding no live relay.
pub const RETRY_DELAY_MS: i64 = 2_000;
/// Rounds before giving up: a joiner's tries at meeting the host, or
/// anyone's probes in a row that found no relay.
pub const ATTEMPTS: u32 = 3;
/// The Nostr kind the probe subscribes to: an ephemeral kind (20000-29999,
/// NIP-16), as Trystero's own signaling events are, so a relay that
/// refuses ephemeral subscriptions fails the probe.
pub const PROBE_KIND: u32 = 29_999;

/// The REQ a probe sends: a subscription to an ephemeral kind under a tag no
/// one publishes. Relays keep no ephemeral events, so a working relay
/// answers EOSE at once.
pub fn probe_req() -> String {
    serde_json::json!([
        "REQ",
        "royal-rubble-probe",
        { "kinds": [PROBE_KIND], "#x": ["royal-rubble-probe"], "limit": 1 }
    ])
    .to_string()
}

/// Which side of the room this browser is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Host,
    Join,
}

/// Where finding the others stands, as a lobby would show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "relays", rename_all = "snake_case")]
pub enum RelayStatus {
    /// Asking the pinned relays which are up (`attempt` from 1).
    Probing { attempt: u32 },
    /// Pausing before probing again: no relay answered.
    Retrying { attempt: u32 },
    /// The room is open on `live` relays; waiting for someone.
    Open { attempt: u32, live: usize },
    /// Met: the relays have done their job.
    Met,
    /// Gave up; `error` is a key for `data/` (`online.no_relay`,
    /// `online.timeout`).
    Failed { error: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Probing { id: u32, since: i64 },
    Retrying { until: i64 },
    Open { since: i64 },
    Met,
    Failed(&'static str),
}

/// One browser's search for the others over the relays.
#[derive(Clone, Debug)]
pub struct Finder {
    side: Side,
    room: String,
    state: State,
    /// The joiner's round at meeting the host (from 1).
    round: u32,
    /// Probes in a row that found no live relay.
    dry: u32,
    /// The relays the open room is on (empty: no room open).
    open_on: Vec<String>,
    probes: u32,
    requests: Vec<Request>,
}

impl Finder {
    /// Starts by probing the relays.
    pub fn new(side: Side, room: &str, now_ms: i64) -> Finder {
        let mut f = Finder {
            side,
            room: room.to_string(),
            state: State::Met,
            round: 1,
            dry: 0,
            open_on: Vec::new(),
            probes: 0,
            requests: Vec::new(),
        };
        f.probe(now_ms);
        f
    }

    fn probe(&mut self, now_ms: i64) {
        self.probes += 1;
        self.state = State::Probing {
            id: self.probes,
            since: now_ms,
        };
        self.requests.push(Request::ProbeRelays {
            id: self.probes,
            timeout_ms: PROBE_TIMEOUT_MS,
            req: probe_req(),
        });
    }

    fn open(&mut self, live: Vec<String>, now_ms: i64) {
        let room = self.room.clone();
        self.requests.push(match self.side {
            Side::Host => Request::HostRoom {
                room,
                relays: live.clone(),
            },
            Side::Join => Request::JoinRoom {
                room,
                relays: live.clone(),
            },
        });
        self.open_on = live;
        self.state = State::Open { since: now_ms };
    }

    fn fail(&mut self, key: &'static str) {
        self.state = State::Failed(key);
        self.open_on.clear();
        self.requests.push(Request::Leave);
    }

    /// No live relay this round: pause and probe again, or give up.
    fn dry_round(&mut self, now_ms: i64) {
        self.dry += 1;
        if self.dry >= ATTEMPTS {
            self.fail("online.no_relay");
        } else {
            self.state = State::Retrying {
                until: now_ms + RETRY_DELAY_MS,
            };
        }
    }

    /// The page's answer to probe `id`: the pinned relays that answered.
    /// An answer to an older probe is ignored.
    pub fn probed(&mut self, id: u32, live: Vec<String>, now_ms: i64) {
        if !matches!(self.state, State::Probing { id: current, .. } if current == id) {
            return;
        }
        if live.is_empty() {
            self.dry_round(now_ms);
            return;
        }
        self.dry = 0;
        if self.side == Side::Host && live == self.open_on {
            // A host's recheck found the room's relays still up: keep it.
            self.state = State::Open { since: now_ms };
        } else {
            self.open(live, now_ms);
        }
    }

    /// Someone arrived (`host`: they say they are the host). A joiner has
    /// met when the host arrives; a host when anyone does.
    pub fn connected(&mut self, host: bool) {
        let met = self.side == Side::Host || host;
        if met && !matches!(self.state, State::Failed(_)) {
            self.state = State::Met;
        }
    }

    /// The transport says no relay is reachable (`online.no_relay`): count
    /// a dry round and probe again.
    pub fn no_relay(&mut self, now_ms: i64) {
        if matches!(self.state, State::Open { .. }) {
            self.dry_round(now_ms);
        }
    }

    /// Every frame: timeouts.
    pub fn update(&mut self, now_ms: i64) {
        match self.state {
            State::Probing { since, .. } if now_ms >= since + PROBE_TIMEOUT_MS + PROBE_GRACE_MS => {
                self.dry_round(now_ms);
            }
            State::Retrying { until } if now_ms >= until => self.probe(now_ms),
            State::Open { since } => match self.side {
                Side::Join if now_ms >= since + FIND_TIMEOUT_MS => {
                    if self.round >= ATTEMPTS {
                        self.fail("online.timeout");
                    } else {
                        self.round += 1;
                        self.probe(now_ms);
                    }
                }
                Side::Host if now_ms >= since + RECHECK_MS => self.probe(now_ms),
                _ => {}
            },
            _ => {}
        }
    }

    /// What to ask of the transport since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    pub fn status(&self) -> RelayStatus {
        let attempt = match self.side {
            Side::Join => self.round,
            Side::Host => self.dry + 1,
        };
        match &self.state {
            State::Probing { .. } => RelayStatus::Probing { attempt },
            State::Retrying { .. } => RelayStatus::Retrying { attempt },
            State::Open { .. } => RelayStatus::Open {
                attempt,
                live: self.open_on.len(),
            },
            State::Met => RelayStatus::Met,
            State::Failed(key) => RelayStatus::Failed {
                error: key.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(urls: &[&str]) -> Vec<String> {
        urls.iter().map(|u| u.to_string()).collect()
    }

    fn ops(f: &mut Finder) -> Vec<Request> {
        f.take_requests()
    }

    fn probe_id(r: &Request) -> u32 {
        match r {
            Request::ProbeRelays { id, .. } => *id,
            other => panic!("expected a probe, got {other:?}"),
        }
    }

    #[test]
    fn a_room_opens_on_every_live_relay_and_only_those() {
        let mut f = Finder::new(Side::Join, "room", 0);
        let first = ops(&mut f);
        assert_eq!(first.len(), 1);
        let id = probe_id(&first[0]);
        f.probed(id, live(&["wss://a", "wss://c", "wss://d"]), 100);
        assert_eq!(
            ops(&mut f),
            vec![Request::JoinRoom {
                room: "room".into(),
                relays: live(&["wss://a", "wss://c", "wss://d"]),
            }]
        );
        assert_eq!(f.status(), RelayStatus::Open { attempt: 1, live: 3 });
        f.connected(true);
        assert_eq!(f.status(), RelayStatus::Met);
        f.update(1_000_000);
        assert!(ops(&mut f).is_empty(), "nothing more once met");
    }

    #[test]
    fn a_joiner_who_meets_no_host_reprobes_then_fails_with_a_timeout() {
        let mut f = Finder::new(Side::Join, "room", 0);
        let mut now = 0;
        for round in 1..=ATTEMPTS {
            let id = probe_id(&ops(&mut f)[0]);
            f.probed(id, live(&["wss://a"]), now);
            assert!(matches!(ops(&mut f)[..], [Request::JoinRoom { .. }]));
            assert_eq!(
                f.status(),
                RelayStatus::Open {
                    attempt: round,
                    live: 1
                }
            );
            // A peer that is not the host does not count as meeting.
            f.connected(false);
            f.update(now + FIND_TIMEOUT_MS - 1);
            assert!(ops(&mut f).is_empty(), "still waiting in round {round}");
            now += FIND_TIMEOUT_MS;
            f.update(now);
        }
        assert_eq!(ops(&mut f), vec![Request::Leave]);
        assert_eq!(
            f.status(),
            RelayStatus::Failed {
                error: "online.timeout".into()
            }
        );
        f.update(now * 2);
        assert!(ops(&mut f).is_empty(), "a failure is final, not a hang or a loop");
    }

    #[test]
    fn no_live_relay_retries_after_a_pause_then_fails_with_no_relay() {
        let mut f = Finder::new(Side::Host, "room", 0);
        let mut now = 0;
        for attempt in 1..=ATTEMPTS {
            let id = probe_id(&ops(&mut f)[0]);
            assert_eq!(f.status(), RelayStatus::Probing { attempt });
            f.probed(id, Vec::new(), now);
            if attempt < ATTEMPTS {
                assert_eq!(f.status(), RelayStatus::Retrying { attempt: attempt + 1 });
                f.update(now + RETRY_DELAY_MS - 1);
                assert!(ops(&mut f).is_empty(), "pauses before probing again");
                now += RETRY_DELAY_MS;
                f.update(now);
            }
        }
        assert_eq!(ops(&mut f), vec![Request::Leave]);
        assert_eq!(
            f.status(),
            RelayStatus::Failed {
                error: "online.no_relay".into()
            }
        );
    }

    #[test]
    fn a_probe_the_page_never_answers_counts_as_no_relay() {
        let mut f = Finder::new(Side::Join, "room", 0);
        let id = probe_id(&ops(&mut f)[0]);
        let late = PROBE_TIMEOUT_MS + PROBE_GRACE_MS;
        f.update(late);
        assert!(matches!(f.status(), RelayStatus::Retrying { .. }));
        f.update(late + RETRY_DELAY_MS);
        let again = probe_id(&ops(&mut f)[0]);
        assert_ne!(again, id);
        // The lost answer turns up late: ignored, it belongs to an older probe.
        f.probed(id, live(&["wss://a"]), late);
        assert!(ops(&mut f).is_empty());
        f.probed(again, live(&["wss://b"]), late);
        assert_eq!(
            ops(&mut f),
            vec![Request::JoinRoom {
                room: "room".into(),
                relays: live(&["wss://b"]),
            }]
        );
    }

    #[test]
    fn the_transport_losing_every_relay_triggers_a_fresh_probe() {
        let mut f = Finder::new(Side::Join, "room", 0);
        let id = probe_id(&ops(&mut f)[0]);
        f.probed(id, live(&["wss://a"]), 0);
        ops(&mut f);
        f.no_relay(10);
        f.update(10 + RETRY_DELAY_MS);
        let next = ops(&mut f);
        assert!(matches!(next[..], [Request::ProbeRelays { .. }]), "{next:?}");
    }

    #[test]
    fn a_lone_host_rechecks_and_reopens_only_when_the_live_set_changes() {
        let mut f = Finder::new(Side::Host, "room", 0);
        let id = probe_id(&ops(&mut f)[0]);
        f.probed(id, live(&["wss://a", "wss://b"]), 0);
        assert!(matches!(ops(&mut f)[..], [Request::HostRoom { .. }]));
        // A host never times out: it rechecks.
        let mut now = RECHECK_MS;
        f.update(now);
        let id = probe_id(&ops(&mut f)[0]);
        f.probed(id, live(&["wss://a", "wss://b"]), now);
        assert!(ops(&mut f).is_empty(), "same relays: the room stays open");
        now += RECHECK_MS;
        f.update(now);
        let id = probe_id(&ops(&mut f)[0]);
        f.probed(id, live(&["wss://b"]), now);
        assert_eq!(
            ops(&mut f),
            vec![Request::HostRoom {
                room: "room".into(),
                relays: live(&["wss://b"]),
            }]
        );
        for _ in 0..10 {
            now += RECHECK_MS;
            f.update(now);
            let id = probe_id(&ops(&mut f)[0]);
            f.probed(id, live(&["wss://b"]), now);
        }
        assert_eq!(f.status(), RelayStatus::Open { attempt: 1, live: 1 });
        f.connected(false);
        assert_eq!(f.status(), RelayStatus::Met, "any arrival is enough for a host");
    }

    #[test]
    fn the_probe_subscribes_to_an_ephemeral_kind() {
        let req: serde_json::Value = serde_json::from_str(&probe_req()).unwrap();
        assert_eq!(req[0], "REQ");
        let kind = req[2]["kinds"][0].as_u64().unwrap();
        assert!((20_000..30_000).contains(&kind), "NIP-16 ephemeral range");
    }
}
