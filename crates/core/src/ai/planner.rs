//! The siege hook: `plan_siege` has play/'s `ShotPlanner` signature, so
//! `play.set_shot_planner(ai::planner::plan_siege)` gives bot trebuchets
//! the aimer.
//!
//! play/ asks for every shot at the siege start, so each shot is aimed at
//! the world as it stands then, at the royals the earlier shots are not
//! predicted to kill (each planned shot is played out once to see which
//! royals it takes). Shots go in the ammo bar's order, spaced as play/'s
//! stand-in planner spaces them.

use super::aim::{plan_shot_avoiding, play_out, Plan, Shooter, Target};
use crate::game::flow::Difficulty;
use crate::physics::AirEffect;
use crate::play::siege::{air_effect, PlannedShot, SiegeView};
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

pub fn plan_siege(v: &SiegeView) -> Vec<PlannedShot> {
    // a ghost's stand-in trebuchet aims like a normal bot
    let d = v.difficulty.unwrap_or(Difficulty::Normal);
    let effects: Vec<AirEffect> = v.air.iter().map(|k| air_effect(*k)).collect();
    let mut left = royal_targets(v);
    let seed = v.world.checksum();
    let mut out = Vec::new();
    let mut swing = play_tune::BOT_FIRST_SWING;
    let mut n = 0u64;
    // shots that miss on paper, so the next one tries something else
    let mut misses: Vec<Plan> = Vec::new();
    for (slot, (ammo, count)) in v.ammo.iter().enumerate() {
        let row = crate::physics::ammo::shot_tune(ammo.kind());
        let s = Shooter {
            base: v.base,
            facing_left: false,
            power: v.power,
            radius: row.radius,
            mass: row.mass,
            fire: row.ignites,
            spread: row.pieces > 1,
        };
        for _ in 0..*count {
            // every royal already falls: aim at them all again
            let targets = if left.is_empty() {
                royal_targets(v)
            } else {
                left.clone()
            };
            let plan = plan_shot_avoiding(v.world, &s, &targets, &effects, d, seed ^ n, &misses);
            let air = plan.effect.and_then(|(e, at)| {
                effects
                    .iter()
                    .position(|x| *x == e)
                    .map(|slot| (slot as u8, at.max(1)))
            });
            out.push(PlannedShot {
                ammo: slot as u8,
                swing_at: swing,
                release_after: plan.release_tick,
                air,
            });
            if !targets.is_empty() {
                let mask = play_out(v.world, &s, plan, &targets).killed_mask;
                if mask == 0 {
                    misses.push(plan);
                }
                left = targets
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i >= 64 || mask & (1 << i) == 0)
                    .map(|(_, t)| *t)
                    .collect();
            }
            swing += play_tune::BOT_SHOT_EVERY;
            n += 1;
        }
    }
    out
}
