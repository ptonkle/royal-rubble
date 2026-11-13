//! AI probe: decision time of the aimer against a castle of about 200
//! segments, recorded in MEASUREMENTS.md. Ignored by default; run with
//! `cargo test --release --test ai_probe -- --ignored --nocapture`.

use std::time::Instant;
use tool_core::ai::aim::{plan_shot, Shooter, Target};
use tool_core::game::flow::Difficulty;
use tool_core::physics::*;

/// The physics probe's castle (stone columns, wood floors and braces) with
/// three royals on its top floor, settled.
fn castle() -> (World, Vec<Target>) {
    let mut w = World::new(Terrain::flat(80, Fx::ZERO));
    let (cols, rows, x0) = (13, 6, 14);
    let mut top = Vec::new();
    for r in 0..rows {
        for c in 0..=cols {
            w.add_beam(Vec2::ints(x0 + c, r), Vec2::ints(x0 + c, r + 1), Material::Stone)
                .unwrap();
        }
        for c in 0..cols {
            let s = w
                .add_beam(
                    Vec2::ints(x0 + c, r + 1),
                    Vec2::ints(x0 + c + 1, r + 1),
                    Material::Wood,
                )
                .unwrap();
            if r == rows - 1 {
                top.push(s);
            }
            if (r + c) % 2 == 0 {
                w.add_beam(
                    Vec2::ints(x0 + c, r),
                    Vec2::ints(x0 + c + 1, r + 1),
                    Material::Wood,
                )
                .unwrap();
            }
        }
    }
    let targets = [1, 6, 11]
        .iter()
        .map(|&c| Target {
            body: w.add_body(top[c], Fx::HALF, Fx::ratio(1, 4)),
            value: 1,
        })
        .collect();
    for _ in 0..=TUNE.settle_ticks {
        w.step();
    }
    (w, targets)
}

#[test]
#[ignore]
fn probe_decision_time() {
    let (w, targets) = castle();
    let s = Shooter {
        base: Vec2::ints(4, 0),
        facing_left: false,
        power: 0,
        radius: Fx::ratio(1, 4),
        mass: Fx::ONE,
        fire: false,
        spread: false,
    };
    println!("castle segments {}", w.segments.len());
    for (name, effects) in [
        ("plain", vec![]),
        ("drop", vec![AirEffect::Drop]),
        ("drop+split", vec![AirEffect::Drop, AirEffect::Split]),
    ] {
        for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            let mut worst = 0u128;
            let mut total = 0u128;
            let runs = 5u64;
            for seed in 0..runs {
                let t0 = Instant::now();
                let plan = plan_shot(&w, &s, &targets, &effects, d, seed);
                let us = t0.elapsed().as_micros();
                worst = worst.max(us);
                total += us;
                if seed == 0 {
                    println!("{name} {d:?}: {plan:?}");
                }
            }
            println!("{name} {d:?}: mean {} us, worst {worst} us", total / runs as u128);
        }
    }
}

/// A whole siege's plan (every shot is planned at the siege start).
#[test]
#[ignore]
fn probe_siege_plan_time() {
    use tool_core::game::upgrades::AirKind;
    use tool_core::play::siege::{Ammo, SiegeView};
    let (w, _) = castle();
    let plot = tool_core::terrain::Plot::generate(1);
    for (shots, air) in [(5u32, vec![]), (5, vec![AirKind::Drop])] {
        for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            let ammo = [(Ammo::Normal, shots)];
            let v = SiegeView {
                world: &w,
                plot: &plot,
                base: Vec2::ints(4, 0),
                power: 0,
                ammo: &ammo,
                air: &air,
                difficulty: Some(d),
                royals: &[],
            };
            let t0 = Instant::now();
            let plan = tool_core::ai::planner::plan_siege(&v);
            let ms = t0.elapsed().as_millis();
            println!(
                "siege {shots} shots air {air:?} {d:?}: {ms} ms, {} planned",
                plan.len()
            );
        }
    }
}

/// Seven bots planning a whole siege each on the probe castle: the worst
/// tick at the siege start before (every shot in one call) and after (a
/// budget of work a tick, `play::plan_slice`), R1.2000.
#[test]
#[ignore]
fn probe_botplan_worst_tick() {
    use tool_core::ai::planner::{plan_siege, SiegePlanner};
    use tool_core::game::upgrades::AirKind;
    use tool_core::play::siege::{Ammo, PlanJob, SiegeView};
    use tool_core::play::tune::BOT_PLAN_BUDGET;
    let (w, _) = castle();
    let plot = tool_core::terrain::Plot::generate(1);
    let ammo = [(Ammo::Normal, 5u32)];
    let bots = 7;
    for (air, d) in [
        (vec![], Difficulty::Normal),
        (vec![AirKind::Drop], Difficulty::Hard),
        (vec![AirKind::Drop, AirKind::Split], Difficulty::Hard),
    ] {
        let v = SiegeView {
            world: &w,
            plot: &plot,
            base: Vec2::ints(4, 0),
            power: 0,
            ammo: &ammo,
            air: &air,
            difficulty: Some(d),
            royals: &[],
        };
        let t0 = Instant::now();
        for _ in 0..bots {
            plan_siege(&v);
        }
        let before = t0.elapsed().as_micros();
        let mut jobs: Vec<SiegePlanner> = (0..bots).map(|_| SiegePlanner::new(&v)).collect();
        let (mut worst, mut worst_units, mut total, mut units) = (0u128, 0u64, 0u128, 0u64);
        let mut ticks = 0u32;
        // the tick each bot's first and last shot was ready
        let mut first: Vec<Option<u32>> = vec![None; bots];
        let mut last = vec![0u32; bots];
        while jobs.iter().any(|j| j.next_cost() > 0) {
            let t = Instant::now();
            let mut refs: Vec<&mut (dyn PlanJob + 'static)> = jobs
                .iter_mut()
                .map(|j| j as &mut (dyn PlanJob + 'static))
                .collect();
            let spent = tool_core::play::plan_slice(&mut refs, BOT_PLAN_BUDGET);
            let us = t.elapsed().as_micros();
            for (i, j) in jobs.iter_mut().enumerate() {
                if !j.take_ready().is_empty() {
                    first[i].get_or_insert(ticks);
                    last[i] = ticks;
                }
            }
            if us > worst {
                worst = us;
                worst_units = spent;
            }
            total += us;
            units += spent;
            ticks += 1;
        }
        println!(
            "{bots} bots {d:?} air {air:?}: all at once {before} us in one tick; sliced worst tick {worst} us ({worst_units} units), mean {} us, {ticks} ticks, {} ns/unit, first shots ready by tick {:?}, last {:?}",
            total / ticks as u128,
            total * 1000 / units.max(1) as u128,
            first.iter().map(|f| f.unwrap_or(0)).max().unwrap_or(0),
            last.iter().max().unwrap_or(&0),
        );
    }
}
