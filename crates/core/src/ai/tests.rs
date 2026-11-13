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
fn ai_drop_effect_lands_a_shot_between_plain_landings() {
    // 17.0 sits between two plain landings (16.0 and 18.0); a drop puts the
    // shot straight down onto it in one shot
    let (mut w, targets) = lone_royal(17, Fx::ZERO);
    let plan = plan_shot(&w, &shooter(), &targets, &[AirEffect::Drop], Difficulty::Hard, 1);
    fire_and_step(&mut w, &shooter(), plan, 300);
    assert!(!w.bodies[targets[0].body].alive, "{plan:?}");
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
