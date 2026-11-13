use super::castle::BodyOwner;
use super::units::{self, GridPt, STEPS_PER_CELL};
use super::{Input, Play};
use crate::game::flow::{Phase, Seat, Settings};
use crate::game::ledger::{BuildingKind, Material, Piece};
use crate::game::royals::RoyalKind;
use crate::game::{tune as rules, Refusal};
use crate::physics::{trebuchet_launch, Fx, Launch, Vec2, World};
use crate::terrain::{Plot, SpotKind, CELL};

const WOOD: Piece = Piece::Segment(Material::Wood);
/// Terrain units in one grid step.
const STEP: i32 = CELL / STEPS_PER_CELL;

fn duel(seed: u64) -> Play {
    let mut p = Play::new(Settings {
        seats: vec![Seat::Human, Seat::Human],
        wind: false,
        seed,
    })
    .unwrap();
    p.start().unwrap();
    p
}

/// The ground at the left edge of column `col`, as a grid point.
fn ground(plot: &Plot, col: usize) -> GridPt {
    let h = plot.heights[col];
    GridPt::new(col as i32 * STEPS_PER_CELL, (h + STEP / 2).div_euclid(STEP))
}

/// A column `from` columns into the level-1 span. The ground is never quite
/// flat; a grid point is within half a step of it, inside the tolerance.
fn flat_col(plot: &Plot, from: usize) -> usize {
    plot.buildable(1).start + from
}

fn up(p: GridPt, cells: i32) -> GridPt {
    GridPt::new(p.x, p.y + cells * STEPS_PER_CELL)
}

fn right(p: GridPt, cells: i32) -> GridPt {
    GridPt::new(p.x + cells * STEPS_PER_CELL, p.y)
}

/// Ready everyone and tick until the phase changes.
fn ready_through(play: &mut Play) {
    let phase = play.rules.phase;
    for p in play.rules.alive() {
        let _ = play.apply(p, Input::Ready);
    }
    let mut n = 0;
    while play.rules.phase == phase || play.rules.phase == Phase::SiegeOver {
        play.tick();
        n += 1;
        assert!(n < 20_000, "stuck in {:?}", play.rules.phase);
    }
}

#[test]
fn units_one_cell_is_one_physics_unit_and_four_grid_steps() {
    assert_eq!(units::terrain_to_fx(CELL), Fx::ONE);
    assert_eq!(GridPt::new(4, 10).to_fx(), Vec2::new(Fx::ONE, Fx::ratio(5, 2)));
    assert_eq!(GridPt::new(7, 0).col(), 1);
    // royals stand 4 segment lengths of 2 cells apart
    assert_eq!(crate::game::royals::spacing_cells(), 8);
}

#[test]
fn rules_plot_size_is_the_terrain_span() {
    let play = duel(3);
    let pl = &play.rules.players[0];
    // level 2 at round 1: 40 + 7 columns, 12 + 2 cells high
    assert_eq!(pl.purse.level, 2);
    assert_eq!(pl.plot().width, 47);
    assert_eq!(pl.plot().height, 14);
}

#[test]
fn build_segment_costs_gold_and_joins_the_castle() {
    let mut play = duel(3);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 2));
    let gold = play.rules.players[0].purse.gold;
    assert_eq!(
        play.apply(
            0,
            Input::PlaceSegment {
                piece: WOOD,
                a: g,
                b: up(g, 2)
            }
        ),
        Ok(())
    );
    assert_eq!(play.rules.players[0].purse.gold, gold - 1);
    let id = *play.castles[0].geo.keys().next().unwrap();
    let w = play.castles[0].world(&play.rules.players[0], false);
    assert_eq!(w.seg_ids.iter().filter(|s| **s == id).count(), 1);
}

#[test]
fn build_too_long_or_out_of_span_is_refused_for_free() {
    let mut play = duel(3);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 2));
    let gold = play.rules.players[0].purse.gold;
    // wood's longest is 2 cells
    let long = Input::PlaceSegment {
        piece: WOOD,
        a: g,
        b: GridPt::new(g.x, g.y + 9),
    };
    assert_eq!(play.apply(0, long), Err(Refusal::BadSpot));
    let pad = ground(&plot, plot.launch_col);
    let outside = Input::PlaceSegment {
        piece: WOOD,
        a: pad,
        b: up(pad, 1),
    };
    assert_eq!(play.apply(0, outside), Err(Refusal::BadSpot));
    assert_eq!(play.rules.players[0].purse.gold, gold);
    assert!(play.castles[0].geo.is_empty());
}

#[test]
fn royal_must_stand_on_a_floor_or_the_ground() {
    let mut play = duel(3);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 2));
    let air = up(g, 3);
    assert_eq!(
        play.apply(0, Input::PlaceRoyal { index: 0, at: air }),
        Err(Refusal::BadSpot)
    );
    // a floor 3/2 cells up on two posts
    let r = ground(&plot, flat_col(&plot, 4));
    let (lt, rt) = (GridPt::new(g.x, g.y + 6), GridPt::new(r.x, g.y + 6));
    for (a, b) in [(g, lt), (r, rt), (lt, rt)] {
        play.apply(0, Input::PlaceSegment { piece: WOOD, a, b }).unwrap();
    }
    let on_floor = GridPt::new(g.x + 4, g.y + 6);
    assert_eq!(
        play.apply(
            0,
            Input::PlaceRoyal {
                index: 0,
                at: on_floor
            }
        ),
        Ok(())
    );
}

#[test]
fn strain_shows_a_cantilever_and_not_a_beam_on_the_ground() {
    let mut play = duel(3);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 2));
    let g1 = ground(&plot, flat_col(&plot, 3));
    play.apply(
        0,
        Input::PlaceSegment {
            piece: WOOD,
            a: g,
            b: g1,
        },
    )
    .unwrap();
    let post = ground(&plot, flat_col(&plot, 8));
    play.apply(
        0,
        Input::PlaceSegment {
            piece: WOOD,
            a: post,
            b: up(post, 2),
        },
    )
    .unwrap();
    play.apply(
        0,
        Input::PlaceSegment {
            piece: WOOD,
            a: up(post, 2),
            b: right(up(post, 2), 2),
        },
    )
    .unwrap();
    let ids: Vec<u32> = play.castles[0].geo.keys().copied().collect();
    let strain = play.strain(0);
    let of = |id: u32| strain.iter().find(|(k, _)| *k == id).unwrap().1;
    assert_eq!(of(ids[0]), 0, "both ends anchored: no load");
    assert!(of(ids[2]) > 0, "the arm carries load");
    assert!(strain.iter().all(|(_, s)| *s <= 1000));
}

/// Into the siege with player 0's squire on the ground at `col`.
fn siege_with_squire(seed: u64) -> (Play, Vec2) {
    let mut play = duel(seed);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 3));
    play.apply(0, Input::PlaceRoyal { index: 0, at: g }).unwrap();
    for p in [0, 1] {
        play.apply(p, Input::Ready).unwrap();
    }
    play.tick();
    assert_eq!(play.rules.phase, Phase::Siege);
    (play, g.to_fx())
}

fn field_on(play: &mut Play, owner: u8) -> &mut super::siege::Field {
    play.siege
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.owner == owner)
        .unwrap()
}

#[test]
fn siege_shot_on_a_royal_kills_it_and_costs_health() {
    let (mut play, at) = siege_with_squire(3);
    let f = field_on(&mut play, 0);
    assert_eq!(f.castle.bodies, vec![BodyOwner::Royal(0)]);
    // drop a shot straight onto the squire
    let launch = Launch {
        pos: at + Vec2::new(Fx::ZERO, Fx::from_int(2)),
        vel: Vec2::new(Fx::ZERO, -Fx::ratio(1, 10)),
    };
    f.castle.world.fire(launch, Fx::ratio(1, 4), Fx::from_int(2));
    ready_through(&mut play);
    let r = play.last_result.clone().unwrap();
    let c = r.castles.iter().find(|c| c.owner == 0).unwrap();
    assert_eq!(c.royals_killed, vec![RoyalKind::Squire]);
    // a squire deals 1 in round 1
    assert_eq!(play.rules.players[0].health, rules::START_HEALTH - 1);
    assert_eq!(play.rules.players[1].health, rules::START_HEALTH);
}

#[test]
fn siege_swing_and_release_spend_a_shot() {
    let (mut play, _) = siege_with_squire(3);
    let left = field_on(&mut play, 0).shooter.shots_left();
    play.step(&[(1, Input::Swing)]);
    for _ in 0..39 {
        play.tick();
    }
    assert_eq!(play.step(&[(1, Input::Release)]), vec![Ok(())]);
    let f = field_on(&mut play, 0);
    assert_eq!(f.shooter.shots_left(), left - 1);
    assert_eq!(f.castle.world.projectiles.len(), 1);
}

#[test]
fn siege_shot_in_the_water_is_spent() {
    let (mut play, _) = siege_with_squire(3);
    let f = field_on(&mut play, 0);
    let col = (f.plot.water.cols.start + f.plot.water.cols.end) / 2;
    let pos = Vec2::new(
        Fx::from_int(col as i64),
        units::terrain_to_fx(f.plot.water.level) - Fx::ratio(1, 8),
    );
    let i = f
        .castle
        .world
        .fire(Launch { pos, vel: Vec2::ZERO }, Fx::ratio(1, 4), Fx::from_int(2));
    f.step();
    assert!(!f.castle.world.projectiles[i].alive);
}

#[test]
fn spot_gold_vein_building_that_survives_pays() {
    let seed = (1..200u64)
        .find(|s| {
            Plot::generate(*s)
                .spots
                .iter()
                .any(|sp| sp.kind == SpotKind::GoldVein && sp.level <= 2)
        })
        .unwrap();
    let mut play = duel(seed);
    let plot = play.rules.terrain.clone();
    let vein = plot
        .spots
        .iter()
        .find(|s| s.kind == SpotKind::GoldVein)
        .unwrap()
        .cols;
    let at = ground(&plot, vein.start + 1);
    play.apply(
        0,
        Input::PlaceBuilding {
            kind: BuildingKind::Carpenter,
            at,
        },
    )
    .unwrap();
    let gold = play.rules.players[0].purse.gold;
    ready_through(&mut play); // build -> siege
    ready_through(&mut play); // siege -> upgrade (round 2), nobody fired
    let pl = &play.rules.players[0];
    assert_eq!(pl.purse.gold, gold + rules::GOLD_VEIN_GOLD);
    assert_eq!(pl.ledger.stock.wood, 3);
}

/// Two scripted players: build a post each round, fire every shot at one
/// arc, pick the first card. Returns the checksum after every round.
fn scripted_match(seed: u64, rounds: u32) -> (Play, Vec<u64>) {
    let mut play = duel(seed);
    let plot = play.rules.terrain.clone();
    let mut sums = Vec::new();
    let mut built_round = 0;
    let mut ticks = 0;
    while play.rules.round <= rounds && play.rules.phase != Phase::Results {
        let mut inputs = Vec::new();
        match play.rules.phase {
            Phase::Upgrade => {
                for p in [0, 1] {
                    inputs.push((p, Input::Pick(0)));
                    inputs.push((p, Input::Ready));
                }
            }
            Phase::Build if built_round != play.rules.round => {
                built_round = play.rules.round;
                let c = flat_col(&plot, 2 + 3 * built_round as usize % 30);
                let g = ground(&plot, c);
                for p in [0, 1] {
                    inputs.push((
                        p,
                        Input::PlaceSegment {
                            piece: WOOD,
                            a: g,
                            b: up(g, 2),
                        },
                    ));
                    inputs.push((p, Input::Ready));
                }
            }
            Phase::Siege => {
                let s = play.siege.as_ref().unwrap();
                for f in &s.fields {
                    let sh = &f.shooter;
                    match sh.swing_from {
                        Some(t) if s.tick - t >= 42 => inputs.push((f.attacker, Input::Release)),
                        Some(_) => {}
                        None if sh.shots_left() > 0 => {
                            let slot = sh.ammo.iter().position(|a| a.1 > 0).unwrap() as u8;
                            inputs.push((f.attacker, Input::SelectAmmo(slot)));
                            inputs.push((f.attacker, Input::Swing));
                        }
                        None if !f.active() => inputs.push((f.attacker, Input::Ready)),
                        None => {}
                    }
                }
            }
            _ => {}
        }
        let round = play.rules.round;
        play.step(&inputs);
        if play.rules.round != round {
            sums.push(play.checksum());
        }
        ticks += 1;
        assert!(ticks < 100_000, "stuck");
    }
    (play, sums)
}

#[test]
fn full_match_two_players_play_rounds_with_real_physics() {
    let (play, sums) = scripted_match(5, 4);
    assert!(play.rules.round >= 5 || play.rules.phase == Phase::Results);
    assert_eq!(sums.len(), 4);
    // every shot was thrown: each castle saw its attacker's normal shots
    let r = play.last_result.as_ref().unwrap();
    assert_eq!(r.castles.len(), 2);
    // the same inputs give the same match
    let (_, again) = scripted_match(5, 4);
    assert_eq!(sums, again);
}

#[test]
fn full_match_bot_trebuchet_throws_its_shots() {
    let mut play = Play::new(Settings {
        seats: vec![Seat::Human, Seat::Bot(crate::game::flow::Difficulty::Normal)],
        wind: true,
        seed: 9,
    })
    .unwrap();
    play.start().unwrap();
    play.apply(0, Input::Ready).unwrap();
    while play.rules.phase == Phase::Build {
        play.tick();
    }
    let mut thrown = 0;
    while play.rules.phase == Phase::Siege {
        play.tick();
        if let Some(s) = &play.siege {
            thrown = s
                .fields
                .iter()
                .find(|f| f.owner == 0)
                .unwrap()
                .castle
                .world
                .projectiles
                .len();
        }
    }
    // level 2 has 3 normal shots
    assert_eq!(thrown, 3);
}

/// Where a normal shot released at `release` first touches the ground.
fn land_x(terrain: crate::physics::Terrain, base: Vec2, release: u32, power: u8) -> Option<i64> {
    use crate::physics::{ShotKind, ShotMods};
    let mut w = World::new(terrain);
    let launch = trebuchet_launch(base, release, power, false);
    if launch.vel.x <= Fx::ZERO || launch.vel.y <= Fx::ZERO {
        return None;
    }
    let i = w.fire_shot(launch, ShotKind::Normal, ShotMods::default()).balls[0];
    for _ in 0..3000 {
        w.step();
        let b = w.projectiles[i];
        if !b.alive || b.pos.y - b.radius <= w.terrain.height_at(b.pos.x) + Fx::ratio(1, 64) {
            return Some(b.pos.x.floor_int());
        }
    }
    None
}

/// Release ticks the tests try: past the end of the swing.
const SCAN: u32 = 240;

/// The farthest and nearest columns a forward throw lands at, over every
/// release tick, from `plot`'s trebuchet at `power`.
fn reach(plot: &Plot, power: u8) -> (i64, i64) {
    let base = units::trebuchet_base(plot);
    let lands: Vec<i64> = (0..SCAN)
        .filter_map(|r| land_x(units::physics_terrain(plot), base, r, power))
        .collect();
    (*lands.iter().min().unwrap(), *lands.iter().max().unwrap())
}

#[test]
fn trebuchet_level_one_reaches_the_whole_level_one_plot() {
    let power = super::siege::launch_power(rules::LEVELS[0].shot_power_pct);
    for seed in 1..=4 {
        let plot = Plot::generate(seed);
        let span = plot.buildable(1);
        let (near, far) = reach(&plot, power);
        assert!(
            near <= span.start as i64 && far >= span.end as i64,
            "seed {seed}: {near}..{far} vs {span:?}"
        );
    }
}

#[test]
fn trebuchet_full_power_reaches_the_far_end_of_the_level_nine_plot() {
    let power = super::siege::launch_power(rules::LEVELS[8].shot_power_pct);
    for seed in 1..=4 {
        let plot = Plot::generate(seed);
        let (_, far) = reach(&plot, power);
        assert!(far >= plot.buildable(9).end as i64, "seed {seed}: {far}");
    }
}

#[test]
fn trebuchet_level_one_clears_a_far_raised_plot() {
    // the far case the terrain is moving to: the plot's front 161 cells out
    // and 30 cells above the pad, with a 40-column level-1 span
    let mut heights = vec![Fx::ZERO; 120];
    heights.extend((0..10).map(|k| Fx::from_int(3 * k)));
    heights.extend(vec![Fx::from_int(30); 300]);
    let t = crate::physics::Terrain {
        col_width: Fx::ONE,
        heights,
    };
    let base = Vec2::ints(5, 0);
    let power = super::siege::launch_power(rules::LEVELS[0].shot_power_pct);
    let far = (0..SCAN)
        .filter_map(|r| land_x(t.clone(), base, r, power))
        .max()
        .unwrap();
    assert!(far >= 5 + 161 + 40, "level 1 lands at most {far}");
}

/// Into the siege after `setup` changes the match in the build phase, with
/// player 0's squire on the ground.
fn siege_after(seed: u64, setup: impl FnOnce(&mut Play)) -> (Play, Vec2) {
    let mut play = duel(seed);
    setup(&mut play);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 3));
    play.apply(0, Input::PlaceRoyal { index: 0, at: g }).unwrap();
    for p in [0, 1] {
        play.apply(p, Input::Ready).unwrap();
    }
    play.tick();
    assert_eq!(play.rules.phase, Phase::Siege);
    (play, g.to_fx())
}

/// A shot dropped straight down onto `at` from 2 cells up.
fn drop_on(f: &mut super::siege::Field, at: Vec2) -> usize {
    let launch = Launch {
        pos: at + Vec2::new(Fx::ZERO, Fx::from_int(2)),
        vel: Vec2::new(Fx::ZERO, -Fx::ratio(1, 10)),
    };
    let i = f.castle.world.fire(launch, Fx::ratio(1, 4), Fx::from_int(2));
    for _ in 0..60 {
        f.step();
    }
    i
}

#[test]
fn wire_strength_upgrade_raises_segment_hit_points() {
    let mut play = duel(3);
    let plot = play.rules.terrain.clone();
    let g = ground(&plot, flat_col(&plot, 2));
    play.apply(
        0,
        Input::PlaceSegment {
            piece: WOOD,
            a: g,
            b: up(g, 2),
        },
    )
    .unwrap();
    let id = *play.castles[0].geo.keys().next().unwrap();
    play.rules.players[0].mods.strength_pct.push((Material::Wood, 25));
    let cw = play.castles[0].world(&play.rules.players[0], false);
    let i = cw.seg_ids.iter().position(|s| *s == id).unwrap();
    // wood's 1/2 hit point plus 25%
    assert_eq!(cw.world.segments[i].hp, Fx::ratio(5, 8));
}

#[test]
fn wire_shot_mods_reach_the_fired_shot() {
    use crate::game::upgrades::AmmoKind;
    let (mut play, _) = siege_after(3, |p| {
        let m = &mut p.rules.players[1].mods;
        m.weight_pct = 25;
        m.blast_pct = 50;
        m.all_ignite = true;
        m.ammo.push((AmmoKind::Exploding, 1));
    });
    let f = field_on(&mut play, 0);
    f.select(1).unwrap();
    f.swing(0).unwrap();
    f.release(45).unwrap();
    let b = f.castle.world.projectiles[f.shooter.live[0]];
    // a normal shot's 6 plus 25%, alight; blast 3/2 plus 50%
    assert_eq!(b.mass, Fx::ratio(15, 2));
    assert!(b.fire);
    assert_eq!(f.fuses[0].radius, Fx::ratio(9, 4));
}

#[test]
fn wire_air_uses_run_out_for_the_round() {
    use crate::game::upgrades::AirKind;
    let (mut play, _) = siege_after(3, |p| {
        let m = &mut p.rules.players[1].mods;
        m.air.push(AirKind::Drop);
        m.air_uses.push(2);
        m.extra_shots = 3;
    });
    let f = field_on(&mut play, 0);
    let mut used = Vec::new();
    for k in 0..3 {
        f.swing(k * 100).unwrap();
        f.release(k * 100 + 45).unwrap();
        used.push(f.air(0));
    }
    assert_eq!(used, vec![Ok(()), Ok(()), Err(Refusal::Limit)]);
}

#[test]
fn wire_chainmail_saves_a_squire_from_one_hit() {
    let (mut play, at) = siege_after(3, |p| {
        p.rules.players[0]
            .mods
            .royal_saves
            .push((Some(RoyalKind::Squire), 1));
    });
    let f = field_on(&mut play, 0);
    let first = drop_on(f, at);
    assert!(f.castle.world.bodies[0].alive, "the first hit is saved");
    assert!(!f.castle.world.projectiles[first].alive, "and stops the shot");
    drop_on(f, at);
    assert!(!f.castle.world.bodies[0].alive);
}

#[test]
fn wire_second_volley_fires_the_first_shot_twice() {
    let (mut play, _) = siege_after(3, |p| p.rules.players[1].mods.first_shot_copies = 2);
    let f = field_on(&mut play, 0);
    f.swing(0).unwrap();
    f.release(45).unwrap();
    assert_eq!(f.castle.world.projectiles.len(), 2);
    f.swing(100).unwrap();
    f.release(145).unwrap();
    assert_eq!(f.castle.world.projectiles.len(), 3);
}

#[test]
fn wire_royals_killed_are_listed_in_kill_order() {
    let (mut play, _) = siege_after(3, |p| {
        let plot = p.rules.terrain.clone();
        p.rules.players[0].royals.push(crate::game::royals::Royal {
            kind: RoyalKind::King,
            spot: None,
        });
        let g = ground(&plot, flat_col(&plot, 3 + 8));
        p.apply(0, Input::PlaceRoyal { index: 1, at: g }).unwrap();
    });
    let royals = play.rules.players[0].royals.clone();
    let f = field_on(&mut play, 0);
    let king = f
        .castle
        .bodies
        .iter()
        .position(|b| *b == BodyOwner::Royal(1))
        .unwrap();
    let squire = f
        .castle
        .bodies
        .iter()
        .position(|b| *b == BodyOwner::Royal(0))
        .unwrap();
    // the king (the later body) dies first
    f.castle.world.bodies[king].alive = false;
    f.step();
    f.castle.world.bodies[squire].alive = false;
    f.step();
    let out = f.outcome(&royals);
    assert_eq!(out.royals_killed, vec![RoyalKind::King, RoyalKind::Squire]);
}

/// The longest run of back-to-back release ticks whose throw at `level`'s
/// power lands on `level`'s plot, for the terrain seed (1-4) where it is shortest.
fn shortest_plot_window(level: u8) -> (u64, usize) {
    let power = super::siege::launch_power(rules::LEVELS[level as usize - 1].shot_power_pct);
    (1..=4)
        .map(|seed| {
            let plot = Plot::generate(seed);
            let base = units::trebuchet_base(&plot);
            let span = plot.buildable(level);
            let (mut best, mut run) = (0, 0);
            for r in 0..SCAN {
                let on = land_x(units::physics_terrain(&plot), base, r, power)
                    .is_some_and(|x| x >= span.start as i64 && x < span.end as i64);
                run = if on { run + 1 } else { 0 };
                best = best.max(run);
            }
            (seed, best)
        })
        .min_by_key(|(_, best)| *best)
        .unwrap()
}

#[test]
fn trebuchet_level_one_plot_window_is_at_least_12_ticks() {
    let (seed, ticks) = shortest_plot_window(1);
    assert!(ticks >= 12, "seed {seed}: {ticks} ticks");
}

#[test]
fn trebuchet_level_nine_plot_window_is_at_least_12_ticks() {
    let (seed, ticks) = shortest_plot_window(9);
    assert!(ticks >= 12, "seed {seed}: {ticks} ticks");
}
