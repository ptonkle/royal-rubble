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
pub mod online;
pub mod siege;
pub mod tune;
pub mod units;

#[cfg(test)]
mod tests;

use castle::{Castle, Geo};
use siege::{Field, Shooter, ShotPlanner, Siege, SiegeView};
use units::GridPt;

use crate::game::flow::{Match, Phase, Player, Seat, Settings, SiegeResult};
use crate::game::ledger::{BuildingKind, Piece};
use crate::game::pairing::Pairing;
use crate::game::{PlayerId, Refusal};

/// `Input::Release` on the tick it is applied, with no part of a tick
/// (R1.2103).
pub const ON_TICK: u8 = 16;

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
    /// Let go: the time since `Swing` sets the arc. The number is when,
    /// in sixteenths of a tick from the start of the tick before this one
    /// (0-31; [`ON_TICK`] is this tick exactly, R1.2103).
    Release(u8),
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
    /// Bot shot planning begun during the build, against the castles as
    /// they stand (R1.2800).
    pre: Vec<PrePlan>,
    /// Inputs each player has had accepted this build; planning against a
    /// castle (or with a loadout) that changed since starts again.
    edits: Vec<u32>,
}

/// A bot's siege shots, planned during the build.
#[derive(Clone, Debug)]
struct PrePlan {
    owner: PlayerId,
    attacker: PlayerId,
    /// `edits` of the owner and the attacker when planning began.
    edits: (u32, u32),
    /// Fingerprint of everything the planner was shown (`PlanSetup::key`).
    key: u64,
    job: Box<dyn siege::PlanJob>,
}

/// One field's start: who shoots at whom, from where, with what.
struct PlanSetup {
    cw: castle::CastleWorld,
    royals: Vec<(usize, u32)>,
    shooter: Shooter,
    base: crate::physics::Vec2,
    difficulty: Option<crate::game::flow::Difficulty>,
}

impl PlanSetup {
    fn view<'a>(&'a self, plot: &'a crate::terrain::Plot) -> SiegeView<'a> {
        SiegeView {
            royals: &self.royals,
            world: &self.cw.world,
            plot,
            base: self.base,
            power: self.shooter.power,
            ammo: &self.shooter.ammo,
            air: &self.shooter.air,
            difficulty: self.difficulty,
        }
    }

    /// A fingerprint of what the planner sees, so planning begun during the
    /// build is kept only when the siege starts from the same thing.
    fn key(&self) -> u64 {
        let mut h: u64 = self.cw.world.checksum();
        let text = format!(
            "{:?}",
            (
                &self.royals,
                self.base,
                self.shooter.power,
                &self.shooter.ammo,
                &self.shooter.air,
                self.difficulty
            )
        );
        for b in text.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
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
            planner: siege::simple_job,
            pre: Vec::new(),
            edits: Vec::new(),
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
        let build = self.rules.phase == Phase::Build;
        let r = self.apply_input(p, input);
        if build && r.is_ok() && input != Input::Ready {
            if self.edits.len() <= p as usize {
                self.edits.resize(p as usize + 1, 0);
            }
            self.edits[p as usize] += 1;
        }
        r
    }

    fn apply_input(&mut self, p: PlayerId, input: Input) -> Result<(), Refusal> {
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
            Input::Release(part) => {
                self.need_live_siege()?;
                let now = self.siege_tick();
                self.field_of(p)?.release_at(now, part)
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
        // one royal to a room, two with Close Quarters (R1.3706)
        if let Some(room) = castle::room_at(&cw.world, at.to_fx()) {
            let c = &self.castles[p as usize];
            let mates = (0..pl.royals.len())
                .filter(|&i| i != index)
                .filter_map(|i| c.royal_at.get(i).copied().flatten())
                .filter(|q| castle::room_at(&cw.world, q.to_fx()) == Some(room))
                .count();
            if mates as u32 >= pl.mods.royal_spacing.unwrap_or(1) {
                return Err(Refusal::BadSpot);
            }
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
        if before == Phase::Build {
            self.preplan();
        }
        if let Some(s) = self.siege.as_mut() {
            let live = self.rules.phase == Phase::Siege;
            if live {
                let mut jobs: Vec<&mut (dyn siege::PlanJob + 'static)> =
                    s.fields.iter_mut().filter_map(|f| f.job.as_deref_mut()).collect();
                plan_slice(&mut jobs, tune::BOT_PLAN_BUDGET);
            }
            for f in s.fields.iter_mut() {
                if live {
                    f.take_planned(s.tick);
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
        if self.rules.phase != Phase::Build {
            self.pre.clear();
            self.edits.clear();
        }
    }

    /// Who shoots at whom this round: (owner, attacker, ghost, the shooter
    /// is a stand-in bot), one per field (R1.205 for ghosts).
    fn field_plan(&self) -> Vec<(PlayerId, PlayerId, bool, bool)> {
        let mut plan = Vec::new();
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
        plan
    }

    /// The field `a` shoots at `owner`'s castle in, from the players as
    /// given (the live ones at the siege start, `as_at_siege` copies during
    /// the build).
    fn setup(&self, owner: PlayerId, o: &Player, a: &Player, stand_in: bool) -> PlanSetup {
        let mut cw = self.castles[owner as usize].world(o, true);
        cw.world.wind = siege::physics_wind(self.rules.wind);
        let difficulty = match a.seat {
            Seat::Bot(d) => Some(d),
            Seat::Human => None,
        };
        let bot = stand_in || difficulty.is_some();
        let shooter = Shooter::new(a.id, bot, &a.loadout(), &a.mods);
        let royals = cw
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(body, who)| match who {
                castle::BodyOwner::Royal(i) => o.royals.get(*i).map(|r| (body, r.kind.base_damage())),
                castle::BodyOwner::Building(_) => None,
            })
            .collect();
        PlanSetup {
            cw,
            royals,
            shooter,
            base: units::trebuchet_base(&o.terrain),
            difficulty: if stand_in { None } else { difficulty },
        }
    }

    fn edits_of(&self, p: PlayerId) -> u32 {
        self.edits.get(p as usize).copied().unwrap_or(0)
    }

    /// One build tick of bot shot planning (R1.2800): each bot field's
    /// planning starts (again) from its castle as it now stands whenever
    /// its owner or attacker has had an input accepted since, then the jobs
    /// share `BOT_PLAN_BUDGET` as in the siege. Only accepted inputs and
    /// the clock drive it, so every browser plans the same shots.
    fn preplan(&mut self) {
        let mut kept = Vec::new();
        for (owner, attacker, _, stand_in) in self.field_plan() {
            let Some(a) = self.rules.player(attacker) else {
                continue;
            };
            if !(stand_in || matches!(a.seat, Seat::Bot(_))) {
                continue;
            }
            let edits = (self.edits_of(owner), self.edits_of(attacker));
            let same = self
                .pre
                .iter()
                .position(|p| p.owner == owner && p.attacker == attacker && p.edits == edits);
            if let Some(i) = same {
                kept.push(self.pre.swap_remove(i));
                continue;
            }
            let (Some(o), Some(a)) = (self.rules.as_at_siege(owner), self.rules.as_at_siege(attacker)) else {
                continue;
            };
            let st = self.setup(owner, &o, &a, stand_in);
            kept.push(PrePlan {
                owner,
                attacker,
                edits,
                key: st.key(),
                job: (self.planner)(&st.view(&o.terrain)),
            });
        }
        self.pre = kept;
        let mut jobs: Vec<&mut (dyn siege::PlanJob + 'static)> =
            self.pre.iter_mut().map(|p| p.job.as_mut()).collect();
        plan_slice(&mut jobs, tune::BOT_PLAN_BUDGET);
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

    /// One field per castle under attack (R1.205 for ghosts). A bot keeps
    /// the planning it began during the build when the siege starts from
    /// exactly what that planning saw, and starts again otherwise (R1.2800).
    fn begin_siege(&mut self) {
        let mut pre = std::mem::take(&mut self.pre);
        let mut fields = Vec::new();
        for (owner, attacker, ghost, stand_in) in self.field_plan() {
            let o = &self.rules.players[owner as usize];
            let a = &self.rules.players[attacker as usize];
            let st = self.setup(owner, o, a, stand_in);
            let mut job = None;
            if st.shooter.bot {
                let key = st.key();
                let kept = pre
                    .iter()
                    .position(|p| p.owner == owner && p.attacker == attacker && p.key == key)
                    .map(|i| pre.swap_remove(i).job);
                job = Some(kept.unwrap_or_else(|| (self.planner)(&st.view(&o.terrain))));
            }
            let lives = siege::body_lives(&st.cw.bodies, o);
            fields.push(Field {
                owner,
                attacker,
                ghost,
                castle: st.cw,
                shooter: st.shooter,
                base: st.base,
                plot: o.terrain.clone(),
                fuses: Vec::new(),
                lives,
                killed: Vec::new(),
                job,
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
                ((s * 1000) >> 16).min(1000) as u32
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

/// One tick of bot shot planning across every field (R1.2000): each unit
/// of work goes to the bot with the fewest shots planned (the first such
/// field on a tie) whose next unit fits in what is left of `budget`, so
/// first shots come one bot after another rather than all at the end.
/// The budget is in work units, never clock time, so the slices are the
/// same on every machine. Returns the units spent.
pub fn plan_slice(jobs: &mut [&mut (dyn siege::PlanJob + 'static)], budget: u64) -> u64 {
    let mut left = budget;
    loop {
        let next = jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| {
                let need = j.next_cost();
                need > 0 && need <= left
            })
            .min_by_key(|(i, j)| (j.planned(), *i))
            .map(|(i, _)| i);
        let Some(i) = next else {
            return budget - left;
        };
        left -= jobs[i].advance().min(left);
    }
}
