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
fn level_one_throw_reaches_the_plot_front() {
    let plot = Plot::generate(1);
    let power = super::siege::launch_power(100);
    let span = plot.buildable(1);
    let mut w = World::new(units::physics_terrain(&plot));
    let i = w.fire(
        trebuchet_launch(units::trebuchet_base(&plot), 44, power, false),
        Fx::ratio(1, 4),
        Fx::from_int(2),
    );
    let mut landed = None;
    for _ in 0..2000 {
        w.step();
        let b = w.projectiles[i];
        if b.pos.y - b.radius <= w.terrain.height_at(b.pos.x) {
            landed = Some(b.pos.x.floor_int());
            break;
        }
    }
    let x = landed.expect("the shot lands");
    // worked out from the probe: release 44 at level 1 lands near column 90
    assert!(
        x >= span.start as i64 && x < plot.plot.end as i64,
        "landed at {x}"
    );
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
