//! The siege hook: `plan_siege_job` has play/'s `ShotPlanner` signature,
//! so `play.set_shot_planner(ai::planner::plan_siege_job)` gives bot
//! trebuchets the aimer.
//!
//! Each shot is aimed at the world as it stood at the siege start, at the
//! royals the earlier shots are not predicted to kill (each planned shot is
//! played out once to see which royals it takes). Shots go in the ammo
//! bar's order, spaced as play/'s stand-in planner spaces them. The work is
//! done a slice at a time while the siege runs (R1.2000); `plan_siege`
//! does it all at once and gives the same shots.

use super::aim::{Plan, PlayOut, Search, Shooter, Target};
use crate::game::flow::Difficulty;
use crate::physics::{AirEffect, World};
use crate::play::siege::{air_effect, Ammo, PlanJob, PlannedShot, SiegeView};
use crate::play::tune as play_tune;

/// The royals in a siege world, each worth its damage, so a King draws the
/// aim before a Squire (R1.912). A view with no royal list (the probes)
/// tells a royal from a building by its radius, each worth 1.
pub fn royal_targets(v: &SiegeView) -> Vec<Target> {
    if !v.royals.is_empty() {
        return v
            .royals
            .iter()
            .filter(|(body, _)| v.world.bodies.get(*body).is_some_and(|b| b.alive))
            .map(|&(body, damage)| Target { body, value: damage })
            .collect();
    }
    v.world
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.alive && b.radius == play_tune::ROYAL_RADIUS)
        .map(|(body, _)| Target { body, value: 1 })
        .collect()
}

/// Every shot of the siege at once, as `SiegePlanner` plans them a slice
/// at a time (the probes and tests).
pub fn plan_siege(v: &SiegeView) -> Vec<PlannedShot> {
    let mut job = SiegePlanner::new(v);
    while job.next_cost() > 0 {
        job.advance();
    }
    job.take_ready()
}

/// `plan_siege` as play/'s `ShotPlanner`: the shots are planned while the
/// siege runs, a budget of work a tick (R1.2000).
pub fn plan_siege_job(v: &SiegeView) -> Box<dyn PlanJob> {
    Box::new(SiegePlanner::new(v))
}

/// A whole siege's shots, planned a slice at a time against a copy of the
/// world as it stood at the siege start, so the choices are the same as
/// planning them all at once.
#[derive(Clone)]
pub struct SiegePlanner {
    world: World,
    base: crate::physics::Vec2,
    power: u8,
    ammo: Vec<(Ammo, u32)>,
    effects: Vec<AirEffect>,
    d: Difficulty,
    /// Every royal at the start, to aim at again once all are predicted to fall.
    all: Vec<Target>,
    left: Vec<Target>,
    seed: u64,
    swing: u32,
    n: u64,
    /// Shots that miss on paper, so the next one tries something else.
    misses: Vec<Plan>,
    /// Ammo bar slot and shot within it planned next.
    slot: usize,
    k: u32,
    stage: Step,
    ready: Vec<PlannedShot>,
}

#[derive(Clone, Debug)]
enum Step {
    /// Start the next shot's search (or finish).
    Next,
    Search(Box<Search>, Shooter, Vec<Target>),
    /// Playing the chosen shot out to see which royals it takes.
    Kills(Box<PlayOut>, Plan, Vec<Target>),
    Done,
}

impl std::fmt::Debug for SiegePlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SiegePlanner(shot {}, ready {})", self.n, self.ready.len())
    }
}

impl SiegePlanner {
    pub fn new(v: &SiegeView) -> SiegePlanner {
        let all = royal_targets(v);
        SiegePlanner {
            world: v.world.clone(),
            base: v.base,
            power: v.power,
            ammo: v.ammo.to_vec(),
            effects: v.air.iter().map(|k| air_effect(*k)).collect(),
            // a ghost's stand-in trebuchet aims like a normal bot
            d: v.difficulty.unwrap_or(Difficulty::Normal),
            left: all.clone(),
            all,
            seed: v.world.checksum(),
            swing: play_tune::BOT_FIRST_SWING,
            n: 0,
            misses: Vec::new(),
            slot: 0,
            k: 0,
            stage: Step::Next,
            ready: Vec::new(),
        }
    }

    fn shooter(&self, slot: usize) -> Shooter {
        let row = crate::physics::ammo::shot_tune(self.ammo[slot].0.kind());
        Shooter {
            base: self.base,
            facing_left: false,
            power: self.power,
            radius: row.radius,
            mass: row.mass,
            fire: row.ignites,
            spread: row.pieces > 1,
        }
    }

    fn step(&mut self, stage: Step) -> (Step, u64) {
        match stage {
            Step::Next => {
                while self.slot < self.ammo.len() && self.k >= self.ammo[self.slot].1 {
                    self.slot += 1;
                    self.k = 0;
                }
                if self.slot >= self.ammo.len() {
                    return (Step::Done, 1);
                }
                let s = self.shooter(self.slot);
                // every royal already falls: aim at them all again
                let targets = if self.left.is_empty() {
                    self.all.clone()
                } else {
                    self.left.clone()
                };
                let search = Search::new(
                    &self.world,
                    s,
                    &targets,
                    &self.effects,
                    self.d,
                    self.seed ^ self.n,
                    &self.misses,
                );
                (Step::Search(Box::new(search), s, targets), 1)
            }
            Step::Search(mut search, s, targets) => {
                let cost = search.advance(&self.world);
                let Some(plan) = search.plan() else {
                    return (Step::Search(search, s, targets), cost);
                };
                let air = plan.effect.and_then(|(e, at)| {
                    self.effects
                        .iter()
                        .position(|x| *x == e)
                        .map(|slot| (slot as u8, at.max(1)))
                });
                self.ready.push(PlannedShot {
                    ammo: self.slot as u8,
                    swing_at: self.swing,
                    release_after: plan.release_tick,
                    air,
                });
                self.swing += play_tune::BOT_SHOT_EVERY;
                self.n += 1;
                self.k += 1;
                if targets.is_empty() {
                    return (Step::Next, cost);
                }
                let run = PlayOut::new(s, plan, &targets);
                (Step::Kills(Box::new(run), plan, targets), cost)
            }
            Step::Kills(mut run, plan, targets) => {
                let cost = run.advance(&self.world);
                let Some(o) = run.outcome() else {
                    return (Step::Kills(run, plan, targets), cost);
                };
                let mask = o.killed_mask;
                if mask == 0 {
                    self.misses.push(plan);
                }
                self.left = targets
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i >= 64 || mask & (1 << i) == 0)
                    .map(|(_, t)| *t)
                    .collect();
                (Step::Next, cost)
            }
            Step::Done => (Step::Done, 0),
        }
    }
}

impl PlanJob for SiegePlanner {
    fn next_cost(&self) -> u64 {
        match &self.stage {
            Step::Next => 1,
            Step::Search(s, ..) => s.next_cost(),
            Step::Kills(r, ..) => r.next_cost(),
            Step::Done => 0,
        }
    }

    fn advance(&mut self) -> u64 {
        let stage = std::mem::replace(&mut self.stage, Step::Done);
        let (stage, cost) = self.step(stage);
        self.stage = stage;
        cost
    }

    fn take_ready(&mut self) -> Vec<PlannedShot> {
        std::mem::take(&mut self.ready)
    }

    fn planned(&self) -> u32 {
        self.n as u32
    }

    fn box_clone(&self) -> Box<dyn PlanJob> {
        Box::new(self.clone())
    }
}
