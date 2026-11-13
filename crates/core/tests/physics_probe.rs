//! Physics probes: printed numbers for tuning, and the step-time measurement
//! recorded in MEASUREMENTS.md. Ignored by default; run with
//! `cargo test --release --test physics_probe -- --ignored --nocapture`.

use std::time::Instant;
use tool_core::physics::*;

fn tower() -> World {
    let mut w = World::new(Terrain::flat(80, Fx::ZERO));
    for (a, b) in [
        ((2, 0), (2, 1)),
        ((2, 1), (2, 2)),
        ((3, 0), (3, 1)),
        ((3, 1), (3, 2)),
        ((2, 1), (3, 1)),
        ((2, 2), (3, 2)),
        ((2, 0), (3, 1)),
        ((2, 1), (3, 2)),
    ] {
        w.add_beam(Vec2::ints(a.0, a.1), Vec2::ints(b.0, b.1), Material::Wood)
            .unwrap();
    }
    w
}

/// Max strain (percent) of each segment for wood cantilevers 1..4 long.
#[test]
#[ignore]
fn probe_cantilever_strain() {
    for n in 1..=4 {
        let mut w = tower();
        for k in 0..n {
            w.add_beam(Vec2::ints(3 + k, 2), Vec2::ints(4 + k, 2), Material::Wood)
                .unwrap();
        }
        let r = w.static_test(240);
        println!(
            "cantilever {n}: broken {:?} max {:?}",
            r.broken,
            r.max_strain.iter().map(|s| s.0 * 100 / 65536).collect::<Vec<_>>()
        );
    }
}

/// A castle of about 200 segments: stone columns, wood floors, wood braces.
fn castle() -> World {
    let mut w = World::new(Terrain::flat(80, Fx::ZERO));
    let (cols, rows, x0) = (13, 6, 30);
    for r in 0..rows {
        for c in 0..=cols {
            w.add_beam(Vec2::ints(x0 + c, r), Vec2::ints(x0 + c, r + 1), Material::Stone)
                .unwrap();
        }
        for c in 0..cols {
            w.add_beam(
                Vec2::ints(x0 + c, r + 1),
                Vec2::ints(x0 + c + 1, r + 1),
                Material::Wood,
            )
            .unwrap();
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
    w
}

#[test]
#[ignore]
fn probe_step_time() {
    let mut w = castle();
    let segs = w.segments.len();
    let r = w.static_test(240);
    println!("castle segments {segs}, static breaks {}", r.broken.len());
    // flat shots straight into the wall at five heights
    for k in 0..5i64 {
        let pos = Vec2::new(Fx::from_int(20), Fx::ONE + Fx::from_int(k));
        let vel = Vec2::new(Fx::ratio(1, 2), Fx::ZERO);
        w.fire(Launch { pos, vel }, Fx::ratio(1, 3), Fx::from_int(6));
    }
    let ticks = 600;
    let start = Instant::now();
    let mut worst = std::time::Duration::ZERO;
    let mut events = 0;
    for _ in 0..ticks {
        let t = Instant::now();
        events += w.step().len();
        worst = worst.max(t.elapsed());
    }
    let el = start.elapsed();
    let broken = w.segments.iter().filter(|s| s.broken).count();
    println!(
        "{ticks} ticks: {:?} total, {:?} mean step, {:?} worst step; events {events}, broken {broken}, debris {}",
        el,
        el / ticks,
        worst,
        w.debris.len()
    );
}
