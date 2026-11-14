//! The match as the online session runs it (D23, R1.403).
//!
//! [`NetMatch`] is a [`Play`] plus what every browser must agree on beside
//! it: the siege log for the replay (R1.1206). It implements
//! [`crate::online::Rollback`], so the session runs the real match from the
//! same start on every browser with only inputs on the wire.
//!
//! - Each seat sends at most one [`Input`] a tick, packed into a `u64` by
//!   [`pack`] (0 is no input). A browser with several inputs waiting sends
//!   one a tick.
//! - Bot castles are built inside the tick that enters the build phase, from
//!   match state only ([`step`], R1.1204), so every browser builds the same.
//!   Solo play runs through the same [`step`].
//! - A person whose browser left mid-match is played by a normal bot from
//!   the tick the host agreed (R1.1500).

use std::rc::Rc;

use super::siege::Field;
use super::units::GridPt;
use super::{Input, Play};
use crate::game::flow::{Difficulty, Phase, Seat, Settings};
use crate::game::ledger::{BuildingKind, Material, Piece};
use crate::game::{PlayerId, Refusal};
use crate::lobby::MatchStart;
use crate::online::{Rollback, SEATS};

/// A siege input that worked, at the siege tick it was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Logged {
    pub tick: u32,
    pub player: PlayerId,
    pub input: Input,
}

/// What the replay of a siege needs: every field as the siege began and the
/// people's siege inputs. Kept until the next siege begins.
#[derive(Clone, Debug, Default)]
pub struct SiegeLog {
    /// Shared, so a snapshot per tick costs a pointer.
    pub start: Option<Rc<Vec<Field>>>,
    pub inputs: Vec<Logged>,
}

impl SiegeLog {
    /// `owner`'s castle as the last siege began, and its attacker's inputs.
    pub fn replay_of(&self, owner: PlayerId) -> Option<(Field, Vec<Logged>)> {
        let field = self
            .start
            .as_ref()?
            .iter()
            .find(|f| f.owner == owner && !f.ghost)?
            .clone();
        let inputs = self
            .inputs
            .iter()
            .filter(|l| l.player == field.attacker)
            .copied()
            .collect();
        Some((field, inputs))
    }
}

/// Plays one logged input on a replayed field at siege tick `now`.
pub fn replay_input(field: &mut Field, input: Input, now: u32) {
    let _ = match input {
        Input::SelectAmmo(slot) => field.select(slot),
        Input::Swing => field.swing(now),
        Input::Release(part) => field.release_at(now, part),
        Input::Air(slot) => field.air(slot),
        _ => Ok(()),
    };
}

fn is_siege_input(i: Input) -> bool {
    matches!(
        i,
        Input::SelectAmmo(_) | Input::Swing | Input::Release(_) | Input::Air(_)
    )
}

/// One match tick with these inputs, then what follows inside the same tick:
/// the siege log, and the bots' castles when a build phase begins.
pub fn step(play: &mut Play, log: &mut SiegeLog, inputs: &[(PlayerId, Input)]) -> Vec<Result<(), Refusal>> {
    let before = play.rules.phase;
    let round = play.rules.round;
    let siege_tick = play.siege.as_ref().map(|s| s.tick);
    let results = play.step(inputs);
    if let Some(t) = siege_tick {
        for ((p, i), r) in inputs.iter().zip(results.iter()) {
            if r.is_ok() && is_siege_input(*i) {
                log.inputs.push(Logged {
                    tick: t,
                    player: *p,
                    input: *i,
                });
            }
        }
    }
    let phase = play.rules.phase;
    if phase == Phase::Siege && before != Phase::Siege {
        log.start = play.siege.as_ref().map(|s| Rc::new(s.fields.clone()));
        log.inputs.clear();
    }
    if phase == Phase::Build && (before != Phase::Build || round != play.rules.round) {
        bots_build(play);
    }
    results
}

/// A match from the lobby's start, round 1 begun and the bots' castles built.
pub fn new_play(start: &MatchStart) -> Result<Play, Refusal> {
    let seats = start
        .seats
        .iter()
        .map(|st| match st.bot {
            Some(d) => Seat::Bot(match d {
                crate::lobby::Difficulty::Easy => Difficulty::Easy,
                crate::lobby::Difficulty::Normal => Difficulty::Normal,
                crate::lobby::Difficulty::Hard => Difficulty::Hard,
            }),
            None => Seat::Human,
        })
        .collect();
    let mut play = Play::new(Settings {
        seats,
        wind: start.options.wind,
        seed: start.seed,
        flat: start.options.flat,
    })?;
    play.start()?;
    play.set_shot_planner(crate::ai::planner::plan_siege_job);
    bots_build(&mut play);
    Ok(play)
}

/// Bot castles for this build phase: the castle AI grows each bot's castle
/// and stands its royals (`ai::build::bot_turn`, R1.1451), placed through
/// `play::Input`. Seeded from the match only, so every browser builds the
/// same castles.
pub fn bots_build(play: &mut Play) {
    if play.rules.phase != Phase::Build {
        return;
    }
    let bots: Vec<PlayerId> = play
        .rules
        .players
        .iter()
        .filter(|p| p.alive() && matches!(p.seat, Seat::Bot(_)))
        .map(|p| p.id)
        .collect();
    for b in bots {
        for input in crate::ai::build::bot_turn(play, b) {
            let _ = play.apply(b, input);
        }
    }
}

// ---- packing an input into a seat's u64 ----

/// Bits per grid coordinate; a point outside 0..2^13 cannot be sent.
const COORD_BITS: u32 = 13;
const COORD_MAX: i32 = (1 << COORD_BITS) - 1;

fn coord(v: i32) -> Option<u64> {
    (0..=COORD_MAX).contains(&v).then_some(v as u64)
}

fn point(p: GridPt) -> Option<u64> {
    Some(coord(p.x)? | coord(p.y)? << COORD_BITS)
}

fn unpoint(v: u64) -> GridPt {
    let m = COORD_MAX as u64;
    GridPt::new((v & m) as i32, (v >> COORD_BITS & m) as i32)
}

fn piece_code(p: Piece) -> u64 {
    match p {
        Piece::Segment(Material::Wood) => 0,
        Piece::Segment(Material::Stone) => 1,
        Piece::Segment(Material::Metal) => 2,
        Piece::Rope => 3,
        Piece::Building(BuildingKind::Carpenter) => 4,
        Piece::Building(BuildingKind::Quarry) => 5,
        Piece::Building(BuildingKind::Mine) => 6,
        Piece::Building(BuildingKind::Armory) => 7,
    }
}

fn piece_of(c: u64) -> Piece {
    match c & 7 {
        0 => Piece::Segment(Material::Wood),
        1 => Piece::Segment(Material::Stone),
        2 => Piece::Segment(Material::Metal),
        3 => Piece::Rope,
        4 => Piece::Building(BuildingKind::Carpenter),
        5 => Piece::Building(BuildingKind::Quarry),
        6 => Piece::Building(BuildingKind::Mine),
        _ => Piece::Building(BuildingKind::Armory),
    }
}

fn kind_of(c: u64) -> Option<BuildingKind> {
    match piece_of(c) {
        Piece::Building(k) => Some(k),
        _ => None,
    }
}

/// One input as a seat's number for a tick: a 4-bit tag, then its fields.
/// Never 0 and never [`NetMatch::NOBODY`]; None for a point too far out to
/// send (the match would refuse it anyway).
pub fn pack(i: Input) -> Option<u64> {
    let (tag, body): (u64, u64) = match i {
        Input::Pick(s) => (1, s as u64),
        Input::Reroll => (2, 0),
        Input::PlaceSegment { piece, a, b } => (3, piece_code(piece) | point(a)? << 3 | point(b)? << 29),
        Input::PlaceBuilding { kind, at } => (4, piece_code(Piece::Building(kind)) | point(at)? << 3),
        Input::Sell(id) => (5, id as u64),
        Input::PlaceRoyal { index, at } => (6, index as u64 | point(at)? << 8),
        Input::BuyXp => (7, 0),
        Input::BuyShot => (8, 0),
        Input::SelectAmmo(s) => (9, s as u64),
        Input::Swing => (10, 0),
        Input::Release(part) => (11, part.min(31) as u64),
        Input::Air(s) => (12, s as u64),
        Input::Ready => (13, 0),
        Input::Upgrade { id, piece } => (14, piece_code(piece) | (id as u64) << 3),
    };
    Some(tag | body << 4)
}

/// The input a seat's number stands for; None for 0 or anything [`pack`]
/// never makes.
pub fn unpack(k: u64) -> Option<Input> {
    let b = k >> 4;
    let byte = (b & 0xff) as u8;
    Some(match k & 15 {
        1 => Input::Pick(byte),
        2 => Input::Reroll,
        3 => Input::PlaceSegment {
            piece: match piece_of(b) {
                p @ (Piece::Segment(_) | Piece::Rope) => p,
                Piece::Building(_) => return None,
            },
            a: unpoint(b >> 3),
            b: unpoint(b >> 29),
        },
        4 => Input::PlaceBuilding {
            kind: kind_of(b)?,
            at: unpoint(b >> 3),
        },
        5 => Input::Sell(u32::try_from(b).ok()?),
        6 => Input::PlaceRoyal {
            index: byte,
            at: unpoint(b >> 8),
        },
        7 => Input::BuyXp,
        8 => Input::BuyShot,
        9 => Input::SelectAmmo(byte),
        10 => Input::Swing,
        11 => Input::Release(byte.min(31)),
        12 => Input::Air(byte),
        13 => Input::Ready,
        14 => Input::Upgrade {
            id: u32::try_from(b >> 3).ok()?,
            piece: match piece_of(b) {
                p @ Piece::Segment(_) => p,
                _ => return None,
            },
        },
        _ => return None,
    })
}

/// The match one browser runs online.
#[derive(Clone, Debug)]
pub struct NetMatch {
    pub play: Play,
    pub log: SiegeLog,
    /// Each session seat's player, for the seats a person holds.
    seat_player: [Option<PlayerId>; SEATS],
}

impl NetMatch {
    pub fn new(start: &MatchStart) -> Result<NetMatch, Refusal> {
        let mut seat_player = [None; SEATS];
        for (p, st) in start.seats.iter().enumerate() {
            if st.bot.is_none() {
                *seat_player.get_mut(st.seat as usize).ok_or(Refusal::Unknown)? = Some(p as PlayerId);
            }
        }
        Ok(NetMatch {
            play: new_play(start)?,
            log: SiegeLog::default(),
            seat_player,
        })
    }

    /// The match player a session seat plays.
    pub fn player_of(&self, seat: usize) -> Option<PlayerId> {
        self.seat_player.get(seat).copied().flatten()
    }
}

impl Rollback for NetMatch {
    /// A seat with no person: no input, and a person's seat from the tick
    /// its browser left, which a bot then plays.
    const NOBODY: u64 = u64::MAX;

    fn guess(_last: u64) -> u64 {
        0
    }

    fn step(&mut self, keys: [u64; SEATS]) {
        let mut inputs = Vec::new();
        for (seat, k) in keys.iter().enumerate() {
            let Some(p) = self.player_of(seat) else { continue };
            if *k == Self::NOBODY {
                if let Some(pl) = self.play.rules.players.get_mut(p as usize) {
                    if pl.seat == Seat::Human {
                        pl.seat = Seat::Bot(Difficulty::Normal);
                    }
                }
            } else if let Some(i) = unpack(*k) {
                inputs.push((p, i));
            }
        }
        step(&mut self.play, &mut self.log, &inputs);
    }

    /// The match's checksum. String-heavy (R1.704); the session calls it
    /// once every `SUM_EVERY` confirmed ticks.
    fn sum(&self) -> u64 {
        self.play.checksum()
    }
}

#[cfg(test)]
#[path = "online_tests.rs"]
mod tests;
