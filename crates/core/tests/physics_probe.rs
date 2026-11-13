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

/// A 200-segment castle: a braced grid wall.
pub fn castle() -> World {
    let mut w = World::new(Terrain::flat(80, Fx::ZERO));
    let (cols, rows) = (12, 6);
    let x0 = 30;
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
fn probe_castle_breaks() {
    let mut w = castle();
    let start: Vec<Vec2> = w.nodes.iter().map(|n| n.pos).collect();
    let mut c = w.clone();
    for t in 0..120 {
        c.step();
        if t % 20 == 19 {
            let sag = c
                .nodes
                .iter()
                .zip(&start)
                .map(|(n, s)| (s.y - n.pos.y).0)
                .max()
                .unwrap();
            let len_err = c
                .segments
                .iter()
                .map(|s| (((c.nodes[s.b].pos - c.nodes[s.a].pos).len() - s.rest).0.abs() * 1000) / s.rest.0)
                .max()
                .unwrap();
            println!(
                "t{t} max sag {} per mille of a unit, max length error {len_err} per mille",
                sag * 1000 / 65536
            );
        }
    }
    for t in 0..240 {
        for e in w.step() {
            if let Event::Broke { segment, cause } = e {
                let s = &w.segments[segment];
                let (a, b) = (w.nodes[s.a].pos, w.nodes[s.b].pos);
                println!(
                    "t{t} seg {segment} {:?} {:?} ({},{})-({},{}) strain {}",
                    s.material,
                    cause,
                    a.x.0 >> 16,
                    a.y.0 >> 16,
                    b.x.0 >> 16,
                    b.y.0 >> 16,
                    s.strain.0 * 100 / 65536
                );
            }
        }
        if t < 4 {
            for i in [0usize, 13, 14] {
                let s = &w.segments[i];
                println!("t{t} seg{i} axial {} bend {}", s.axial.0, s.bend.0);
            }
        }
        if t == 30 || t == 59 || t == 100 || t == 200 {
            let m = w.segments.iter().map(|s| s.strain.0 * 100 / 65536).max();
            println!("t{t} max strain {:?}", m);
        }
    }
}

fn run_print(name: &str, mut w: World) {
    let top = w.nodes.len() - 1;
    let mut line = String::new();
    for t in 0..120 {
        let ev = w.step();
        if t % 10 == 0 || !ev.is_empty() {
            line += &format!(" t{t}:y{} ev{}", (w.nodes[top].pos.y.0 * 100) >> 16, ev.len());
        }
    }
    println!("{name}:{line}");
}

#[test]
#[ignore]
fn probe_small() {
    for mat in [Material::Wood, Material::Stone] {
        let mut w = World::new(Terrain::flat(80, Fx::ZERO));
        for k in 0..6 {
            w.add_beam(Vec2::ints(5, k), Vec2::ints(5, k + 1), mat).unwrap();
        }
        run_print(&format!("column {mat:?}"), w);
        let mut w = World::new(Terrain::flat(80, Fx::ZERO));
        w.add_beam(Vec2::ints(5, 0), Vec2::ints(5, 1), mat).unwrap();
        w.add_beam(Vec2::ints(6, 0), Vec2::ints(6, 1), mat).unwrap();
        w.add_beam(Vec2::ints(5, 1), Vec2::ints(6, 1), Material::Wood)
            .unwrap();
        run_print(&format!("frame {mat:?}"), w);
    }
}

#[test]
#[ignore]
fn probe_step_time() {
    let mut w = castle();
    let segs = w.segments.len();
    let r = w.static_test(240);
    println!("castle segments {segs}, static breaks {}", r.broken.len());
    for k in 0..5u32 {
        let l = trebuchet_launch(Vec2::ints(5, 0), 50 + k * 2, 6, false);
        w.fire(l, Fx::ratio(1, 3), Fx::from_int(4));
    }
    let ticks = 600;
    let start = Instant::now();
    let mut events = 0;
    for _ in 0..ticks {
        events += w.step().len();
    }
    let el = start.elapsed();
    let broken = w.segments.iter().filter(|s| s.broken).count();
    println!(
        "{ticks} ticks: {:?} total, {:?} per step; events {events}, broken {broken}, debris {}",
        el,
        el / ticks,
        w.debris.len()
    );
}
