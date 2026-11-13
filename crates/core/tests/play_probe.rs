//! Match glue probe: siege tick time with two real castles, recorded in
//! MEASUREMENTS.md. Ignored by default; run with
//! `cargo test --release --test play_probe -- --ignored --nocapture`.

use std::time::Instant;
use tool_core::game::flow::{Phase, Seat, Settings};
use tool_core::game::ledger::{Material, Piece};
use tool_core::play::units::GridPt;
use tool_core::play::{Input, Play};

const METAL: Piece = Piece::Segment(Material::Metal);

/// A metal lattice 20 columns wide and 3 floors high, braced: up to 174 segments.
fn lattice(play: &mut Play, p: u8) {
    let pl = &mut play.rules.players[p as usize];
    pl.purse.level = 9;
    pl.purse.gold = 100_000;
    let plot = pl.terrain.clone();
    let start = plot.buildable(9).start;
    let pt = |k: usize, f: i32| {
        let col = start + 2 + 2 * k;
        let g = (plot.heights[col] + 128).div_euclid(256);
        GridPt::new(col as i32 * 4, g + f * 8)
    };
    // a brace up a rising bay can be a step longer than metal allows: skipped
    let mut put = |a: GridPt, b: GridPt| {
        let _ = play.apply(p, Input::PlaceSegment { piece: METAL, a, b });
    };
    for k in 0..20 {
        for f in 0..3 {
            put(pt(k, f), pt(k, f + 1));
            if k + 1 < 20 {
                put(pt(k, f + 1), pt(k + 1, f + 1));
                put(pt(k, f), pt(k + 1, f + 1));
            }
        }
    }
}

#[test]
#[ignore]
fn probe_siege_tick_time() {
    let mut play = Play::new(Settings {
        seats: vec![Seat::Human, Seat::Human],
        wind: true,
        seed: 5,
    })
    .unwrap();
    play.start().unwrap();
    let t = Instant::now();
    lattice(&mut play, 0);
    lattice(&mut play, 1);
    println!(
        "building {} + {} segments by input: {:?}",
        play.castles[0].geo.len(),
        play.castles[1].geo.len(),
        t.elapsed()
    );
    let t = Instant::now();
    let strain = play.strain(0);
    println!(
        "strain (static test) of one castle: {:?}, {} pieces",
        t.elapsed(),
        strain.len()
    );
    for p in [0, 1] {
        play.apply(p, Input::Ready).unwrap();
    }
    let t = Instant::now();
    play.tick();
    assert_eq!(play.rules.phase, Phase::Siege);
    println!("siege start (2 worlds built): {:?}", t.elapsed());
    let mut times = Vec::new();
    while play.rules.phase == Phase::Siege || play.rules.phase == Phase::SiegeOver {
        let mut inputs = Vec::new();
        if let Some(s) = &play.siege {
            for f in &s.fields {
                let sh = &f.shooter;
                match sh.swing_from {
                    Some(t0) if s.tick - t0 >= 44 => {
                        inputs.push((f.attacker, Input::Release(tool_core::play::ON_TICK)))
                    }
                    Some(_) => {}
                    None if sh.shots_left() > 0 && s.tick % 90 == 0 => {
                        inputs.push((f.attacker, Input::Swing))
                    }
                    None if sh.shots_left() == 0 && !f.active() => inputs.push((f.attacker, Input::Ready)),
                    None => {}
                }
            }
        }
        let t = Instant::now();
        play.step(&inputs);
        times.push(t.elapsed().as_micros());
    }
    let n = times.len() as u128;
    let mean = times.iter().sum::<u128>() / n;
    let worst = times.iter().max().unwrap();
    let r = play.last_result.as_ref().unwrap();
    println!(
        "siege: {n} ticks, mean {mean} us, worst {worst} us; kills {:?}",
        r.castles
            .iter()
            .map(|c| c.royals_killed.len())
            .collect::<Vec<_>>()
    );
}
