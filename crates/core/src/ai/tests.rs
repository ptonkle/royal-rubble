//! Bot intelligence tests: small worlds, one behavior each.

use super::aim::*;
use crate::game::flow::Difficulty;
use crate::physics::*;

const BASE: Vec2 = Vec2::ints(4, 0);

fn shooter() -> Shooter {
    Shooter {
        base: BASE,
        facing_left: false,
        power: 0,
        radius: Fx::ratio(1, 4),
        mass: Fx::ONE,
        fire: false,
        spread: false,
    }
}

/// A flat plot with one royal standing on a one-unit wooden floor laid on
/// the ground, centered at `x`; settled, so nothing moves.
fn lone_royal(x: i64, wind: Fx) -> (World, Vec<Target>) {
    let mut w = World::new(Terrain::flat(80, Fx::ZERO));
    w.wind = wind;
    let floor = w
        .add_beam(
            Vec2::new(Fx::from_int(x) - Fx::HALF, Fx::ZERO),
            Vec2::new(Fx::from_int(x) + Fx::HALF, Fx::ZERO),
            Material::Wood,
        )
        .unwrap();
    let body = w.add_body(floor, Fx::HALF, Fx::ratio(1, 4));
    for _ in 0..=TUNE.settle_ticks {
        w.step();
    }
    (w, vec![Target { body, value: 1 }])
}

/// Shots the bot needs to kill every target, firing at most `max`.
fn shots_to_kill(
    w: &mut World,
    targets: &[Target],
    effects: &[AirEffect],
    d: Difficulty,
    max: u32,
) -> Option<u32> {
    for n in 1..=max {
        let plan = plan_shot(w, &shooter(), targets, effects, d, n as u64);
        fire_and_step(w, &shooter(), plan, 300);
        if targets.iter().all(|t| !w.bodies[t.body].alive) {
            return Some(n);
        }
    }
    None
}

#[test]
fn ai_hard_bot_hits_a_lone_royal_within_three_shots() {
    // 17 sits in a gap between plain landings (R1.902); see the drop test
    for x in [12, 21, 25] {
        let (mut w, targets) = lone_royal(x, Fx::ZERO);
        let n = shots_to_kill(&mut w, &targets, &[], Difficulty::Hard, 3);
        assert!(n.is_some(), "royal at {x} survived three shots");
    }
}

#[test]
fn ai_hard_bot_compensates_for_wind() {
    let wind = Fx::ratio(1, 4000);
    let (calm, targets) = lone_royal(17, Fx::ZERO);
    let calm_plan = plan_shot(&calm, &shooter(), &targets, &[], Difficulty::Hard, 1);
    // the calm plan, fired into the wind, misses
    let (mut windy, _) = lone_royal(17, wind);
    fire_and_step(&mut windy, &shooter(), calm_plan, 300);
    assert!(windy.bodies[targets[0].body].alive, "wind made no difference");
    // the bot aiming in the wind hits
    let (mut windy, _) = lone_royal(17, wind);
    assert!(shots_to_kill(&mut windy, &targets, &[], Difficulty::Hard, 3).is_some());
}

#[test]
fn ai_drop_effect_reaches_a_royal_down_a_well() {
    // a royal at the bottom of a one-unit well three units deep: plain shots
    // come in at a slant and hit the walls; a drop falls straight in
    let kills = |effects: &[AirEffect]| {
        let mut w = World::new(Terrain::flat(80, Fx::ZERO));
        let floor = w
            .add_beam(Vec2::ints(16, 0), Vec2::ints(17, 0), Material::Wood)
            .unwrap();
        for x in [16, 17] {
            for y in 0..3 {
                w.add_beam(Vec2::ints(x, y), Vec2::ints(x, y + 1), Material::Stone)
                    .unwrap();
            }
        }
        let body = w.add_body(floor, Fx::HALF, Fx::ratio(1, 4));
        for _ in 0..=TUNE.settle_ticks {
            w.step();
        }
        let targets = [Target { body, value: 1 }];
        let plan = plan_shot(&w, &shooter(), &targets, effects, Difficulty::Hard, 1);
        fire_and_step(&mut w, &shooter(), plan, 300);
        !w.bodies[body].alive
    };
    assert!(!kills(&[]), "a plain shot reached the bottom");
    assert!(kills(&[AirEffect::Drop]));
}

#[test]
fn ai_easy_misses_on_purpose_and_hard_never_does() {
    let (w, targets) = lone_royal(17, Fx::ZERO);
    let distinct = |d: Difficulty| {
        let mut ticks: Vec<u32> = (0..20u64)
            .map(|s| plan_shot(&w, &shooter(), &targets, &[], d, s).release_tick)
            .collect();
        ticks.sort();
        ticks.dedup();
        ticks.len()
    };
    // the same world every time: only a deliberate miss changes the tick
    assert!(distinct(Difficulty::Easy) >= 3);
    assert_eq!(distinct(Difficulty::Hard), 1);
}

// ---- Castle plans ----------------------------------------------------------

use super::build::*;
use crate::game::ledger::{BuildingKind, Material as Mat, Stock};
use crate::terrain::{Plot, SpotKind};

fn ask(plot: &Plot, level: u8, gold: u32, royals: usize, seed: u64) -> BuildAsk<'_> {
    BuildAsk {
        plot,
        level,
        gold,
        stock: Stock::default(),
        royals,
        buildings: &[],
        difficulty: Difficulty::Normal,
        seed,
    }
}

#[test]
fn ai_castle_plans_stand_up_across_seeds_and_levels() {
    for seed in 0..30u64 {
        let plot = Plot::generate(seed);
        for level in 1..=9u8 {
            let gold = 10 + 12 * level as u32;
            let plan = plan_castle(&ask(&plot, level, gold, 2 + level as usize, seed));
            assert!(
                plan.segments.len() >= 6,
                "seed {seed} level {level}: {} segments",
                plan.segments.len()
            );
            let (w, royals) = to_world(&plan, &plot);
            assert_eq!(royals.len(), plan.royals.len());
            let r = w.static_test(240);
            assert!(
                r.broken.is_empty(),
                "seed {seed} level {level}: {} broke of {}",
                r.broken.len(),
                plan.segments.len()
            );
        }
    }
}

#[test]
fn ai_castle_plans_fit_the_span_height_budget_and_unlocks() {
    for seed in 0..30u64 {
        let plot = Plot::generate(seed);
        for level in 1..=9u8 {
            let gold = 4 + 9 * level as u32;
            let plan = plan_castle(&ask(&plot, level, gold, 3, seed));
            assert!(
                plan.gold <= gold,
                "seed {seed} level {level}: spent {} of {gold}",
                plan.gold
            );
            let span = plot.buildable(level);
            let (lo, hi) = (super::units::col_x(span.start), super::units::col_x(span.end));
            let roof = super::units::cells_to_fx(
                plot.heights[span.start..span.end].iter().copied().max().unwrap() + Plot::build_height(level),
            );
            for s in &plan.segments {
                for p in [s.a, s.b] {
                    assert!(
                        p.x >= lo && p.x <= hi,
                        "seed {seed} level {level}: x {:?} outside",
                        p.x
                    );
                    assert!(p.y <= roof, "seed {seed} level {level}: too tall");
                }
                let unlock = match s.material {
                    Mat::Wood => 1,
                    Mat::Stone => 4,
                    Mat::Metal => 7,
                };
                assert!(
                    level >= unlock,
                    "seed {seed} level {level}: {:?} locked",
                    s.material
                );
            }
        }
    }
}

#[test]
fn ai_castle_builds_stone_from_level_four_when_it_can_afford_it() {
    let plot = Plot::generate(3);
    let has = |level: u8, m: Mat| {
        plan_castle(&ask(&plot, level, 200, 2, 3))
            .segments
            .iter()
            .any(|s| s.material == m)
    };
    assert!(!has(3, Mat::Stone));
    assert!(has(4, Mat::Stone));
    assert!(has(7, Mat::Metal));
}

#[test]
fn ai_castle_royals_keep_their_spacing_on_upper_floors() {
    let plot = Plot::generate(5);
    let plan = plan_castle(&ask(&plot, 6, 120, 3, 5));
    assert_eq!(plan.royals.len(), 3);
    let spacing = super::units::royal_spacing();
    for (i, a) in plan.royals.iter().enumerate() {
        for b in &plan.royals[i + 1..] {
            assert!((a.at - b.at).len() >= spacing, "{:?} {:?}", a.at, b.at);
        }
    }
    // the first royal goes on an upper floor, under the roof
    let first = plan.royals[0].at.y;
    let ys: Vec<Fx> = plan.segments.iter().flat_map(|s| [s.a.y, s.b.y]).collect();
    let (low, roof) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
    assert!(
        first > low + Fx::from_int(2) - Fx::HALF && first < roof,
        "{first:?} in {low:?}..{roof:?}"
    );
}

#[test]
fn ai_castle_puts_a_producer_on_its_own_spot() {
    let plot = (0..200u64)
        .map(Plot::generate)
        .find(|p| {
            let span = p.buildable(2);
            p.spots
                .iter()
                .any(|s| s.kind == SpotKind::Forest && span.contains(s.cols.start))
        })
        .expect("some seed has a forest open at level 2");
    let mut a = ask(&plot, 2, 30, 1, 1);
    a.buildings = &[BuildingKind::Carpenter];
    let plan = plan_castle(&a);
    assert_eq!(plan.buildings.len(), 1);
    let col = plan.buildings[0].col;
    assert!(plot
        .spots
        .iter()
        .any(|s| s.kind == SpotKind::Forest && s.cols.contains(col)));
    assert!(plan.gold <= 30);
}

// ---- Through the match (play/) ---------------------------------------------

use crate::game::flow::{Seat, Settings};
use crate::play::{Input, Play};

fn match_with(seats: Vec<Seat>, seed: u64) -> Play {
    let mut p = Play::new(Settings {
        seats,
        wind: false,
        seed,
    })
    .unwrap();
    p.start().unwrap();
    p
}

#[test]
fn ai_castle_plan_goes_through_the_match_inputs() {
    for seed in 0..5u64 {
        let mut play = match_with(vec![Seat::Human, Seat::Human], seed);
        play.rules.players[0].purse.gold = 60;
        let me = &play.rules.players[0];
        let plan = plan_castle(&BuildAsk {
            plot: &me.terrain,
            level: me.purse.level,
            gold: me.purse.gold,
            stock: Stock::default(),
            royals: 1,
            buildings: &[],
            difficulty: Difficulty::Hard,
            seed,
        });
        for input in to_inputs(&plan, &[0]) {
            assert_eq!(play.apply(0, input), Ok(()), "seed {seed}: {input:?}");
        }
    }
}

#[test]
fn ai_planner_kills_a_royal_through_the_match() {
    let mut play = match_with(vec![Seat::Human, Seat::Bot(Difficulty::Hard)], 3);
    play.set_shot_planner(super::planner::plan_siege);
    let plot = play.rules.terrain.clone();
    let col = plot.buildable(1).start + 3;
    // the ground at the column's left edge, to the nearest grid step
    let step = crate::terrain::CELL / 4;
    let at = crate::play::units::GridPt::new(col as i32 * 4, (plot.heights[col] + step / 2).div_euclid(step));
    play.apply(0, Input::PlaceRoyal { index: 0, at }).unwrap();
    play.apply(0, Input::Ready).unwrap();
    let mut n = 0;
    while play.last_result.is_none() {
        play.tick();
        n += 1;
        assert!(n < 20_000, "stuck in {:?}", play.rules.phase);
    }
    let r = play.last_result.clone().unwrap();
    let c = r.castles.iter().find(|c| c.owner == 0).unwrap();
    assert_eq!(c.royals_killed.len(), 1);
}
