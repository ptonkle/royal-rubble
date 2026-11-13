//! Physics behavior tests: small builds, one behavior each.

use super::*;

fn flat_world() -> World {
    World::new(Terrain::flat(80, Fx::ZERO))
}

fn beam(w: &mut World, a: (i64, i64), b: (i64, i64), m: Material) -> usize {
    w.add_beam(Vec2::ints(a.0, a.1), Vec2::ints(b.0, b.1), m).unwrap()
}

/// A braced tower one unit wide at x = 2..3.
fn tower_of(w: &mut World, height: i64, m: Material) {
    for y in 0..height {
        beam(w, (2, y), (2, y + 1), m);
        beam(w, (3, y), (3, y + 1), m);
        beam(w, (2, y + 1), (3, y + 1), m);
        beam(w, (2, y), (3, y + 1), m);
    }
}

fn tower(w: &mut World, height: i64) {
    tower_of(w, height, Material::Wood);
}

fn node_at(w: &World, x: i64, y: i64) -> usize {
    w.nodes.iter().position(|n| n.pos == Vec2::ints(x, y)).unwrap()
}

fn shot(pos: Vec2, vel: Vec2) -> Launch {
    Launch { pos, vel }
}

#[test]
fn a_free_beam_falls_by_gravity_each_tick() {
    let mut w = flat_world();
    beam(&mut w, (5, 5), (6, 5), Material::Wood);
    w.step();
    assert_eq!(w.nodes[0].pos.y, Fx::from_int(5) - TUNE.gravity);
    w.step();
    // verlet: the second tick falls about twice as far
    assert!(w.nodes[0].pos.y < Fx::from_int(5) - TUNE.gravity * 2);
}

#[test]
fn nodes_on_the_ground_are_anchored() {
    let mut w = flat_world();
    beam(&mut w, (5, 0), (5, 1), Material::Wood);
    w.build();
    assert!(w.nodes[0].anchored);
    assert!(!w.nodes[1].anchored);
}

#[test]
fn a_segment_longer_than_its_material_allows_is_refused() {
    let mut w = flat_world();
    let a = w.add_node(Vec2::ints(0, 0));
    let b = w.add_node(Vec2::ints(3, 0));
    assert_eq!(w.add_segment(a, b, Material::Wood), Err(BuildError::TooLong));
    assert!(w.add_segment(a, b, Material::Metal).is_ok());
}

#[test]
fn the_same_build_and_shots_give_the_same_checksum() {
    let run = || {
        let mut w = flat_world();
        tower(&mut w, 3);
        w.wind = Fx::ratio(1, 2000);
        w.fire(
            trebuchet_launch(Vec2::ints(-8, 0), 48, 2, false),
            Fx::ratio(1, 3),
            Fx::from_int(4),
        );
        for _ in 0..240 {
            w.step();
        }
        w.checksum()
    };
    let first = run();
    assert_eq!(first, run());
    let mut other = flat_world();
    tower(&mut other, 3);
    assert_ne!(first, other.checksum());
}

#[test]
fn a_long_cantilever_breaks_where_a_braced_one_holds() {
    let mut bare = flat_world();
    tower_of(&mut bare, 2, Material::Stone);
    let root = beam(&mut bare, (3, 2), (4, 2), Material::Wood);
    beam(&mut bare, (4, 2), (5, 2), Material::Wood);
    beam(&mut bare, (5, 2), (6, 2), Material::Wood);
    let report = bare.static_test(240);
    assert!(report.broken.contains(&root), "{report:?}");

    let mut braced = flat_world();
    tower_of(&mut braced, 2, Material::Stone);
    beam(&mut braced, (3, 2), (4, 2), Material::Wood);
    beam(&mut braced, (4, 2), (5, 2), Material::Wood);
    beam(&mut braced, (5, 2), (6, 2), Material::Wood);
    // a truss under it: bottom chord, posts and diagonals
    beam(&mut braced, (3, 1), (4, 1), Material::Wood);
    beam(&mut braced, (4, 1), (5, 1), Material::Wood);
    beam(&mut braced, (4, 1), (4, 2), Material::Wood);
    beam(&mut braced, (5, 1), (5, 2), Material::Wood);
    beam(&mut braced, (3, 1), (4, 2), Material::Wood);
    beam(&mut braced, (4, 1), (5, 2), Material::Wood);
    beam(&mut braced, (5, 1), (6, 2), Material::Wood);
    let report = braced.static_test(240);
    assert!(report.broken.is_empty(), "{report:?}");
}

#[test]
fn the_static_test_reports_strain_rising_toward_the_root() {
    let mut w = flat_world();
    tower(&mut w, 2);
    let root = beam(&mut w, (3, 2), (4, 2), Material::Wood);
    let tip = beam(&mut w, (4, 2), (5, 2), Material::Wood);
    let report = w.static_test(120);
    assert!(report.broken.is_empty());
    assert!(report.max_strain[root] > report.max_strain[tip]);
    assert!(report.max_strain[tip] > Fx::ZERO);
}

/// A tall tower with a heavy stone beam sticking out at height 1, and maybe
/// a rope from the tower top to its tip.
fn stone_arm(with_rope: bool) -> (World, usize, Option<usize>) {
    let mut w = flat_world();
    tower(&mut w, 3);
    beam(&mut w, (3, 1), (5, 1), Material::Stone);
    let rope = with_rope.then(|| beam(&mut w, (3, 3), (5, 1), Material::Rope));
    let tip = node_at(&w, 5, 1);
    (w, tip, rope)
}

#[test]
fn a_rope_carries_tension_and_holds_up_a_beam() {
    let (mut held, tip, rope) = stone_arm(true);
    for _ in 0..180 {
        held.step();
    }
    assert!(held.segments.iter().all(|s| !s.broken));
    assert!((held.nodes[tip].pos.y - Fx::ONE).abs() < Fx::ratio(1, 10));
    assert!(held.segments[rope.unwrap()].axial > Fx::ZERO);

    let (mut loose, tip, _) = stone_arm(false);
    for _ in 0..180 {
        loose.step();
    }
    assert!(loose.nodes[tip].pos.y < Fx::HALF);
}

#[test]
fn a_shot_passes_through_a_rope() {
    let mut w = flat_world();
    tower(&mut w, 3);
    let rope = beam(&mut w, (3, 3), (5, 1), Material::Rope);
    let vel = Vec2::new(Fx::ratio(1, 2), Fx::ZERO);
    let p = w.fire(
        shot(Vec2::new(Fx::from_int(4) - Fx::ONE, Fx::from_int(2)), vel),
        Fx::ratio(1, 4),
        Fx::ONE,
    );
    for _ in 0..4 {
        w.step();
    }
    assert!(!w.segments[rope].broken);
    assert_eq!(w.projectiles[p].vel.x, vel.x);
}

#[test]
fn a_shot_that_knocks_out_a_support_drops_the_floor_above() {
    let mut w = flat_world();
    let support = beam(&mut w, (5, 0), (5, 1), Material::Wood);
    beam(&mut w, (5, 1), (5, 2), Material::Wood);
    beam(&mut w, (7, 0), (7, 1), Material::Wood);
    beam(&mut w, (7, 1), (7, 2), Material::Wood);
    beam(&mut w, (5, 2), (7, 2), Material::Stone);
    let corner = node_at(&w, 5, 2);
    assert!(w.static_test(120).broken.is_empty());
    for _ in 0..60 {
        w.step();
    }
    let at = Vec2::new(Fx::from_int(4), Fx::HALF);
    w.fire(
        shot(at, Vec2::new(Fx::ratio(1, 2), Fx::ZERO)),
        Fx::ratio(1, 4),
        Fx::from_int(6),
    );
    let mut broke_support = false;
    for _ in 0..240 {
        for e in w.step() {
            if e == (Event::Broke {
                segment: support,
                cause: BreakCause::Impact,
            }) {
                broke_support = true;
            }
        }
    }
    assert!(broke_support);
    assert!(w.nodes[corner].pos.y < Fx::ONE);
}

/// A loose wood floor at `height` with a body on it, which falls to the ground.
fn falling_floor(height: i64) -> Vec<Event> {
    let mut w = flat_world();
    let s = beam(&mut w, (10, height), (11, height), Material::Wood);
    w.add_body(s, Fx::HALF, Fx::ratio(1, 4));
    let mut events = Vec::new();
    for _ in 0..240 {
        events.extend(w.step());
    }
    events
}

#[test]
fn a_body_that_falls_too_far_dies_and_a_short_fall_does_not() {
    assert!(falling_floor(5)
        .iter()
        .any(|e| matches!(e, Event::BodyFell { body: 0, .. })));
    assert!(falling_floor(2).is_empty());
}

#[test]
fn a_shot_that_touches_a_body_kills_it() {
    let mut w = flat_world();
    tower(&mut w, 2);
    let top = w
        .segments
        .iter()
        .position(|s| s.a == node_at(&w, 2, 2) && s.b == node_at(&w, 3, 2))
        .unwrap();
    w.add_body(top, Fx::HALF, Fx::ratio(1, 4));
    let from = Vec2::new(Fx::ZERO, Fx::from_int(2) + Fx::ratio(2, 5));
    let p = w.fire(
        shot(from, Vec2::new(Fx::ratio(3, 10), Fx::ZERO)),
        Fx::ratio(1, 4),
        Fx::ONE,
    );
    let mut hit = false;
    for _ in 0..30 {
        hit |= w.step().contains(&Event::BodyHit {
            body: 0,
            projectile: p,
        });
    }
    assert!(hit);
    assert!(!w.bodies[0].alive);
}

#[test]
fn heavy_fast_debris_crushes_a_body_and_light_debris_does_not() {
    let crush = |mass: Fx| {
        let mut w = flat_world();
        let s = beam(&mut w, (10, 0), (11, 0), Material::Stone);
        w.add_body(s, Fx::HALF, Fx::ratio(1, 4));
        let above = w.bodies[0].pos + Vec2::new(Fx::ZERO, Fx::ONE);
        w.debris.push(Ball::new(
            above,
            Vec2::new(Fx::ZERO, -Fx::ratio(1, 5)),
            Fx::ratio(1, 8),
            mass,
        ));
        let mut events = Vec::new();
        for _ in 0..20 {
            events.extend(w.step());
        }
        events.contains(&Event::BodyCrushed { body: 0 })
    };
    assert!(crush(Fx::from_int(2)));
    assert!(!crush(Fx::ratio(1, 10)));
}

fn flying() -> (World, usize) {
    let mut w = flat_world();
    let p = w.fire(
        shot(Vec2::ints(5, 10), Vec2::new(Fx::ratio(1, 5), Fx::ratio(1, 10))),
        Fx::ratio(1, 4),
        Fx::from_int(3),
    );
    (w, p)
}

#[test]
fn air_drop_stops_sideways_motion() {
    let (mut w, p) = flying();
    w.air_effect(p, AirEffect::Drop);
    assert_eq!(w.projectiles[p].vel.x, Fx::ZERO);
    w.step();
    assert_eq!(w.projectiles[p].pos.x, Fx::from_int(5));
}

#[test]
fn air_reverse_flips_sideways_motion() {
    let (mut w, p) = flying();
    w.air_effect(p, AirEffect::Reverse);
    assert_eq!(w.projectiles[p].vel.x, -Fx::ratio(1, 5));
    assert_eq!(w.projectiles[p].vel.y, Fx::ratio(1, 10));
}

#[test]
fn air_phase_passes_through_a_beam() {
    let wall = |phase: bool| {
        let mut w = flat_world();
        let s = beam(&mut w, (8, 0), (8, 2), Material::Wood);
        let p = w.fire(
            shot(Vec2::ints(6, 1), Vec2::new(Fx::ratio(1, 2), Fx::ZERO)),
            Fx::ratio(1, 4),
            Fx::from_int(4),
        );
        if phase {
            w.air_effect(p, AirEffect::Phase);
        }
        for _ in 0..10 {
            w.step();
        }
        (w.segments[s].broken, w.segments[s].hp)
    };
    assert_eq!(wall(true), (false, TUNE.wood.hp));
    assert_ne!(wall(false).1, TUNE.wood.hp);
}

#[test]
fn air_split_makes_three_shots_fanned_around_the_path() {
    let (mut w, p) = flying();
    let before = w.projectiles[p];
    let out = w.air_effect(p, AirEffect::Split);
    assert_eq!(out.len(), 3);
    assert_eq!(w.projectiles.len(), 3);
    assert_eq!(w.projectiles[p].vel, before.vel);
    assert_eq!(w.projectiles[p].mass, Fx::from_int(1));
    let (l, r) = (w.projectiles[out[1]].vel, w.projectiles[out[2]].vel);
    // one turned up, one turned down, by the same angle
    assert!(l.y < before.vel.y && r.y > before.vel.y);
    assert!((before.vel.cross(l) + before.vel.cross(r)).abs() <= Fx(4));
}

#[test]
fn air_dive_heads_straight_down_at_double_speed() {
    let (mut w, p) = flying();
    let speed = w.projectiles[p].vel.len();
    w.air_effect(p, AirEffect::Dive);
    assert_eq!(w.projectiles[p].vel, Vec2::new(Fx::ZERO, -(speed * 2)));
}

#[test]
fn air_boost_adds_speed_along_the_path() {
    let (mut w, p) = flying();
    let before = w.projectiles[p].vel;
    w.air_effect(p, AirEffect::Boost);
    let after = w.projectiles[p].vel;
    assert!((after.len() - before.len() - TUNE.boost_speed).abs() <= Fx(8));
    assert!(before.cross(after).abs() <= Fx(8));
}

fn launch_angle_order(a: Launch, b: Launch) -> bool {
    // a is steeper than b (both moving forward): a.vy/a.vx > b.vy/b.vx
    a.vel.y.raw() * b.vel.x.raw() > b.vel.y.raw() * a.vel.x.raw()
}

#[test]
fn a_later_release_throws_a_flatter_arc() {
    let base = Vec2::ints(0, 0);
    let forward = |l: &Launch| l.vel.x > Fx::ZERO && l.vel.y > Fx::ZERO;
    let useful: Vec<Launch> = (0..trebuchet::SWING_END)
        .map(|t| trebuchet_launch(base, t, 3, false))
        .filter(forward)
        .collect();
    assert!(useful.len() >= 10, "useful window {}", useful.len());
    for pair in useful.windows(2) {
        assert!(launch_angle_order(pair[0], pair[1]), "{pair:?}");
    }
    // the first forward throw goes nearly straight up
    let up = useful[0];
    assert!(up.vel.y > up.vel.x * 3, "{up:?}");
    // too early throws backward, too late throws into the ground
    assert!(trebuchet_launch(base, 10, 3, false).vel.x < Fx::ZERO);
    let late = trebuchet_launch(base, trebuchet::SWING_END - 1, 3, false);
    assert!(late.vel.y <= Fx::ZERO, "{late:?}");
    // facing left mirrors it
    let r = trebuchet_launch(base, 50, 3, false);
    let l = trebuchet_launch(base, 50, 3, true);
    assert_eq!((l.vel.x, l.vel.y), (-r.vel.x, r.vel.y));
}

#[test]
fn more_power_throws_faster() {
    let base = Vec2::ints(0, 0);
    let weak = trebuchet_launch(base, 50, 0, false).vel.len();
    let strong = trebuchet_launch(base, 50, 60, false).vel.len();
    assert!(strong > weak + weak / 2);
}

#[test]
fn wind_pushes_a_shot_downwind() {
    let land = |wind: Fx| {
        let mut w = flat_world();
        w.wind = wind;
        let p = w.fire(
            shot(Vec2::ints(5, 5), Vec2::new(Fx::ratio(1, 10), Fx::ZERO)),
            Fx::ratio(1, 4),
            Fx::ONE,
        );
        for _ in 0..120 {
            w.step();
        }
        w.projectiles[p].pos.x
    };
    assert!(land(Fx::ratio(1, 1000)) > land(Fx::ZERO));
}

#[test]
fn fire_spreads_through_touching_wood_but_not_stone() {
    let mut w = flat_world();
    let lit = beam(&mut w, (5, 0), (5, 1), Material::Wood);
    let wood = beam(&mut w, (5, 1), (6, 1), Material::Wood);
    let stone = beam(&mut w, (5, 1), (4, 1), Material::Stone);
    assert!(w.ignite(lit));
    assert!(!w.ignite(stone));
    for _ in 0..(TUNE.burn_spread_every + 1) {
        w.step();
    }
    assert!(w.segments[wood].burning());
    assert!(!w.segments[stone].burning());
    assert!(w.segments[lit].hp < TUNE.wood.hp);
}

/// A light shot flying level into a braced stone tower at x = 2; returns
/// its sideways speed on the first tick it is no longer heading right, and
/// whether Bounce is still armed then.
fn into_stone_wall(armed: bool) -> (Fx, bool) {
    let mut w = flat_world();
    tower_of(&mut w, 5, Material::Stone);
    w.build();
    let p = w.fire(
        shot(Vec2::ints(0, 3), Vec2::new(Fx::HALF, Fx::ZERO)),
        Fx::ratio(1, 4),
        Fx::ratio(1, 10),
    );
    if armed {
        w.air_effect(p, AirEffect::Bounce);
    }
    for _ in 0..20 {
        w.step();
        let b = w.projectiles[p];
        if b.vel.x <= Fx::ZERO {
            return (b.vel.x, b.bounce);
        }
    }
    panic!("the shot never reached the wall");
}

#[test]
fn air_bounce_skips_off_a_wall_keeping_most_of_its_speed() {
    // came in at 1/2 a tick: Bounce keeps 4/5 of it, about 2/5 back the
    // other way; a plain hit keeps about 1/4 of it, 1/8 back
    let (armed, _) = into_stone_wall(true);
    let (plain, _) = into_stone_wall(false);
    assert!(armed < -Fx::ratio(3, 10), "armed {armed:?}");
    assert!(plain > -Fx::ratio(1, 5), "plain {plain:?}");
}

#[test]
fn air_bounce_is_spent_after_one_contact() {
    let (_, still_armed) = into_stone_wall(true);
    assert!(!still_armed);
    // the ground then takes it as a plain landing: speed lost, not kept
    let mut w = flat_world();
    let p = w.fire(
        shot(Vec2::ints(5, 1), Vec2::new(Fx::ZERO, -Fx::HALF)),
        Fx::ratio(1, 4),
        Fx::ONE,
    );
    w.air_effect(p, AirEffect::Bounce);
    // the upward speed at the start of each rebound off the ground
    let mut rebounds = Vec::new();
    let mut rising = false;
    for _ in 0..400 {
        w.step();
        let vy = w.projectiles[p].vel.y;
        if vy > Fx::ZERO && !rising {
            rebounds.push(vy);
        }
        rising = vy > Fx::ZERO;
    }
    assert!(rebounds.len() >= 2, "{rebounds:?}");
    // first off the ground: about 4/5 of 1/2; second: about 1/4 of that
    assert!(rebounds[0] > Fx::ratio(3, 10), "{rebounds:?}");
    assert!(rebounds[1] < Fx::ratio(3, 20), "{rebounds:?}");
}

#[test]
fn trebuchet_forward_release_window_is_one_block_before_the_arm_stops() {
    let base = Vec2::ints(0, 0);
    let forward: Vec<u32> = (0..300)
        .filter(|&t| {
            let l = trebuchet_launch(base, t, 0, false);
            l.vel.x > Fx::ZERO && l.vel.y > Fx::ZERO
        })
        .collect();
    let last = *forward.last().unwrap();
    let first = forward[0];
    assert!(
        forward.len() as u32 == last - first + 1 && forward.len() >= 30 && last < trebuchet::SWING_END,
        "forward and up {forward:?}"
    );
}

#[test]
fn a_full_speed_shot_cannot_skip_a_single_stone_wall() {
    // a small shot at 1.5 units a tick (faster than any throw) from eight
    // starting offsets: every one touches the one-segment wall
    for k in 0..8 {
        let mut w = flat_world();
        let wall = beam(&mut w, (20, 0), (20, 2), Material::Stone);
        let hp = TUNE.material(Material::Stone).hp;
        let from = Vec2::new(Fx::from_int(10) + Fx::ratio(k, 8), Fx::ONE);
        w.fire(
            shot(from, Vec2::new(Fx::ratio(3, 2), Fx::ZERO)),
            Fx::ratio(1, 6),
            Fx::ONE,
        );
        for _ in 0..12 {
            w.step();
        }
        let s = &w.segments[wall];
        assert!(s.broken || s.hp < hp, "offset {k}/8: the shot passed through");
    }
}

#[test]
fn trebuchet_arm_speeds_up_once_and_slows_once_without_jerks() {
    // R1.1601: one rise, one fall, and no tick-to-tick jump over 100 brads
    let speeds: Vec<i64> = (0..=trebuchet::SWING_END).map(|t| arm_at(t).1).collect();
    let mut turns = 0;
    let mut rising = true;
    for w in speeds.windows(2) {
        assert!((w[1] - w[0]).abs() <= 100, "jump {} to {}", w[0], w[1]);
        if rising && w[1] < w[0] {
            rising = false;
            turns += 1;
        } else if !rising && w[1] > w[0] {
            turns += 1;
        }
    }
    assert_eq!(turns, 1, "{speeds:?}");
    assert_eq!(*speeds.last().unwrap(), 0);
}

#[test]
fn trebuchet_pose_holds_the_shot_until_release_then_the_arm_rocks_and_rests() {
    let base = Vec2::ints(0, 0);
    let rest = pose(base, Phase::Rest, false);
    assert!(rest.loaded);
    // in the pouch: the sling's last point is where the shot leaves
    let t = 75;
    let swing = pose(base, Phase::Swing(t), false);
    assert_eq!(swing.sling[2], trebuchet_launch(base, t, 0, false).pos);
    let after = pose(base, Phase::After { at: t, since: 10 }, false);
    assert!(!after.loaded);
    // the arm follows through past where it let go
    assert!(after.tip.x > swing.tip.x, "{:?} {:?}", after.tip, swing.tip);
    // and once rocked and winched back it is cocked again
    let done = trebuchet::ROCK_TICKS + trebuchet::WINCH_TICKS;
    assert_eq!(pose(base, Phase::After { at: t, since: done }, false), rest);
}

#[test]
fn trebuchet_arm_turns_at_a_steady_speed_through_the_release_block() {
    // R1.1803, the person: "a consistentish speed of swinging around"; from
    // tick 30 (the speed-up's end; 15 before R1.3700 halved the arm speed) to
    // the last forward-and-up release the fastest tick is under twice the slowest
    let base = Vec2::ints(0, 0);
    let last = (0..trebuchet::SWING_END)
        .filter(|&t| {
            let v = trebuchet_launch(base, t, 0, false).vel;
            v.x > Fx::ZERO && v.y > Fx::ZERO
        })
        .max()
        .unwrap();
    let speeds: Vec<i64> = (30..=last).map(|t| arm_at(t).1).collect();
    let (lo, hi) = (*speeds.iter().min().unwrap(), *speeds.iter().max().unwrap());
    assert!(lo > 0 && hi < lo * 2, "speeds {lo} to {hi}");
}

/// Whether segments a0-a1 and b0-b1 cross.
fn crosses(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).cross(r - p).raw().signum();
    side(a0, a1, b0) * side(a0, a1, b1) < 0 && side(b0, b1, a0) * side(b0, b1, a1) < 0
}

#[test]
fn trebuchet_counterweight_stays_clear_of_the_frame_and_the_ground() {
    // R1.1800: the box drops through the middle of the A-frame
    let base = Vec2::ints(0, 0);
    for t in 0..=trebuchet::SWING_END {
        let p = pose(base, Phase::Swing(t), false);
        let b = p.box_;
        for k in 0..4 {
            let (c0, c1) = (b[k], b[(k + 1) % 4]);
            assert!(c0.y > p.base[0].y, "tick {t}: corner {c0:?} below the base beam");
            for foot in p.base {
                assert!(
                    !crosses(c0, c1, foot, p.pivot),
                    "tick {t}: box {b:?} crosses the frame"
                );
            }
        }
    }
}

/// Where a plain shot released `release` sixteenths into the swing first
/// dips below flat ground at height 0, free flight, no wind (raw Q16.16 x).
fn sixteenth_landing(release: u32) -> i64 {
    let l = trebuchet_launch_at(Vec2::ZERO, release, 0, false);
    let (mut p, mut v) = (l.pos, l.vel);
    while p.y >= Fx::ZERO {
        v.y -= TUNE.gravity;
        p += v;
    }
    p.x.raw()
}

#[test]
fn trebuchet_sixteenth_release_lands_between_whole_ticks() {
    // R1.2100: through the flat part of the forward window each sixteenth
    // of a tick moves the landing a little, the same way, and none jumps
    // more than a quarter of the whole tick's move
    // the ticks doubled when R1.3700 halved the arm speed and moved off the top of the range (were 65, 70, 85)
    for t in [105u32, 115, 125] {
        let a = sixteenth_landing(t * RELEASE_PARTS);
        let b = sixteenth_landing((t + 1) * RELEASE_PARTS);
        let whole = (b - a).abs();
        assert!(whole > Fx::ONE.raw(), "tick {t}: whole-tick move {whole}");
        let mut prev = a;
        for f in 1..=RELEASE_PARTS {
            let x = sixteenth_landing(t * RELEASE_PARTS + f);
            let step = x - prev;
            assert!(
                (step > 0) == (b > a) && step != 0,
                "tick {t} part {f}: step {step}"
            );
            assert!(
                step.abs() * 4 <= whole,
                "tick {t} part {f}: step {step} of {whole}"
            );
            prev = x;
        }
        assert_eq!(prev, b);
    }
}

#[test]
fn a_ball_landing_on_an_upward_slope_bounces_and_does_not_ride_up_it() {
    // R1.3705, the person: "a ball traveling towards a 20 degree upwards
    // slope hit it while traveling at a 10 degree downwards trajectory and
    // instead of hitting the ground and bouncing it just kinda stuck and slid
    // up the slope". Level to x = 20, then rising tan(20 deg) = 0.364 a column.
    let heights = (0..80i64)
        .map(|c| Fx::ratio(364 * (c - 20).max(0), 1000))
        .collect();
    let mut w = World::new(Terrain {
        col_width: Fx::ONE,
        heights,
    });
    let r = Fx::HALF;
    let start = Vec2::new(
        Fx::from_int(21),
        w.terrain.height_at(Fx::from_int(21)) + r + Fx::ratio(1, 10),
    );
    // half a unit a tick, 10 degrees down: (cos 10, -sin 10)
    let vel = Vec2::new(Fx::ratio(492, 1000), -Fx::ratio(87, 1000));
    let i = w.fire(shot(start, vel), r, Fx::from_int(6));
    let (mut touched, mut bounced, mut run, mut longest) = (false, false, 0, 0);
    for _ in 0..240 {
        w.step();
        let b = w.projectiles[i];
        if !b.alive {
            break;
        }
        let on = b.pos.y - b.radius <= w.terrain.height_at(b.pos.x) + Fx::ratio(1, 64);
        touched |= on;
        bounced |= touched && !on && b.vel.y > Fx::ZERO;
        // sliding up: on the ground and still moving uphill
        run = if on && b.vel.x > Fx::ratio(1, 100) {
            run + 1
        } else {
            0
        };
        longest = longest.max(run);
    }
    assert!(touched && bounced, "touched {touched}, bounced {bounced}");
    assert!(longest <= 2, "slid up the slope for {longest} ticks");
}
