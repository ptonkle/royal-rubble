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
use crate::physics::{
    trebuchet_launch, trebuchet_launch_at, AirEffect, Ball, Event, Fx, Vec2, World, RELEASE_PARTS, TUNE,
};
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

/// One shot: release time from the swing start, in sixteenths of a tick
/// (R1.2100), and an optional air effect applied just before the world
/// step `trigger` steps after the release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub release: u32,
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

fn launch_ball(s: &Shooter, release: u32) -> Ball {
    let l = trebuchet_launch_at(s.base, release, s.power, s.facing_left);
    let mut b = Ball::new(l.pos, l.vel, s.radius, s.mass);
    b.fire = s.fire;
    b
}

/// The shots a release puts in the air: one, or three for spread ammo.
fn launch_balls(s: &Shooter, release: u32) -> Vec<Ball> {
    let b = launch_ball(s, release);
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
#[derive(Clone, Debug)]
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

/// A live target: its body's foot circle center, radius and upright.
type Aim = (Vec2, Fx, Fx);

/// How far a ball's edge is from a target's (below zero: overlapping).
fn edge_gap(b: &Ball, &(q, rad, tall): &Aim) -> Fx {
    let near = Vec2::new(q.x, b.pos.y.clamp_to(q.y, q.y + tall));
    (b.pos - near).len() - rad - b.radius
}

fn live_targets(world: &World, targets: &[Target]) -> Vec<Aim> {
    targets
        .iter()
        .filter_map(|t| world.bodies.get(t.body))
        .filter(|b| b.alive)
        .map(|b| (b.pos, b.radius, b.tall))
        .collect()
}

/// Plays a plan out on a clone of `world` with the full physics and reports
/// what it did. When the world is quiet (settled, nothing flying) the shot
/// flies on its own until it reaches the castle, which saves most of the
/// cost; otherwise every tick is stepped.
pub fn play_out(world: &World, s: &Shooter, plan: Plan, targets: &[Target]) -> Outcome {
    let mut run = PlayOut::new(*s, plan, targets);
    loop {
        if let Some(o) = run.outcome() {
            return o;
        }
        run.advance(world);
    }
}

/// `play_out` in slices: each `advance` does one unit of work (the free
/// flight and the world's clone, then one full-physics step) and says what
/// it cost, in the work units of `tune::WORLD_STEP_COST`.
#[derive(Clone, Debug)]
pub struct PlayOut {
    s: Shooter,
    plan: Plan,
    targets: Vec<Target>,
    state: PlayState,
}

#[derive(Clone, Debug)]
enum PlayState {
    Start,
    Run(Box<PlayRun>),
    Done(Outcome),
}

#[derive(Clone, Debug)]
struct PlayRun {
    w: World,
    bounds: Option<Bounds>,
    t: u32,
    effect_done: bool,
    mine: Vec<usize>,
    out: Outcome,
    after: u32,
    steps: u32,
    alive_before: Vec<bool>,
}

/// Work units of one free-flight tick of one shot, with `aims` targets.
fn fly_cost(aims: usize) -> u64 {
    1 + aims as u64
}

impl PlayOut {
    pub fn new(s: Shooter, plan: Plan, targets: &[Target]) -> PlayOut {
        PlayOut {
            s,
            plan,
            targets: targets.to_vec(),
            state: PlayState::Start,
        }
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match &self.state {
            PlayState::Done(o) => Some(*o),
            _ => None,
        }
    }

    /// The most the next `advance` can cost.
    pub fn next_cost(&self) -> u64 {
        match &self.state {
            PlayState::Start => tune::FLIGHT_MAX as u64 * 3 * fly_cost(self.targets.len()) + tune::CLONE_COST,
            PlayState::Run(_) => tune::WORLD_STEP_COST,
            PlayState::Done(_) => 0,
        }
    }

    pub fn advance(&mut self, world: &World) -> u64 {
        match &mut self.state {
            PlayState::Start => {
                let (run, cost) = self.start(world);
                self.state = PlayState::Run(Box::new(run));
                cost
            }
            PlayState::Run(run) => {
                Self::step(run, self.plan, &self.targets);
                if run.steps < tune::VERIFY_TICKS + tune::AFTER_TICKS && run.after < tune::AFTER_TICKS {
                    return tune::WORLD_STEP_COST;
                }
                let mut out = run.out;
                for (i, (tg, was)) in self.targets.iter().zip(&run.alive_before).enumerate() {
                    if *was && run.w.bodies.get(tg.body).is_some_and(|b| !b.alive) {
                        out.killed += tg.value;
                        if i < 64 {
                            out.killed_mask |= 1 << i;
                        }
                    }
                }
                self.state = PlayState::Done(out);
                tune::WORLD_STEP_COST
            }
            PlayState::Done(_) => 0,
        }
    }

    fn start(&self, world: &World) -> (PlayRun, u64) {
        let (s, plan, targets) = (&self.s, self.plan, &self.targets[..]);
        let quiet = world.settle_left == 0
            && world.projectiles.iter().all(|b| !b.alive)
            && world.debris.iter().all(|b| !b.alive);
        let bounds = Bounds::of(world, targets);
        let mut balls = launch_balls(s, plan.release);
        let mut t = 0u32;
        let mut effect_done = false;
        let mut cost = tune::CLONE_COST;
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
                    cost += balls.len() as u64;
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
        let run = PlayRun {
            w,
            bounds,
            t,
            effect_done,
            mine,
            out: Outcome {
                closest: i64::MAX,
                ..Outcome::default()
            },
            after: 0,
            steps: 0,
            alive_before,
        };
        (run, cost)
    }

    fn step(r: &mut PlayRun, plan: Plan, targets: &[Target]) {
        if let (Some((e, at)), false) = (plan.effect, r.effect_done) {
            if at == r.t {
                let w = &mut r.w;
                let live: Vec<usize> = r
                    .mine
                    .iter()
                    .copied()
                    .filter(|&p| w.projectiles[p].alive)
                    .collect();
                r.mine = live.into_iter().flat_map(|p| w.air_effect(p, e)).collect();
                r.effect_done = true;
            }
        }
        let aims = live_targets(&r.w, targets);
        for &p in &r.mine {
            let b = r.w.projectiles[p];
            if !b.alive {
                continue;
            }
            for a in &aims {
                let d = edge_gap(&b, a).raw().max(0);
                r.out.closest = r.out.closest.min(d);
            }
        }
        for ev in r.w.step() {
            if let Event::Broke { segment, .. } = ev {
                let seg = &r.w.segments[segment];
                let mid = Vec2::lerp(r.w.nodes[seg.a].pos, r.w.nodes[seg.b].pos, Fx::HALF);
                let near = Fx::from_int(tune::NEAR_UNITS);
                if aims.iter().any(|(q, _, _)| (mid - *q).len() <= near) {
                    r.out.near_breaks += 1;
                }
            }
        }
        r.t += 1;
        r.steps += 1;
        // a shot is spent once it is gone or has left the castle's box
        let bounds = &r.bounds;
        let spent = |b: &Ball| !b.alive || bounds.as_ref().is_some_and(|bx| !bx.holds(b.pos, b.radius));
        if r.mine.iter().all(|&p| spent(&r.w.projectiles[p])) {
            r.after += 1;
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
    let mut search = Search::new(world, *s, targets, effects, d, seed, avoid);
    loop {
        if let Some(p) = search.plan() {
            return p;
        }
        search.advance(world);
    }
}

/// `plan_shot_avoiding` in slices, against the same `world` every call.
/// Two passes. The quick pass flies each release tick's plain path, then
/// each air-effect trigger from it, one flight per `advance`. The best
/// few are then played out (`PlayOut`), one physics step per `advance`.
#[derive(Clone, Debug)]
pub struct Search {
    s: Shooter,
    targets: Vec<Target>,
    effects: Vec<AirEffect>,
    avoid: Vec<Plan>,
    seed: u64,
    a: AimTune,
    aims: Vec<Aim>,
    /// Effects are tried only while the shot is between these x.
    lo: Fx,
    hi: Fx,
    /// Release times tried, sixteenths: the whole ticks first, then the
    /// fine times around the best plain shots once those are known.
    ticks: Vec<u32>,
    /// How many of `ticks` are whole ticks (`usize::MAX`: not refined yet).
    coarse: usize,
    ranked: Vec<(i64, Plan)>,
    stage: Stage,
}

#[derive(Clone, Debug)]
enum Stage {
    /// Release tick `ticks[ti]`: `path[t]` is the plain flight's state
    /// before step t (empty: not flown yet), `prefix[t]` the best gap
    /// before it; effect `ei` is tried next at trigger `at`.
    Quick {
        ti: usize,
        path: Vec<Ball>,
        prefix: Vec<i64>,
        ei: usize,
        at: u32,
    },
    Verify {
        check: Vec<Plan>,
        ci: usize,
        run: Option<PlayOut>,
        best: Option<(i64, Plan)>,
    },
    Done(Plan),
}

impl Search {
    pub fn new(
        world: &World,
        s: Shooter,
        targets: &[Target],
        effects: &[AirEffect],
        d: Difficulty,
        seed: u64,
        avoid: &[Plan],
    ) -> Search {
        let aims = live_targets(world, targets);
        let reach = Fx::from_int(tune::TRIGGER_REACH_UNITS);
        let lo = aims.iter().map(|(p, _, _)| p.x).min().unwrap_or(Fx::ZERO) - reach;
        let hi = aims.iter().map(|(p, _, _)| p.x).max().unwrap_or(Fx::ZERO) + reach;
        Search {
            s,
            targets: targets.to_vec(),
            effects: effects.to_vec(),
            avoid: avoid.to_vec(),
            seed,
            a: tune::aim(d),
            aims,
            lo,
            hi,
            ticks: release_ticks(&s).iter().map(|t| t * RELEASE_PARTS).collect(),
            coarse: usize::MAX,
            ranked: Vec::new(),
            stage: Stage::Quick {
                ti: 0,
                path: Vec::new(),
                prefix: Vec::new(),
                ei: 0,
                at: 0,
            },
        }
    }

    /// The chosen plan, once the search is over.
    pub fn plan(&self) -> Option<Plan> {
        match self.stage {
            Stage::Done(p) => Some(p),
            _ => None,
        }
    }

    /// The most the next `advance` can cost (0: done).
    pub fn next_cost(&self) -> u64 {
        match &self.stage {
            // a split trigger flies three shots the whole way
            Stage::Quick { .. } => {
                tune::FLIGHT_MAX as u64 * 3 * fly_cost(self.aims.len()) + tune::FLIGHT_MAX as u64
            }
            Stage::Verify { check, ci, run, .. } => match run {
                Some(r) => r.next_cost(),
                None if *ci < check.len() => PlayOut::new(self.s, check[*ci], &self.targets).next_cost(),
                None => 1,
            },
            Stage::Done(_) => 0,
        }
    }

    fn gap(&self, b: &Ball) -> i64 {
        self.aims
            .iter()
            // below zero: overlapping, more central the lower
            .map(|a| edge_gap(b, a).raw())
            .min()
            .unwrap_or(i64::MAX)
    }

    /// One unit of work; returns its cost.
    pub fn advance(&mut self, world: &World) -> u64 {
        let placeholder = Stage::Done(Plan {
            release: 0,
            effect: None,
        });
        let stage = std::mem::replace(&mut self.stage, placeholder);
        let (stage, cost) = match stage {
            Stage::Quick {
                ti,
                path,
                prefix,
                ei,
                at,
            } => self.quick(world, ti, path, prefix, ei, at),
            Stage::Verify { check, ci, run, best } => self.verify(world, check, ci, run, best),
            done => (done, 0),
        };
        self.stage = stage;
        cost
    }

    fn quick(
        &mut self,
        world: &World,
        ti: usize,
        mut path: Vec<Ball>,
        mut prefix: Vec<i64>,
        mut ei: usize,
        mut at: u32,
    ) -> (Stage, u64) {
        if ti == self.ticks.len() && self.coarse == usize::MAX {
            let n = self.ranked.len() as u64;
            self.refine();
            if ti < self.ticks.len() {
                return (
                    Stage::Quick {
                        ti,
                        path,
                        prefix,
                        ei,
                        at,
                    },
                    1 + n,
                );
            }
        }
        let Some(&release) = self.ticks.get(ti) else {
            let n = self.ranked.len() as u64;
            return (self.rank(), 1 + n);
        };
        let fc = fly_cost(self.aims.len());
        let stride = self.a.trigger_stride.max(1);
        if path.is_empty() {
            let mut b = launch_ball(&self.s, release);
            let mut best = i64::MAX;
            for _ in 0..tune::FLIGHT_MAX {
                path.push(b);
                prefix.push(best);
                fly(&mut b, world.wind);
                best = best.min(self.gap(&b));
                if b.pos.y - b.radius < world.terrain.height_at(b.pos.x) {
                    break;
                }
            }
            self.ranked.push((
                best,
                Plan {
                    release,
                    effect: None,
                },
            ));
            let cost = path.len() as u64 * fc;
            if ti >= self.coarse {
                // fine times fly the plain shot only
                let next = Stage::Quick {
                    ti: ti + 1,
                    path: Vec::new(),
                    prefix: Vec::new(),
                    ei: 0,
                    at: 0,
                };
                return (next, cost);
            }
            let next = Stage::Quick {
                ti,
                path,
                prefix,
                ei: 0,
                at: stride,
            };
            return (next, cost);
        }
        // the next trigger near the targets, or the next release tick
        let mut skipped = 0u64;
        loop {
            let Some(&e) = self.effects.get(ei) else {
                let next = Stage::Quick {
                    ti: ti + 1,
                    path: Vec::new(),
                    prefix: Vec::new(),
                    ei: 0,
                    at: 0,
                };
                return (next, 1 + skipped);
            };
            if at as usize >= path.len() {
                ei += 1;
                at = stride;
                continue;
            }
            let x = path[at as usize].pos.x;
            if x < self.lo || x > self.hi {
                at += stride;
                skipped += 1;
                continue;
            }
            let mut balls = vec![path[at as usize]];
            apply_effect(&mut balls, e);
            let mut best = prefix[at as usize];
            let mut cost = skipped;
            for mut b in balls {
                for _ in at..tune::FLIGHT_MAX {
                    fly(&mut b, world.wind);
                    cost += fc;
                    best = best.min(self.gap(&b));
                    if b.pos.y - b.radius < world.terrain.height_at(b.pos.x) {
                        break;
                    }
                }
            }
            self.ranked.push((
                best,
                Plan {
                    release,
                    effect: Some((e, at)),
                },
            ));
            let next = Stage::Quick {
                ti,
                path,
                prefix,
                ei,
                at: at + stride,
            };
            return (next, cost.max(1));
        }
    }

    /// The whole ticks are flown: queue the sixteenths within a tick either
    /// side of the best few plain shots (R1.2100).
    fn refine(&mut self) {
        self.coarse = self.ticks.len();
        let mut plain: Vec<(i64, u32)> = self
            .ranked
            .iter()
            .filter(|(_, p)| p.effect.is_none())
            .map(|(d, p)| (*d, p.release))
            .collect();
        plain.sort();
        let (lo, hi) = (RELEASE_PARTS, tune::RELEASE_SCAN_MAX * RELEASE_PARTS);
        let mut fine: Vec<u32> = Vec::new();
        for &(_, r) in plain.iter().take(self.a.refine) {
            for k in 1..RELEASE_PARTS {
                for t in [r.saturating_sub(k), r + k] {
                    if t >= lo && t <= hi && !fine.contains(&t) && !self.ticks.contains(&t) {
                        fine.push(t);
                    }
                }
            }
        }
        fine.sort();
        self.ticks.extend(fine);
    }

    /// The quick pass is over: rank its plans and pick the ones to verify.
    fn rank(&mut self) -> Stage {
        let avoid = &self.avoid;
        let ranked = &mut self.ranked;
        // closest first; plain shots before effects, earlier ticks first on ties
        ranked.retain(|(_, p)| !avoid.contains(p));
        ranked.sort_by_key(|(d, p)| (*d, p.effect.is_some(), p.release, p.effect.map(|e| e.1)));
        let mut check: Vec<Plan> = ranked.iter().take(self.a.verified.max(1)).map(|r| r.1).collect();
        if self.a.each_effect {
            // the best plan with each effect too: the quick pass can't see walls
            for &e in &self.effects {
                if let Some((_, p)) = ranked.iter().find(|(_, p)| p.effect.map(|x| x.0) == Some(e)) {
                    if !check.contains(p) {
                        check.push(*p);
                    }
                }
            }
        }
        Stage::Verify {
            check,
            ci: 0,
            run: None,
            best: None,
        }
    }

    fn verify(
        &mut self,
        world: &World,
        check: Vec<Plan>,
        mut ci: usize,
        run: Option<PlayOut>,
        mut best: Option<(i64, Plan)>,
    ) -> (Stage, u64) {
        let mut run = match run {
            Some(r) => r,
            None if ci < check.len() => PlayOut::new(self.s, check[ci], &self.targets),
            None => return (Stage::Done(self.choose(best)), 1),
        };
        let cost = run.advance(world);
        let run = match run.outcome() {
            Some(o) => {
                let score = o.score();
                if best.is_none_or(|(b, _)| score > b) {
                    best = Some((score, check[ci]));
                }
                ci += 1;
                None
            }
            None => Some(run),
        };
        (Stage::Verify { check, ci, run, best }, cost)
    }

    /// The best verified plan, then the deliberate error.
    fn choose(&self, best: Option<(i64, Plan)>) -> Plan {
        let a = &self.a;
        let mut plan = best.map(|(_, p)| p).unwrap_or(Plan {
            release: self.ticks.first().copied().unwrap_or(RELEASE_PARTS),
            effect: None,
        });
        let mut rng = Rng::new(self.seed);
        if a.miss_ticks > 0 && rng.chance(a.miss_chance.0, a.miss_chance.1) {
            let mut off = rng.between(1, a.miss_ticks);
            if rng.chance(1, 2) {
                off = -off;
            }
            // the miss stays in whole ticks
            let r = plan.release as i32 + off * RELEASE_PARTS as i32;
            plan.release = r.max(0) as u32;
        }
        plan
    }
}

/// Fires `plan` in the live world and steps it `ticks` times, applying the
/// air effect on time. For tests and for play/ to copy the convention.
pub fn fire_and_step(world: &mut World, s: &Shooter, plan: Plan, ticks: u32) -> Vec<Event> {
    let l = trebuchet_launch_at(s.base, plan.release, s.power, s.facing_left);
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
