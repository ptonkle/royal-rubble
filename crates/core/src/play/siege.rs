//! The siege: one physics field per castle under attack, a trebuchet per
//! field, and the shots each shooter has left this round.

use super::castle::{BodyOwner, CastleWorld};
use super::tune::{self, ShotRow};
use super::units;
use crate::game::flow::{Difficulty, Loadout};
use crate::game::upgrades::{AirKind, AmmoKind};
use crate::game::{CastleOutcome, PlayerId, Refusal};
use crate::physics::{trebuchet_launch, AirEffect, Event, Fx, Vec2, World};
use crate::terrain;

/// One kind of shot in a shooter's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ammo {
    Normal,
    Special(AmmoKind),
}

impl Ammo {
    pub fn row(self) -> &'static ShotRow {
        match self {
            Ammo::Normal => &tune::NORMAL_SHOT,
            Ammo::Special(AmmoKind::Heavy) => &tune::HEAVY_SHOT,
            Ammo::Special(AmmoKind::Fire) => &tune::FIRE_SHOT,
            Ammo::Special(AmmoKind::Exploding) => &tune::EXPLODING_SHOT,
            Ammo::Special(AmmoKind::Spread) => &tune::SPREAD_SHOT,
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
}

/// Chooses a bot trebuchet's shots for a siege. The aiming AI plugs in
/// here (`Play::set_shot_planner`).
pub type ShotPlanner = fn(&SiegeView) -> Vec<PlannedShot>;

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
                air: None,
            });
            t += tune::BOT_SHOT_EVERY;
        }
    }
    out
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
    pub selected: u8,
    pub power: u8,
    /// Siege tick the current swing started.
    pub swing_from: Option<u32>,
    /// Projectiles of the last shot, and whether its air effect is used.
    pub live: Vec<usize>,
    pub air_used: bool,
    pub plan: Vec<PlannedShot>,
    /// Release tick and air trigger tick still to come for a planned shot.
    pending_air: Option<(u8, u32)>,
}

/// Physics launch power for a level's `shot_power_pct` (R1.703).
pub fn launch_power(pct: u32) -> u8 {
    let base = crate::physics::TUNE.treb_power_base;
    let scale = tune::LAUNCH_BASE_PCT * pct as i64 / 100;
    (base * scale / 100 - base).clamp(0, 255) as u8
}

impl Shooter {
    pub fn new(player: PlayerId, bot: bool, loadout: &Loadout) -> Shooter {
        let mut ammo = vec![(Ammo::Normal, loadout.normal)];
        ammo.extend(loadout.ammo.iter().map(|(k, n)| (Ammo::Special(*k), *n)));
        Shooter {
            player,
            bot,
            ammo,
            air: loadout.air.clone(),
            selected: 0,
            power: launch_power(loadout.power_pct),
            swing_from: None,
            live: Vec::new(),
            air_used: false,
            plan: Vec::new(),
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
        let from = self.shooter.swing_from.ok_or(Refusal::WrongPhase)?;
        let slot = self.shooter.selected as usize;
        let (ammo, left) = self.shooter.ammo[slot];
        if left == 0 {
            return Err(Refusal::Limit);
        }
        self.shooter.ammo[slot].1 -= 1;
        self.shooter.swing_from = None;
        let launch = trebuchet_launch(self.base, now - from, self.shooter.power, false);
        let row = ammo.row();
        let w = &mut self.castle.world;
        let i = w.fire(launch, row.radius, row.mass);
        w.projectiles[i].fire = row.fire;
        self.shooter.live = if row.spread {
            w.air_effect(i, AirEffect::Split)
        } else {
            vec![i]
        };
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
        if self.shooter.air_used {
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
            if p.swing_at + p.release_after == now && self.release(now).is_ok() {
                if let Some((slot, after)) = p.air {
                    self.shooter.pending_air = Some((slot, now + after));
                }
            }
        }
    }

    /// One physics tick; shots in the water are spent.
    pub fn step(&mut self) -> Vec<Event> {
        let ev = self.castle.world.step();
        for b in self.castle.world.projectiles.iter_mut() {
            if b.alive && units::in_water(&self.plot, b.pos) {
                b.alive = false;
            }
        }
        ev
    }

    /// Shots or debris still moving.
    pub fn active(&self) -> bool {
        let w = &self.castle.world;
        w.projectiles.iter().chain(w.debris.iter()).any(|b| b.alive)
    }

    /// A bot is done when its plan is spent and nothing it threw still flies.
    pub fn bot_done(&self, now: u32) -> bool {
        self.shooter.bot
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
        let mut royals_killed = Vec::new();
        let mut buildings_survived = Vec::new();
        for (i, owner) in self.castle.bodies.iter().enumerate() {
            let alive = self.castle.world.bodies[i].alive;
            match *owner {
                BodyOwner::Royal(r) if !alive => {
                    if let Some(k) = royals.get(r) {
                        royals_killed.push(k.kind);
                    }
                }
                BodyOwner::Building(id) if alive => buildings_survived.push(id),
                _ => {}
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
