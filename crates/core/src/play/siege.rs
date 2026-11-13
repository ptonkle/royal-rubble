//! The siege: one physics field per castle under attack, a trebuchet per
//! field, and the shots each shooter has left this round.

use super::castle::{BodyOwner, CastleWorld};
use super::tune;
use super::units;
use crate::game::flow::{Difficulty, Loadout};
use crate::game::upgrades::{AirKind, AmmoKind, Modifiers};
use crate::game::{CastleOutcome, PlayerId, Refusal};
use crate::physics::{trebuchet_launch_at, AirEffect, Event, Fuse, Fx, ShotKind, ShotMods, Vec2, World};
use crate::terrain;

/// One kind of shot in a shooter's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ammo {
    Normal,
    Special(AmmoKind),
}

impl Ammo {
    /// The physics shot kind (`physics::ammo::SHOTS` holds its numbers).
    pub fn kind(self) -> ShotKind {
        match self {
            Ammo::Normal => ShotKind::Normal,
            Ammo::Special(AmmoKind::Heavy) => ShotKind::Heavy,
            Ammo::Special(AmmoKind::Fire) => ShotKind::Fire,
            Ammo::Special(AmmoKind::Exploding) => ShotKind::Exploding,
            Ammo::Special(AmmoKind::Spread) => ShotKind::Spread,
            Ammo::Special(AmmoKind::Chain) => ShotKind::Chain,
            Ammo::Special(AmmoKind::Meteor) => ShotKind::Meteor,
        }
    }
}

pub fn air_effect(k: AirKind) -> AirEffect {
    match k {
        AirKind::Drop => AirEffect::Drop,
        AirKind::Reverse => AirEffect::Reverse,
        AirKind::Phase => AirEffect::Phase,
        AirKind::Split => AirEffect::Split,
        AirKind::Dive => AirEffect::Dive,
        AirKind::Boost => AirEffect::Boost,
        AirKind::Hover => AirEffect::Hover,
    }
}

/// One shot a bot trebuchet will take: ticks count from the siege start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedShot {
    /// Slot in the shooter's ammo bar.
    pub ammo: u8,
    pub swing_at: u32,
    /// Ticks from swing start to release (the arc).
    pub release_after: u32,
    /// Sixteenths of a tick past that the release is timed (R1.2100).
    pub release_frac: u8,
    /// An air effect slot and the ticks after release to trigger it.
    pub air: Option<(u8, u32)>,
}

/// What a shot planner sees: the target world before the first tick.
pub struct SiegeView<'a> {
    pub world: &'a World,
    pub plot: &'a terrain::Plot,
    pub base: Vec2,
    pub power: u8,
    pub ammo: &'a [(Ammo, u32)],
    pub air: &'a [AirKind],
    /// The seat's difficulty; `None` for a ghost's stand-in trebuchet.
    pub difficulty: Option<Difficulty>,
    /// Each royal's body index in `world` and its damage before round
    /// scaling, so the aimer can go for the King (R1.912).
    pub royals: &'a [(usize, u32)],
}

/// A bot trebuchet's shot planning, done a slice at a time so no tick
/// spends long on it (R1.2000). Work is counted in fixed units, never
/// clock time, so every browser of an online match plans the same shots
/// on the same ticks.
pub trait PlanJob: std::fmt::Debug + Send {
    /// The most the next `advance` can cost, in work units; 0 when done.
    fn next_cost(&self) -> u64;
    /// One slice of work; returns what it cost.
    fn advance(&mut self) -> u64;
    /// Shots finished since the last call, in firing order. `swing_at` is
    /// the earliest tick from the siege start to swing.
    fn take_ready(&mut self) -> Vec<PlannedShot>;
    /// Shots planned so far, taken or not.
    fn planned(&self) -> u32;
    fn box_clone(&self) -> Box<dyn PlanJob>;
}

impl Clone for Box<dyn PlanJob> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// A plan made all at once.
#[derive(Clone, Debug)]
pub struct ReadyPlan(pub Vec<PlannedShot>);

impl PlanJob for ReadyPlan {
    fn next_cost(&self) -> u64 {
        0
    }
    fn advance(&mut self) -> u64 {
        0
    }
    fn take_ready(&mut self) -> Vec<PlannedShot> {
        std::mem::take(&mut self.0)
    }
    fn planned(&self) -> u32 {
        0
    }
    fn box_clone(&self) -> Box<dyn PlanJob> {
        Box::new(self.clone())
    }
}

/// Starts a bot trebuchet's shot planning for a siege. The aiming AI plugs
/// in here (`Play::set_shot_planner`).
pub type ShotPlanner = fn(&SiegeView) -> Box<dyn PlanJob>;

/// The stand-in planner as a `ShotPlanner`.
pub fn simple_job(v: &SiegeView) -> Box<dyn PlanJob> {
    Box::new(ReadyPlan(simple_planner(v)))
}

/// The stand-in planner: every shot in bar order, same arc, evenly spaced.
pub fn simple_planner(v: &SiegeView) -> Vec<PlannedShot> {
    let mut out = Vec::new();
    let mut t = tune::BOT_FIRST_SWING;
    for (slot, (_, n)) in v.ammo.iter().enumerate() {
        for _ in 0..*n {
            out.push(PlannedShot {
                ammo: slot as u8,
                swing_at: t,
                release_after: tune::BOT_RELEASE_TICK,
                release_frac: 0,
                air: None,
            });
            t += tune::BOT_SHOT_EVERY;
        }
    }
    out
}

/// When a newly planned shot swings: at its planned tick, but never before
/// `now` (the tick it is ready) nor sooner than one shot's spacing after
/// the shot before it.
pub fn swing_tick(last: Option<&PlannedShot>, planned: u32, now: u32) -> u32 {
    let after_last = last.map_or(0, |l| l.swing_at + tune::BOT_SHOT_EVERY);
    planned.max(now).max(after_last)
}

/// A trebuchet and its shots for one siege.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shooter {
    /// Whose loadout it fires (the copied player for a ghost's stand-in).
    pub player: PlayerId,
    /// Driven by a plan, not by inputs.
    pub bot: bool,
    pub ammo: Vec<(Ammo, u32)>,
    pub air: Vec<AirKind>,
    /// Uses left this round of each entry in `air`.
    pub air_uses: Vec<u32>,
    pub selected: u8,
    pub power: u8,
    /// Siege tick the current swing started.
    pub swing_from: Option<u32>,
    /// The last throw: the swing tick it left at and the siege tick it left,
    /// for drawing the arm's follow-through.
    pub thrown: Option<(u32, u32)>,
    /// Projectiles of the last shot, and whether its air effect is used.
    pub live: Vec<usize>,
    pub air_used: bool,
    pub plan: Vec<PlannedShot>,
    /// The shooter's upgrades as they touch each shot.
    pub mods: ShotMods,
    /// Balls the first shot of the round fires (Second Volley), 1 without.
    pub first_copies: u32,
    /// Shots released this siege.
    pub fired: u32,
    /// Release tick and air trigger tick still to come for a planned shot.
    pending_air: Option<(u8, u32)>,
}

/// Physics launch power for a level's `shot_power_pct` (R1.703, R1.1101).
pub fn launch_power(pct: u32) -> u8 {
    let base = crate::physics::TUNE.treb_power_base;
    (base * pct as i64 / 100 - base).clamp(0, 255) as u8
}

impl Shooter {
    pub fn new(player: PlayerId, bot: bool, loadout: &Loadout, mods: &Modifiers) -> Shooter {
        let mut ammo = vec![(Ammo::Normal, loadout.normal)];
        ammo.extend(loadout.ammo.iter().map(|(k, n)| (Ammo::Special(*k), *n)));
        Shooter {
            player,
            bot,
            ammo,
            air: loadout.air.clone(),
            air_uses: loadout.air_uses.clone(),
            selected: 0,
            power: launch_power(loadout.power_pct),
            swing_from: None,
            thrown: None,
            live: Vec::new(),
            air_used: false,
            plan: Vec::new(),
            mods: ShotMods {
                weight_pct: mods.weight_pct,
                blast_pct: mods.blast_pct,
                ignite: mods.all_ignite,
            },
            first_copies: mods.first_shot_copies.max(1),
            fired: 0,
            pending_air: None,
        }
    }

    pub fn shots_left(&self) -> u32 {
        self.ammo.iter().map(|(_, n)| n).sum()
    }
}

/// One castle under attack.
#[derive(Clone, Debug)]
pub struct Field {
    pub owner: PlayerId,
    pub attacker: PlayerId,
    pub ghost: bool,
    pub castle: CastleWorld,
    pub shooter: Shooter,
    pub base: Vec2,
    pub plot: terrain::Plot,
    /// Exploding shots still in the air.
    pub fuses: Vec<Fuse>,
    /// Shot hits each body still survives this siege (Chainmail, Plate
    /// Armor), by body index.
    pub lives: Vec<u32>,
    /// Bodies of royals killed, in the order they died (R1.958).
    pub killed: Vec<usize>,
    /// A bot's shot planning still under way (R1.2000).
    pub job: Option<Box<dyn PlanJob>>,
}

impl Field {
    pub fn select(&mut self, slot: u8) -> Result<(), Refusal> {
        if (slot as usize) < self.shooter.ammo.len() && self.shooter.ammo[slot as usize].1 > 0 {
            self.shooter.selected = slot;
            Ok(())
        } else {
            Err(Refusal::Limit)
        }
    }

    pub fn swing(&mut self, now: u32) -> Result<(), Refusal> {
        let s = &mut self.shooter;
        if s.swing_from.is_some() || s.ammo.get(s.selected as usize).is_none_or(|a| a.1 == 0) {
            return Err(Refusal::Limit);
        }
        s.swing_from = Some(now);
        Ok(())
    }

    pub fn release(&mut self, now: u32) -> Result<(), Refusal> {
        self.release_at(now, 0)
    }

    /// Releases on tick `now`, timed `frac` sixteenths of a tick past it
    /// (R1.2100).
    pub fn release_at(&mut self, now: u32, frac: u8) -> Result<(), Refusal> {
        let from = self.shooter.swing_from.ok_or(Refusal::WrongPhase)?;
        let slot = self.shooter.selected as usize;
        let (ammo, left) = self.shooter.ammo[slot];
        if left == 0 {
            return Err(Refusal::Limit);
        }
        self.shooter.ammo[slot].1 -= 1;
        self.shooter.swing_from = None;
        self.shooter.thrown = Some((now - from, now));
        let parts = crate::physics::RELEASE_PARTS;
        let release = (now - from) * parts + (frac as u32).min(parts - 1);
        let launch = trebuchet_launch_at(self.base, release, self.shooter.power, false);
        let copies = if self.shooter.fired == 0 {
            self.shooter.first_copies
        } else {
            1
        };
        self.shooter.fired += 1;
        self.shooter.live.clear();
        for k in 0..copies as i64 {
            // each copy a little slower, so it lands a little short of the last
            let slow = 100 - tune::VOLLEY_SLOWER_PCT * k;
            let vel = Vec2::new(launch.vel.x * slow / 100, launch.vel.y * slow / 100);
            let shot = self.castle.world.fire_shot(
                crate::physics::Launch { pos: launch.pos, vel },
                ammo.kind(),
                self.shooter.mods,
            );
            self.shooter.live.extend(shot.balls);
            self.fuses.extend(shot.fuse);
        }
        self.shooter.air_used = false;
        // an empty slot moves the bar to the next one with shots
        if self.shooter.ammo[slot].1 == 0 {
            if let Some(k) = self.shooter.ammo.iter().position(|a| a.1 > 0) {
                self.shooter.selected = k as u8;
            }
        }
        Ok(())
    }

    pub fn air(&mut self, slot: u8) -> Result<(), Refusal> {
        let kind = *self.shooter.air.get(slot as usize).ok_or(Refusal::Locked)?;
        if self.shooter.air_used || self.shooter.air_uses.get(slot as usize).is_none_or(|n| *n == 0) {
            return Err(Refusal::Limit);
        }
        let w = &mut self.castle.world;
        let live: Vec<usize> = self
            .shooter
            .live
            .iter()
            .copied()
            .filter(|&i| w.projectiles.get(i).is_some_and(|b| b.alive))
            .collect();
        if live.is_empty() {
            return Err(Refusal::Limit);
        }
        let mut now_live = Vec::new();
        for i in live {
            now_live.extend(w.air_effect(i, air_effect(kind)));
        }
        self.shooter.live = now_live;
        self.shooter.air_used = true;
        self.shooter.air_uses[slot as usize] -= 1;
        Ok(())
    }

    /// Runs a bot's plan for siege tick `now`.
    pub fn run_plan(&mut self, now: u32) {
        if !self.shooter.bot {
            return;
        }
        if let Some((slot, at)) = self.shooter.pending_air {
            if at == now {
                let _ = self.air(slot);
                self.shooter.pending_air = None;
            }
        }
        let plan = self.shooter.plan.clone();
        for p in plan {
            if p.swing_at == now {
                let _ = self.select(p.ammo);
                let _ = self.swing(now);
            }
            if p.swing_at + p.release_after == now && self.release_at(now, p.release_frac).is_ok() {
                if let Some((slot, after)) = p.air {
                    self.shooter.pending_air = Some((slot, now + after));
                }
            }
        }
    }

    /// Moves the bot's newly planned shots into its plan. A shot never
    /// swings before now, nor sooner than one shot's spacing after the
    /// shot before it, so a late plan delays the bot and never loses a shot.
    pub fn take_planned(&mut self, now: u32) {
        let Some(job) = self.job.as_mut() else { return };
        for mut p in job.take_ready() {
            p.swing_at = swing_tick(self.shooter.plan.last(), p.swing_at, now);
            self.shooter.plan.push(p);
        }
    }

    /// Planning work still to do.
    pub fn planning(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j.next_cost() > 0)
    }

    /// One physics tick; shots in the water are spent.
    pub fn step(&mut self) -> Vec<Event> {
        let ev = self.castle.world.step();
        let blasts = self.castle.world.step_fuses(&mut self.fuses);
        let w = &mut self.castle.world;
        for b in w.projectiles.iter_mut() {
            if b.alive && units::in_water(&self.plot, b.pos) {
                b.alive = false;
            }
        }
        // a shot or blast hit on a body with a life left: it stands, the shot stops
        let hits = ev
            .iter()
            .filter_map(|e| match *e {
                Event::BodyHit { body, projectile } => Some((body, Some(projectile))),
                _ => None,
            })
            .chain(blasts.iter().flat_map(|b| b.bodies.iter().map(|&i| (i, None))));
        for (body, shot) in hits {
            if let Some(n) = self.lives.get_mut(body).filter(|n| **n > 0) {
                *n -= 1;
                w.bodies[body].alive = true;
                if let Some(p) = shot {
                    w.projectiles[p].alive = false;
                }
            }
        }
        for (i, owner) in self.castle.bodies.iter().enumerate() {
            if matches!(owner, BodyOwner::Royal(_)) && !w.bodies[i].alive && !self.killed.contains(&i) {
                self.killed.push(i);
            }
        }
        ev
    }

    /// Shots or debris still moving.
    pub fn active(&self) -> bool {
        let w = &self.castle.world;
        !self.fuses.is_empty() || w.projectiles.iter().chain(w.debris.iter()).any(|b| b.alive)
    }

    /// A bot is done when its plan is spent and nothing it threw still flies.
    pub fn bot_done(&self, now: u32) -> bool {
        self.shooter.bot
            && !self.planning()
            && self.shooter.pending_air.is_none()
            && self
                .shooter
                .plan
                .iter()
                .all(|p| p.swing_at + p.release_after < now)
            && !self.active()
    }

    /// Royals killed and buildings standing, for the rules.
    pub fn outcome(&self, royals: &[crate::game::royals::Royal]) -> CastleOutcome {
        let royals_killed = self
            .killed
            .iter()
            .filter_map(|&i| match self.castle.bodies[i] {
                BodyOwner::Royal(r) => royals.get(r).map(|k| k.kind),
                BodyOwner::Building(_) => None,
            })
            .collect();
        let mut buildings_survived = Vec::new();
        for (i, owner) in self.castle.bodies.iter().enumerate() {
            if let BodyOwner::Building(id) = *owner {
                if self.castle.world.bodies[i].alive {
                    buildings_survived.push(id);
                }
            }
        }
        CastleOutcome {
            owner: self.owner,
            attacker: self.attacker,
            ghost: self.ghost,
            royals_killed,
            buildings_survived,
        }
    }
}

/// Shot hits each body survives this siege: royals from their owner's
/// upgrades (`Modifiers::extra_lives`), buildings none.
pub fn body_lives(bodies: &[BodyOwner], owner: &crate::game::flow::Player) -> Vec<u32> {
    bodies
        .iter()
        .map(|b| match *b {
            BodyOwner::Royal(r) => owner.royals.get(r).map_or(0, |k| owner.mods.extra_lives(k.kind)),
            BodyOwner::Building(_) => 0,
        })
        .collect()
}

/// Wind for a rule wind value.
pub fn physics_wind(w: i32) -> Fx {
    Fx(w as i64 * tune::WIND_PER_STEP)
}

/// Every field of one siege.
#[derive(Clone, Debug)]
pub struct Siege {
    pub fields: Vec<Field>,
    /// Ticks since the siege started.
    pub tick: u32,
    pub drain_left: u32,
}
