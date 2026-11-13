//! The glue that makes a real match run: the rules (`game`), each player's
//! castle as physics (`physics`) on the seeded ground (`terrain`).
//!
//! Everything advances by inputs and ticks only: `Play::step(inputs)` applies
//! each player's inputs in order, then advances one tick, so every machine
//! fed the same inputs reaches the same `checksum()`. API summary for the
//! page: `notes/play-api.md`.
//!
//! - `units`: the one grid shared by terrain, physics and rules (R1.700).
//! - `castle`: ledger pieces and where they are, built into a physics world.
//! - `siege`: a field per castle under attack, trebuchets, shots.

pub mod castle;
pub mod siege;
pub mod tune;
pub mod units;

#[cfg(test)]
mod tests;

use castle::{Castle, Geo};
use siege::{Field, Shooter, ShotPlanner, Siege, SiegeView};
use units::GridPt;

use crate::game::flow::{Match, Phase, Seat, Settings, SiegeResult};
use crate::game::ledger::{BuildingKind, Piece};
use crate::game::pairing::Pairing;
use crate::game::{PlayerId, Refusal};

/// Everything a player can do, in any phase. One enum, so online play sends
/// one kind of message (R1.704).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    // Upgrade
    /// Take card `slot` of the offer.
    Pick(u8),
    Reroll,
    // Build
    /// A wood, stone or metal segment, or a rope, from `a` to `b`.
    PlaceSegment {
        piece: Piece,
        a: GridPt,
        b: GridPt,
    },
    /// A building with its feet at `at`, on a floor or the ground.
    PlaceBuilding {
        kind: BuildingKind,
        at: GridPt,
    },
    /// Sell a ledger entry (a segment, rope, building or ruin).
    Sell(u32),
    /// Royal `index` stands with its feet at `at`, on a floor or the ground.
    PlaceRoyal {
        index: u8,
        at: GridPt,
    },
    BuyXp,
    BuyShot,
    // Siege
    /// Choose the ammo bar slot (0 is normal shots).
    SelectAmmo(u8),
    /// Start the swing.
    Swing,
    /// Let go: the ticks since `Swing` set the arc.
    Release,
    /// Trigger owned air effect `slot` on the shot in flight.
    Air(u8),
    // Any timed phase
    Ready,
}

/// A whole match: rules, castles, and the siege while it runs.
#[derive(Clone, Debug)]
pub struct Play {
    pub rules: Match,
    pub castles: Vec<Castle>,
    pub siege: Option<Siege>,
    /// The last siege's outcome, kept for the replay and the page.
    pub last_result: Option<SiegeResult>,
    planner: ShotPlanner,
}

impl Play {
    /// A match in its lobby phase; `start` begins round 1.
    pub fn new(settings: Settings) -> Result<Play, Refusal> {
        let rules = Match::new(settings)?;
        let castles = vec![Castle::default(); rules.players.len()];
        Ok(Play {
            rules,
            castles,
            siege: None,
            last_result: None,
            planner: siege::simple_planner,
        })
    }

    pub fn start(&mut self) -> Result<(), Refusal> {
        self.rules.start()?;
        self.refresh_all();
        Ok(())
    }

    /// How bot trebuchets choose their shots (the aiming AI's hook).
    pub fn set_shot_planner(&mut self, planner: ShotPlanner) {
        self.planner = planner;
    }

    /// Apply this tick's inputs in order, then advance one tick. Returns the
    /// refusal of each input, in order (`Ok` for one that worked).
    pub fn step(&mut self, inputs: &[(PlayerId, Input)]) -> Vec<Result<(), Refusal>> {
        let out = inputs.iter().map(|(p, i)| self.apply(*p, *i)).collect();
        self.tick();
        out
    }

    /// One player input, now.
    pub fn apply(&mut self, p: PlayerId, input: Input) -> Result<(), Refusal> {
        let r = &mut self.rules;
        match input {
            Input::Pick(slot) => r.pick(p, slot as usize),
            Input::Reroll => r.reroll(p),
            Input::BuyXp => {
                r.buy_xp(p)?;
                self.refresh(p);
                Ok(())
            }
            Input::BuyShot => r.buy_shot(p),
            Input::Ready => r.set_ready(p),
            Input::PlaceSegment { piece, a, b } => self.place_segment(p, piece, a, b),
            Input::PlaceBuilding { kind, at } => self.place_building(p, kind, at),
            Input::Sell(id) => {
                r.sell(p, id)?;
                self.castles[p as usize].geo.remove(&id);
                self.refresh(p);
                Ok(())
            }
            Input::PlaceRoyal { index, at } => self.place_royal(p, index as usize, at),
            Input::SelectAmmo(slot) => self.field_of(p)?.select(slot),
            Input::Swing => {
                self.need_live_siege()?;
                let now = self.siege_tick();
                self.field_of(p)?.swing(now)
            }
            Input::Release => {
                self.need_live_siege()?;
                let now = self.siege_tick();
                self.field_of(p)?.release(now)
            }
            Input::Air(slot) => self.field_of(p)?.air(slot),
        }
    }

    fn need_live_siege(&self) -> Result<(), Refusal> {
        if self.rules.phase == Phase::Siege && self.siege.is_some() {
            Ok(())
        } else {
            Err(Refusal::WrongPhase)
        }
    }

    fn siege_tick(&self) -> u32 {
        self.siege.as_ref().map_or(0, |s| s.tick)
    }

    /// The field `p` shoots at, if they drive one this siege.
    fn field_of(&mut self, p: PlayerId) -> Result<&mut Field, Refusal> {
        self.siege
            .as_mut()
            .ok_or(Refusal::WrongPhase)?
            .fields
            .iter_mut()
            .find(|f| f.attacker == p && !f.shooter.bot)
            .ok_or(Refusal::WrongPhase)
    }

    fn place_segment(&mut self, p: PlayerId, piece: Piece, a: GridPt, b: GridPt) -> Result<(), Refusal> {
        let m = castle::physics_material(piece).ok_or(Refusal::BadSpot)?;
        let pl = self.rules.player(p).ok_or(Refusal::Unknown)?;
        if !castle::segment_fits(&pl.terrain, pl.purse.level, a, b, m) {
            return Err(Refusal::BadSpot);
        }
        let dup = self.castles[p as usize]
            .geo
            .values()
            .any(|g| matches!(*g, Geo::Beam { a: x, b: y } if (x == a && y == b) || (x == b && y == a)));
        if dup {
            return Err(Refusal::BadSpot);
        }
        let id = self.rules.place(p, piece)?;
        self.castles[p as usize].geo.insert(id, Geo::Beam { a, b });
        self.refresh(p);
        Ok(())
    }

    fn place_building(&mut self, p: PlayerId, kind: BuildingKind, at: GridPt) -> Result<(), Refusal> {
        let pl = self.rules.player(p).ok_or(Refusal::Unknown)?;
        if !castle::point_fits(&pl.terrain, pl.purse.level, at) {
            return Err(Refusal::BadSpot);
        }
        let cw = self.castles[p as usize].world(pl, false);
        if castle::surface_at(&cw.world, at.to_fx()).is_none() {
            return Err(Refusal::BadSpot);
        }
        let id = self.rules.place_at(p, Piece::Building(kind), at.col())?;
        self.castles[p as usize].geo.insert(id, Geo::Building { at });
        Ok(())
    }

    fn place_royal(&mut self, p: PlayerId, index: usize, at: GridPt) -> Result<(), Refusal> {
        let pl = self.rules.player(p).ok_or(Refusal::Unknown)?;
        if !castle::point_fits(&pl.terrain, pl.purse.level, at) {
            return Err(Refusal::BadSpot);
        }
        let cw = self.castles[p as usize].world(pl, false);
        if castle::surface_at(&cw.world, at.to_fx()).is_none() {
            return Err(Refusal::BadSpot);
        }
        let spot = castle::rule_spot(&pl.terrain, pl.purse.level, at);
        self.rules.place_royal(p, index, spot)?;
        let c = &mut self.castles[p as usize];
        if c.royal_at.len() <= index {
            c.royal_at.resize(index + 1, None);
        }
        c.royal_at[index] = Some(at);
        Ok(())
    }

    /// Tell the rules where `p`'s royals may stand, from the castle as built.
    fn refresh(&mut self, p: PlayerId) {
        let Some(pl) = self.rules.player(p) else { return };
        if !pl.alive() {
            return;
        }
        let cw = self.castles[p as usize].world(pl, false);
        let spots = castle::standing_spots(&cw.world, &pl.terrain, pl.purse.level);
        let _ = self.rules.set_standing_spots(p, spots);
    }

    fn refresh_all(&mut self) {
        for p in 0..self.rules.players.len() {
            self.refresh(p as PlayerId);
        }
    }

    /// Advance one tick: the siege's physics, then the rules' clock.
    pub fn tick(&mut self) {
        let before = self.rules.phase;
        let round = self.rules.round;
        if let Some(s) = self.siege.as_mut() {
            let live = self.rules.phase == Phase::Siege;
            for f in s.fields.iter_mut() {
                if live {
                    f.run_plan(s.tick);
                }
                f.step();
            }
            s.tick += 1;
            if live {
                let done: Vec<PlayerId> = s
                    .fields
                    .iter()
                    .filter(|f| f.bot_done(s.tick))
                    .map(|f| f.attacker)
                    .collect();
                for a in done {
                    if matches!(self.rules.player(a).map(|pl| pl.seat), Some(Seat::Bot(_))) {
                        let _ = self.rules.set_ready(a);
                    }
                }
            }
        }
        self.rules.tick();
        if before == Phase::Build && self.rules.phase == Phase::Siege {
            self.begin_siege();
        }
        if self.rules.phase == Phase::SiegeOver {
            self.drain_or_finish();
        }
        if self.rules.round != round || (before != self.rules.phase && self.rules.phase == Phase::Build) {
            self.refresh_all();
        }
    }

    fn drain_or_finish(&mut self) {
        let Some(s) = self.siege.as_mut() else { return };
        if s.drain_left > 0 && s.fields.iter().any(|f| f.active()) {
            s.drain_left -= 1;
            return;
        }
        let result = SiegeResult {
            castles: s
                .fields
                .iter()
                .map(|f| f.outcome(&self.rules.players[f.owner as usize].royals))
                .collect(),
        };
        self.siege = None;
        let _ = self.rules.apply_siege(&result);
        self.last_result = Some(result);
    }

    /// One field per castle under attack (R1.205 for ghosts).
    fn begin_siege(&mut self) {
        let wind = siege::physics_wind(self.rules.wind);
        // (owner, attacker, ghost, shooter is a stand-in bot)
        let mut plan: Vec<(PlayerId, PlayerId, bool, bool)> = Vec::new();
        for pr in &self.rules.pairings {
            match *pr {
                Pairing::Duel(a, b) => {
                    plan.push((b, a, false, false));
                    plan.push((a, b, false, false));
                }
                Pairing::Ghost { attacker, copy_of } => {
                    plan.push((copy_of, attacker, true, false));
                    plan.push((attacker, copy_of, false, true));
                }
            }
        }
        let mut fields = Vec::new();
        for (owner, attacker, ghost, stand_in) in plan {
            let o = &self.rules.players[owner as usize];
            let a = &self.rules.players[attacker as usize];
            let mut cw = self.castles[owner as usize].world(o, true);
            cw.world.wind = wind;
            let difficulty = match a.seat {
                Seat::Bot(d) => Some(d),
                Seat::Human => None,
            };
            let bot = stand_in || difficulty.is_some();
            let mut shooter = Shooter::new(attacker, bot, &a.loadout());
            let base = units::trebuchet_base(&o.terrain);
            if bot {
                let view = SiegeView {
                    world: &cw.world,
                    plot: &o.terrain,
                    base,
                    power: shooter.power,
                    ammo: &shooter.ammo,
                    air: &shooter.air,
                    difficulty: if stand_in { None } else { difficulty },
                };
                shooter.plan = (self.planner)(&view);
            }
            fields.push(Field {
                owner,
                attacker,
                ghost,
                castle: cw,
                shooter,
                base,
                plot: o.terrain.clone(),
            });
        }
        self.siege = Some(Siege {
            fields,
            tick: 0,
            drain_left: tune::DRAIN_TICKS,
        });
    }

    /// Per-segment strain of `p`'s castle under its own weight, by ledger
    /// id, in thousandths of the breaking point (0..=1000), highest piece
    /// of a split ruin. Runs a static test; call it when the castle changes.
    pub fn strain(&self, p: PlayerId) -> Vec<(u32, u32)> {
        let Some(pl) = self.rules.player(p) else {
            return Vec::new();
        };
        let cw = self.castles[p as usize].world(pl, false);
        let report = cw
            .world
            .static_test(crate::physics::TUNE.settle_ticks + tune::STATIC_TEST_TICKS);
        let mut out: Vec<(u32, u32)> = Vec::new();
        for (i, id) in cw.seg_ids.iter().enumerate() {
            let broke = report.broken.contains(&i);
            let s = report.max_strain.get(i).map_or(0, |f| f.0.max(0));
            let permille = if broke {
                1000
            } else {
                (s * 1000 >> 16).min(1000) as u32
            };
            match out.iter_mut().find(|(k, _)| k == id) {
                Some(e) => e.1 = e.1.max(permille),
                None => out.push((*id, permille)),
            }
        }
        out
    }

    /// The physics world of `p`'s castle as it stands (with royals), for the
    /// page to draw and for the Test button to step.
    pub fn castle_world(&self, p: PlayerId) -> Option<crate::physics::World> {
        let pl = self.rules.player(p)?;
        Some(self.castles[p as usize].world(pl, true).world)
    }

    /// A fingerprint of the whole match, for online desync checks.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for b in bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        mix(format!("{:?}", self.rules).as_bytes());
        mix(format!("{:?}", self.castles).as_bytes());
        if let Some(s) = &self.siege {
            mix(&s.tick.to_le_bytes());
            for f in &s.fields {
                mix(&f.castle.world.checksum().to_le_bytes());
                mix(format!("{:?}", f.shooter).as_bytes());
            }
        }
        h
    }
}
