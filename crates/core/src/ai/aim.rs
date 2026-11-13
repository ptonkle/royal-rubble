//! Aiming: pick a release tick, and maybe an air effect and when to trigger
//! it, to kill royals in a defender's castle.
//!
//! Two passes. A quick pass flies every candidate through wind and over the
//! terrain alone (the same free-flight sums as `physics::balls`) and keeps
//! the paths that come closest to a royal. The best few are then played out
//! on a clone of the real world with the real physics, from the moment the
//! shot reaches the castle, and scored by royals killed. Difficulty sets how
//! many are verified, how finely triggers are tried and how often the bot
//! misses on purpose (seeded, so a replay misses the same way).

use super::tune::{self, AimTune};
use crate::game::flow::Difficulty;
use crate::physics::{trebuchet_launch, AirEffect, Ball, Event, Fx, Vec2, World, TUNE};
use crate::rng::Rng;

/// The attacker's trebuchet and the shot loaded in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shooter {
    pub base: Vec2,
    pub facing_left: bool,
    pub power: u8,
    pub radius: Fx,
    pub mass: Fx,
    pub fire: bool,
    /// Splits into three as it leaves the arm (spread ammo).
    pub spread: bool,
}

/// A royal to aim at: its body in the world and what killing it is worth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub body: usize,
    pub value: u32,
}

/// One shot: release tick from the swing start, and an optional air effect
/// applied just before the world step `trigger` steps after the release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub release_tick: u32,
    pub effect: Option<(AirEffect, u32)>,
}

/// What a plan did when played out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Sum of the values of targets killed.
    pub killed: u32,
    /// Segments broken near a living target.
    pub near_breaks: u32,
    /// Closest any shot came to a living target, raw Q16.16 (i64::MAX: none).
    pub closest: i64,
    /// Bit i set: target i was killed (first 64 targets).
    pub killed_mask: u64,
}

impl Outcome {
    pub fn score(&self) -> i64 {
        let unit = Fx::ONE.raw();
        let close_from = tune::CLOSE_UNITS * unit;
        let close = (close_from - self.closest.min(close_from)) / (unit / 16);
        self.killed as i64 * tune::KILL_SCORE + self.near_breaks as i64 * tune::NEAR_BREAK_SCORE + close
    }
}

fn launch_ball(s: &Shooter, release_tick: u32) -> Ball {
    let l = trebuchet_launch(s.base, release_tick, s.power, s.facing_left);
    let mut b = Ball::new(l.pos, l.vel, s.radius, s.mass);
    b.fire = s.fire;
    b
}

/// The shots a release puts in the air: one, or three for spread ammo.
fn launch_balls(s: &Shooter, release_tick: u32) -> Vec<Ball> {
    let b = launch_ball(s, release_tick);
    if s.spread {
        b.split(TUNE.split_spread).to_vec()
    } else {
        vec![b]
    }
}

/// The release ticks that throw forward, from the real trebuchet: the
/// window follows any retune of the arm.
pub fn release_ticks(s: &Shooter) -> Vec<u32> {
    (1..=tune::RELEASE_SCAN_MAX)
        .filter(|&t| {
            let v = trebuchet_launch(s.base, t, s.power, s.facing_left).vel;
            if s.facing_left {
                v.x < Fx::ZERO
            } else {
                v.x > Fx::ZERO
            }
        })
        .collect()
}

/// An air effect on every shot of a throw, as play/ applies it.
fn apply_effect(balls: &mut Vec<Ball>, effect: AirEffect) {
    let mut out = Vec::with_capacity(balls.len());
    for b in balls.iter() {
        let mut b = *b;
        match effect {
            AirEffect::Drop => b.drop_down(),
            AirEffect::Reverse => b.reverse(),
            AirEffect::Phase => b.phase_for(TUNE.phase_ticks),
            AirEffect::Dive => b.dive(),
            AirEffect::Boost => b.boost(TUNE.boost_speed),
            AirEffect::Hover => b.hover_for(TUNE.hover_ticks),
            AirEffect::Split => {
                out.extend(b.split(TUNE.split_spread));
                continue;
            }
        }
        out.push(b);
    }
    *balls = out;
}

/// One free-flight tick, as `physics::balls` steps a shot in the air.
fn fly(b: &mut Ball, wind: Fx) {
    b.vel.y -= TUNE.gravity;
    b.vel.x += wind;
    b.pos += b.vel;
    b.age += 1;
}

/// The box around the live castle and its targets, widened by a margin.
struct Bounds {
    lo: Vec2,
    hi: Vec2,
}

impl Bounds {
    fn of(world: &World, targets: &[Target]) -> Option<Bounds> {
        let mut pts = world
            .segments
            .iter()
            .filter(|s| !s.broken)
            .flat_map(|s| [world.nodes[s.a].pos, world.nodes[s.b].pos])
            .chain(
                targets
                    .iter()
                    .filter_map(|t| world.bodies.get(t.body))
                    .filter(|b| b.alive)
                    .map(|b| b.pos),
            );
        let first = pts.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in pts {
            lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let m = Fx(tune::BOX_MARGIN_RAW);
        Some(Bounds {
            lo: lo - Vec2::new(m, m),
            hi: hi + Vec2::new(m, m),
        })
    }

    fn holds(&self, p: Vec2, r: Fx) -> bool {
        p.x + r >= self.lo.x && p.x - r <= self.hi.x && p.y + r >= self.lo.y && p.y - r <= self.hi.y
    }
}

fn live_targets(world: &World, targets: &[Target]) -> Vec<(Vec2, Fx)> {
    targets
        .iter()
        .filter_map(|t| world.bodies.get(t.body))
        .filter(|b| b.alive)
        .map(|b| (b.pos, b.radius))
        .collect()
}

/// Plays a plan out on a clone of `world` with the full physics and reports
/// what it did. When the world is quiet (settled, nothing flying) the shot
/// flies on its own until it reaches the castle, which saves most of the
/// cost; otherwise every tick is stepped.
pub fn play_out(world: &World, s: &Shooter, plan: Plan, targets: &[Target]) -> Outcome {
    let quiet = world.settle_left == 0
        && world.projectiles.iter().all(|b| !b.alive)
        && world.debris.iter().all(|b| !b.alive);
    let bounds = Bounds::of(world, targets);
    let mut balls = launch_balls(s, plan.release_tick);
    let mut t = 0u32;
    let mut effect_done = false;
    if quiet {
        if let Some(bx) = &bounds {
            while t < tune::FLIGHT_MAX && !balls.iter().any(|b| bx.holds(b.pos, b.radius)) {
                if let Some((e, at)) = plan.effect {
                    if at == t {
                        apply_effect(&mut balls, e);
                        effect_done = true;
                    }
                }
                for b in balls.iter_mut() {
                    fly(b, world.wind);
                }
                t += 1;
                if balls
                    .iter()
                    .all(|b| b.pos.y - b.radius < world.terrain.height_at(b.pos.x))
                {
                    break;
                }
            }
        }
    }
    let mut w = world.clone();
    let alive_before: Vec<bool> = targets
        .iter()
        .map(|tg| w.bodies.get(tg.body).is_some_and(|b| b.alive))
        .collect();
    let mut mine: Vec<usize> = Vec::new();
    for b in balls {
        w.projectiles.push(b);
        mine.push(w.projectiles.len() - 1);
    }
    let mut out = Outcome {
        closest: i64::MAX,
        ..Outcome::default()
    };
    let mut after = 0u32;
    let mut steps = 0u32;
    while steps < tune::VERIFY_TICKS + tune::AFTER_TICKS && after < tune::AFTER_TICKS {
        if let (Some((e, at)), false) = (plan.effect, effect_done) {
            if at == t {
                let live: Vec<usize> = mine.iter().copied().filter(|&p| w.projectiles[p].alive).collect();
                mine = live.into_iter().flat_map(|p| w.air_effect(p, e)).collect();
                effect_done = true;
            }
        }
        let aims = live_targets(&w, targets);
        for &p in &mine {
            let b = w.projectiles[p];
            if !b.alive {
                continue;
            }
            for (q, r) in &aims {
                let d = ((b.pos - *q).len() - *r - b.radius).raw().max(0);
                out.closest = out.closest.min(d);
            }
        }
        for ev in w.step() {
            if let Event::Broke { segment, .. } = ev {
                let seg = &w.segments[segment];
                let mid = Vec2::lerp(w.nodes[seg.a].pos, w.nodes[seg.b].pos, Fx::HALF);
                let near = Fx::from_int(tune::NEAR_UNITS);
                if aims.iter().any(|(q, _)| (mid - *q).len() <= near) {
                    out.near_breaks += 1;
                }
            }
        }
        t += 1;
        steps += 1;
        // a shot is spent once it is gone or has left the castle's box
        let spent = |b: &Ball| !b.alive || bounds.as_ref().is_some_and(|bx| !bx.holds(b.pos, b.radius));
        if mine.iter().all(|&p| spent(&w.projectiles[p])) {
            after += 1;
        }
    }
    for (i, (tg, was)) in targets.iter().zip(alive_before).enumerate() {
        if was && w.bodies.get(tg.body).is_some_and(|b| !b.alive) {
            out.killed += tg.value;
            if i < 64 {
                out.killed_mask |= 1 << i;
            }
        }
    }
    out
}

/// Quick pass for one release tick: the closest approach (raw, centers
/// less radii) of the plain flight and of every effect trigger, as plans.
/// The plain flight is flown once; each trigger flies on from its state.
fn quick_pass(
    world: &World,
    s: &Shooter,
    release_tick: u32,
    effects: &[AirEffect],
    a: &AimTune,
    aims: &[(Vec2, Fx)],
    out: &mut Vec<(i64, Plan)>,
) {
    let gap = |b: &Ball| -> i64 {
        aims.iter()
            // below zero: overlapping, more central the lower
            .map(|(p, r)| ((b.pos - *p).len() - *r - b.radius).raw())
            .min()
            .unwrap_or(i64::MAX)
    };
    // path[t] is the state before step t; prefix[t] the best gap before it
    let mut path = Vec::new();
    let mut prefix = Vec::new();
    let mut b = launch_ball(s, release_tick);
    let mut best = i64::MAX;
    for _ in 0..tune::FLIGHT_MAX {
        path.push(b);
        prefix.push(best);
        fly(&mut b, world.wind);
        best = best.min(gap(&b));
        if b.pos.y - b.radius < world.terrain.height_at(b.pos.x) {
            break;
        }
    }
    out.push((
        best,
        Plan {
            release_tick,
            effect: None,
        },
    ));
    // effects are tried only near the targets
    let reach = Fx::from_int(tune::TRIGGER_REACH_UNITS);
    let lo = aims.iter().map(|(p, _)| p.x).min().unwrap_or(Fx::ZERO) - reach;
    let hi = aims.iter().map(|(p, _)| p.x).max().unwrap_or(Fx::ZERO) + reach;
    for &e in effects {
        let mut at = a.trigger_stride.max(1);
        while (at as usize) < path.len() {
            let x = path[at as usize].pos.x;
            if x < lo || x > hi {
                at += a.trigger_stride.max(1);
                continue;
            }
            let mut balls = vec![path[at as usize]];
            apply_effect(&mut balls, e);
            let mut best = prefix[at as usize];
            for mut b in balls {
                for _ in at..tune::FLIGHT_MAX {
                    fly(&mut b, world.wind);
                    best = best.min(gap(&b));
                    if b.pos.y - b.radius < world.terrain.height_at(b.pos.x) {
                        break;
                    }
                }
            }
            out.push((
                best,
                Plan {
                    release_tick,
                    effect: Some((e, at)),
                },
            ));
            at += a.trigger_stride.max(1);
        }
    }
}

/// The bot's shot: the best plan it finds against `targets`, then its
/// deliberate error. `effects` are the air effects it owns for this shot.
pub fn plan_shot(
    world: &World,
    s: &Shooter,
    targets: &[Target],
    effects: &[AirEffect],
    d: Difficulty,
    seed: u64,
) -> Plan {
    plan_shot_avoiding(world, s, targets, effects, d, seed, &[])
}

/// As `plan_shot`, never choosing a plan in `avoid` (shots already planned
/// against the same world that are not predicted to kill).
pub fn plan_shot_avoiding(
    world: &World,
    s: &Shooter,
    targets: &[Target],
    effects: &[AirEffect],
    d: Difficulty,
    seed: u64,
    avoid: &[Plan],
) -> Plan {
    let a = tune::aim(d);
    let aims = live_targets(world, targets);
    let mut ranked: Vec<(i64, Plan)> = Vec::new();
    let ticks = release_ticks(s);
    for &r in &ticks {
        quick_pass(world, s, r, effects, &a, &aims, &mut ranked);
    }
    // closest first; plain shots before effects, earlier ticks first on ties
    ranked.retain(|(_, p)| !avoid.contains(p));
    ranked.sort_by_key(|(d, p)| (*d, p.effect.is_some(), p.release_tick, p.effect.map(|e| e.1)));
    let mut best: Option<(i64, Plan)> = None;
    let mut check: Vec<Plan> = ranked.iter().take(a.verified.max(1)).map(|r| r.1).collect();
    if a.each_effect {
        // the best plan with each effect too: the quick pass can't see walls
        for &e in effects {
            if let Some((_, p)) = ranked.iter().find(|(_, p)| p.effect.map(|x| x.0) == Some(e)) {
                if !check.contains(p) {
                    check.push(*p);
                }
            }
        }
    }
    for plan in &check {
        let score = play_out(world, s, *plan, targets).score();
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, *plan));
        }
    }
    let mut plan = best.map(|(_, p)| p).unwrap_or(Plan {
        release_tick: ticks.first().copied().unwrap_or(1),
        effect: None,
    });
    let mut rng = Rng::new(seed);
    if a.miss_ticks > 0 && rng.chance(a.miss_chance.0, a.miss_chance.1) {
        let mut off = rng.between(1, a.miss_ticks);
        if rng.chance(1, 2) {
            off = -off;
        }
        plan.release_tick = (plan.release_tick as i32 + off).max(0) as u32;
    }
    plan
}

/// Fires `plan` in the live world and steps it `ticks` times, applying the
/// air effect on time. For tests and for play/ to copy the convention.
pub fn fire_and_step(world: &mut World, s: &Shooter, plan: Plan, ticks: u32) -> Vec<Event> {
    let l = trebuchet_launch(s.base, plan.release_tick, s.power, s.facing_left);
    let p = world.fire(l, s.radius, s.mass);
    world.projectiles[p].fire = s.fire;
    let mut live = if s.spread {
        world.air_effect(p, AirEffect::Split)
    } else {
        vec![p]
    };
    let mut events = Vec::new();
    for t in 0..ticks {
        if let Some((e, at)) = plan.effect {
            if at == t {
                let now: Vec<usize> = live
                    .iter()
                    .copied()
                    .filter(|&i| world.projectiles[i].alive)
                    .collect();
                live = now.into_iter().flat_map(|i| world.air_effect(i, e)).collect();
            }
        }
        events.extend(world.step());
    }
    events
}
