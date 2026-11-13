//! Bot intelligence tests: small worlds, one behavior each.

use super::aim::*;
use crate::game::flow::Difficulty;
use crate::physics::*;

const BASE: Vec2 = Vec2::ints(4, 0);
/// A plot-like distance from the trebuchet: the real terrain puts the plot
/// front 60-80 columns out (R1.3700), where plain shots come in at a slant.
const FAR: i64 = 4 + 75;
/// Ticks a far shot is followed: long enough for a flat throw to land.
const FAR_FLIGHT: u32 = 400;

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
    let mut w = World::new(Terrain::flat(FAR as usize + 60, Fx::ZERO));
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
        fire_and_step(w, &shooter(), plan, FAR_FLIGHT);
        if targets.iter().all(|t| !w.bodies[t.body].alive) {
            return Some(n);
        }
    }
    None
}

#[test]
fn ai_hard_bot_hits_a_lone_royal_within_three_shots() {
    // plot distances: the sling trebuchet cannot lob short (R1.1601)
    for x in [FAR - 20, FAR, FAR + 30] {
        let (mut w, targets) = lone_royal(x, Fx::ZERO);
        let n = shots_to_kill(&mut w, &targets, &[], Difficulty::Hard, 3);
        assert!(n.is_some(), "royal at {x} survived three shots");
    }
}

#[test]
fn ai_hard_bot_compensates_for_wind() {
    let wind = Fx::ratio(1, 4000);
    let (calm, targets) = lone_royal(FAR, Fx::ZERO);
    let calm_plan = plan_shot(&calm, &shooter(), &targets, &[], Difficulty::Hard, 1);
    // the calm plan, fired into the wind, misses
    let (mut windy, _) = lone_royal(FAR, wind);
    fire_and_step(&mut windy, &shooter(), calm_plan, FAR_FLIGHT);
    assert!(windy.bodies[targets[0].body].alive, "wind made no difference");
    // the bot aiming in the wind hits
    let (mut windy, _) = lone_royal(FAR, wind);
    assert!(shots_to_kill(&mut windy, &targets, &[], Difficulty::Hard, 3).is_some());
}

#[test]
fn ai_drop_effect_reaches_a_royal_down_a_well() {
    // a royal at the bottom of a two-unit well four units deep, a plot's
    // distance out: plain shots come in at a slant and hit the walls; a drop
    // falls straight in (one unit wide and three deep until R1.3702 made the
    // shot one unit across)
    let kills = |effects: &[AirEffect]| {
        let mut w = World::new(Terrain::flat(FAR as usize + 60, Fx::ZERO));
        let floor = w
            .add_beam(Vec2::ints(FAR, 0), Vec2::ints(FAR + 2, 0), Material::Metal)
            .unwrap();
        // each wall two layers thick, so a shot rolling along the ground
        // cannot pass between ticks (R1.1306)
        for x in [FAR, FAR + 2] {
            let outer = if x == FAR { -Fx::HALF } else { Fx::HALF };
            for y in 0..4 {
                for dx in [Fx::ZERO, outer] {
                    let at = |y: i64| Vec2::new(Fx::from_int(x) + dx, Fx::from_int(y));
                    w.add_beam(at(y), at(y + 1), Material::Metal).unwrap();
                }
            }
        }
        let body = w.add_body(floor, Fx::HALF, Fx::ratio(1, 4));
        for _ in 0..=TUNE.settle_ticks {
            w.step();
        }
        let targets = [Target { body, value: 1 }];
        let plan = plan_shot(&w, &shooter(), &targets, effects, Difficulty::Hard, 1);
        fire_and_step(&mut w, &shooter(), plan, FAR_FLIGHT);
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
            .map(|s| plan_shot(&w, &shooter(), &targets, &[], d, s).release)
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
        have: &[],
        plot,
        level,
        gold,
        stock: Stock::default(),
        royals,
        buildings: &[],
        difficulty: Difficulty::Normal,
        round: 1,
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
    // with more than one floor, the first royal goes on an upper floor
    // under a roof: some segment spans its column higher up. (Seed 5's
    // castle on the hillier terrain is one deck on posts, so look for a
    // seed with floors at two heights; R1.1307.)
    let floors = |p: &CastlePlan| {
        let mut ys: Vec<Fx> = p
            .segments
            .iter()
            .filter(|s| s.a.y == s.b.y)
            .map(|s| s.a.y)
            .collect();
        ys.sort();
        ys.dedup();
        ys.len()
    };
    let plan = (1..=12u64)
        .map(|seed| plan_castle(&ask(&Plot::generate(seed), 6, 120, 3, seed)))
        .find(|p| floors(p) >= 2 && p.royals.len() == 3)
        .expect("some seed's castle has two floors");
    let first = plan.royals[0].at;
    let low = plan.segments.iter().flat_map(|s| [s.a.y, s.b.y]).min().unwrap();
    let roofed = plan
        .segments
        .iter()
        .any(|s| s.a.y.min(s.b.y) > first.y && s.a.x.min(s.b.x) <= first.x && first.x <= s.a.x.max(s.b.x));
    assert!(
        first.y > low + Fx::from_int(2) - Fx::HALF,
        "{first:?} over {low:?}"
    );
    assert!(roofed, "{first:?} has no roof");
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
        flat: false,
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
            have: &[],
            plot: &me.terrain,
            level: me.purse.level,
            gold: me.purse.gold,
            stock: Stock::default(),
            royals: 1,
            buildings: &[],
            difficulty: Difficulty::Hard,
            round: 1,
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
    play.set_shot_planner(super::planner::plan_siege_job);
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

/// Royals of `p` with a solid segment over their heads, and beams placed.
fn cover(play: &Play, p: u8) -> (usize, usize) {
    use crate::play::castle::{BodyOwner, Geo};
    let pl = &play.rules.players[p as usize];
    let cw = play.castles[p as usize].world(pl, true);
    let w = &cw.world;
    let covered = cw
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, BodyOwner::Royal(_)))
        .filter(|(i, _)| {
            let b = w.bodies[*i].pos;
            w.segments.iter().any(|s| {
                let (a, c) = (w.nodes[s.a].pos, w.nodes[s.b].pos);
                a.x.min(c.x) < b.x && b.x < a.x.max(c.x) && a.y.min(c.y) > b.y
            })
        })
        .count();
    let beams = play.castles[p as usize]
        .geo
        .values()
        .filter(|g| matches!(g, Geo::Beam { .. }))
        .count();
    (covered, beams)
}

/// Royals of `p` standing in the castle (placed on the plot).
fn royals_placed(play: &Play, p: u8) -> usize {
    use crate::play::castle::BodyOwner;
    let pl = &play.rules.players[p as usize];
    let cw = play.castles[p as usize].world(pl, true);
    cw.bodies
        .iter()
        .filter(|o| matches!(o, BodyOwner::Royal(_)))
        .count()
}

/// Rounds 1-8 of a Normal bot on `seed`: (round, beams, covered, placed)
/// per build phase, each castle checked on the static test.
fn bot_rounds(seed: u64) -> Vec<(u32, usize, usize, usize)> {
    use super::build::bot_turn;
    use crate::game::flow::Phase;
    let mut play = match_with(vec![Seat::Human, Seat::Bot(Difficulty::Normal)], seed);
    let mut seen = Vec::new();
    let mut last = (0, Phase::Lobby);
    let mut n = 0;
    while play.rules.round <= 8 && play.rules.phase != Phase::Results {
        let now = (play.rules.round, play.rules.phase);
        if now != last && now.1 == Phase::Build {
            for input in bot_turn(&play, 1) {
                let _ = play.apply(1, input);
            }
            let pl = &play.rules.players[1];
            let cw = play.castles[1].world(pl, false);
            let r = cw.world.static_test(crate::physics::TUNE.settle_ticks + 120);
            assert!(
                r.broken.is_empty(),
                "seed {seed} round {}: {} broke",
                now.0,
                r.broken.len()
            );
            let (covered, beams) = cover(&play, 1);
            seen.push((now.0, beams, covered, royals_placed(&play, 1)));
        }
        last = now;
        let _ = play.apply(0, Input::Ready);
        let _ = play.apply(1, Input::Ready);
        play.tick();
        n += 1;
        assert!(n < 200_000);
    }
    seen
}

#[test]
fn ai_bot_castle_grows_every_round_and_stands() {
    // (round, segments, covered, placed) per seed are in the failure message
    for seed in 1u64..=6 {
        let seen = bot_rounds(seed);
        assert!(seen.len() >= 8, "seed {seed}: {seen:?}");
        // a castle from round 1, never smaller, and much bigger by round 8
        assert!(seen[0].1 >= 4, "seed {seed}: {seen:?}");
        assert!(seen.windows(2).all(|w| w[1].1 >= w[0].1), "seed {seed}: {seen:?}");
        assert!(seen[7].1 >= 2 * seen[0].1, "seed {seed}: {seen:?}");
        // every placed royal under cover every round, hills or not (R1.1453, R1.2300)
        for s in &seen {
            assert!(s.3 >= 1, "seed {seed}: {seen:?}");
            assert!(s.2 >= s.3, "seed {seed} round {}: {seen:?}", s.0);
        }
    }
}

#[test]
fn ai_siege_targets_carry_each_royals_damage() {
    let play = match_with(vec![Seat::Human, Seat::Human], 2);
    let plot = play.rules.players[1].terrain.clone();
    let mut w = crate::physics::World::new(super::units::physics_terrain(&plot));
    let body = |x: i64| crate::physics::Body {
        pos: crate::physics::Vec2::ints(x, 30),
        prev: crate::physics::Vec2::ints(x, 30),
        radius: Fx::ratio(1, 4),
        tall: Fx::ZERO,
        attached: None,
        peak_y: Fx::from_int(30),
        alive: true,
    };
    w.bodies = vec![body(150), body(160), body(170)];
    w.bodies[2].alive = false;
    // a Squire (1), a King (10), and a dead Knight left out
    let royals = [(0usize, 1u32), (1, 10), (2, 2)];
    let v = crate::play::siege::SiegeView {
        world: &w,
        plot: &plot,
        base: crate::physics::Vec2::ints(4, 0),
        power: 0,
        ammo: &[],
        air: &[],
        difficulty: None,
        royals: &royals,
    };
    let t: Vec<(usize, u32)> = super::planner::royal_targets(&v)
        .iter()
        .map(|t| (t.body, t.value))
        .collect();
    assert_eq!(t, vec![(0, 1), (1, 10)]);
}

/// The sliced siege planner, stepped a slice at a time and copied midway
/// (as an online rollback copies the match), plans the same shots as
/// planning them all at once, and no slice costs more than it said.
#[test]
fn ai_botplan_sliced_siege_matches_planning_all_at_once() {
    use super::planner::{plan_siege, SiegePlanner};
    use crate::game::upgrades::AirKind;
    use crate::play::siege::{Ammo, PlanJob, SiegeView};
    let (w, targets) = lone_royal(FAR, Fx::ZERO);
    let plot = crate::terrain::Plot::generate(1);
    let ammo = [(Ammo::Normal, 2u32)];
    let air = [AirKind::Drop];
    let royals = [(targets[0].body, 1u32)];
    let v = SiegeView {
        world: &w,
        plot: &plot,
        base: BASE,
        power: 0,
        ammo: &ammo,
        air: &air,
        difficulty: Some(Difficulty::Normal),
        royals: &royals,
    };
    let whole = plan_siege(&v);
    assert_eq!(whole.len(), 2);
    let mut job: Box<dyn PlanJob> = Box::new(SiegePlanner::new(&v));
    let mut got = Vec::new();
    let mut slices = 0;
    while job.next_cost() > 0 {
        let need = job.next_cost();
        let cost = job.advance();
        assert!(cost <= need, "slice {slices} cost {cost} over {need}");
        slices += 1;
        if slices == 40 {
            job = job.clone();
        }
        got.extend(job.take_ready());
    }
    assert!(slices > 40);
    assert_eq!(got, whole);
}

/// Through the match: the bot's shots come in while the siege runs, each
/// swings no sooner than the tick it was ready, and the bot never swings
/// with no shot planned.
#[test]
fn ai_botplan_bot_waits_for_each_planned_shot() {
    let mut play = match_with(vec![Seat::Human, Seat::Bot(Difficulty::Hard)], 3);
    play.set_shot_planner(super::planner::plan_siege_job);
    let plot = play.rules.terrain.clone();
    let col = plot.buildable(1).start + 3;
    let step = crate::terrain::CELL / 4;
    let at = crate::play::units::GridPt::new(col as i32 * 4, (plot.heights[col] + step / 2).div_euclid(step));
    play.apply(0, Input::PlaceRoyal { index: 0, at }).unwrap();
    play.apply(0, Input::Ready).unwrap();
    let mut seen = 0;
    let mut n = 0;
    while play.last_result.is_none() {
        play.tick();
        n += 1;
        assert!(n < 20_000, "stuck in {:?}", play.rules.phase);
        let Some(s) = &play.siege else { continue };
        let f = s.fields.iter().find(|f| f.attacker == 1).unwrap();
        let plan = &f.shooter.plan;
        if plan.is_empty() {
            assert!(f.shooter.swing_from.is_none());
        }
        // the tick that just ran was s.tick - 1
        for p in &plan[seen..] {
            assert!(
                p.swing_at + 1 >= s.tick,
                "swing {} before ready {}",
                p.swing_at,
                s.tick
            );
        }
        seen = plan.len();
    }
    assert!(seen > 0);
}

/// Each royal of `p`: the head room from its feet (the body's bottom) to
/// the underside of the lowest level beam over it, and the side room from
/// its middle to the nearest upright beside it within that height.
fn figure_room(play: &Play, p: u8) -> Vec<(Fx, Fx)> {
    use crate::play::castle::BodyOwner;
    use crate::play::tune::FIGURE_TALL;
    let pl = &play.rules.players[p as usize];
    let cw = play.castles[p as usize].world(pl, true);
    let w = &cw.world;
    let half = |s: &crate::physics::world::Segment| TUNE.material(s.material).thickness / 2;
    cw.bodies
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, BodyOwner::Royal(_)))
        .map(|(i, _)| {
            let b = &w.bodies[i];
            let foot = b.pos.y - b.radius;
            let ends = |s: &crate::physics::world::Segment| (w.nodes[s.a].pos, w.nodes[s.b].pos);
            let head = w
                .segments
                .iter()
                .filter(|s| !s.broken)
                .filter_map(|s| {
                    let (a, c) = ends(s);
                    (a.y == c.y && a.x.min(c.x) < b.pos.x && b.pos.x < a.x.max(c.x) && a.y > b.pos.y)
                        .then(|| a.y - half(s) - foot)
                })
                .min()
                .unwrap_or(Fx::from_int(1_000));
            let side = w
                .segments
                .iter()
                .filter(|s| !s.broken)
                .filter_map(|s| {
                    let (a, c) = ends(s);
                    (a.x == c.x && a.y.max(c.y) > foot && a.y.min(c.y) < foot + FIGURE_TALL)
                        .then(|| (a.x - b.pos.x).abs() - half(s))
                })
                .min()
                .unwrap_or(Fx::from_int(1_000));
            (head, side)
        })
        .collect()
}

#[test]
fn ai_bot_royal_figures_fit_under_their_cover() {
    use super::build::bot_turn;
    use crate::game::flow::Phase;
    use crate::play::tune::{FIGURE_TALL, FIGURE_WIDE, HEAD_ROOM};
    // the drawn figure fits the head room the cover rules allow (R1.2400)
    assert!(FIGURE_TALL <= HEAD_ROOM);
    for seed in 1u64..=3 {
        let mut play = match_with(vec![Seat::Human, Seat::Bot(Difficulty::Normal)], seed);
        let mut last = (0, Phase::Lobby);
        let mut checked = 0;
        let mut n = 0;
        while play.rules.round <= 4 && play.rules.phase != Phase::Results {
            let now = (play.rules.round, play.rules.phase);
            if now != last && now.1 == Phase::Build {
                for input in bot_turn(&play, 1) {
                    let _ = play.apply(1, input);
                }
                for (head, side) in figure_room(&play, 1) {
                    assert!(
                        head >= FIGURE_TALL,
                        "seed {seed} round {}: head room {head:?}",
                        now.0
                    );
                    assert!(
                        side + side >= FIGURE_WIDE,
                        "seed {seed} round {}: side room {side:?}",
                        now.0
                    );
                    checked += 1;
                }
            }
            last = now;
            let _ = play.apply(0, Input::Ready);
            let _ = play.apply(1, Input::Ready);
            play.tick();
            n += 1;
            assert!(n < 100_000);
        }
        assert!(checked >= 4, "seed {seed}: {checked} royals checked");
    }
}
