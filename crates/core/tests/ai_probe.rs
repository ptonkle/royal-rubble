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
