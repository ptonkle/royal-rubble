use super::*;

#[test]
fn sandbox_starts_with_a_castle_that_stands_a_test() {
    let mut s = Sandbox::new(5);
    assert!(s.beams.len() > 20);
    assert_eq!(s.world.bodies.len(), 3);
    s.run_test();
    let strain = s.test.clone().unwrap();
    assert!(strain.iter().all(|&v| v < Fx::ONE), "{strain:?}");
}

/// Where a shot released at tick `t` first comes down to the ground, with
/// no wind and nothing in the way.
fn landing(s: &Sandbox, t: u32) -> Fx {
    let l = trebuchet_launch(s.engine_base(), t, POWER, false);
    let (mut p, mut v) = (l.pos, l.vel);
    let mut n = 0;
    while p.y > s.ground(p.x) && n < 2000 {
        v.y -= TUNE.gravity;
        p += v;
        n += 1;
    }
    p.x
}

#[test]
fn sandbox_power_reaches_the_whole_level_one_plot() {
    let s = Sandbox::new(5);
    let (zl, zr, _) = s.zone();
    let lands: Vec<(u32, i64)> = (20..80).map(|t| (t, landing(&s, t).floor_int())).collect();
    let inside: Vec<u32> = lands
        .iter()
        .filter(|(_, x)| *x >= zl.floor_int() && *x <= zr.floor_int())
        .map(|(t, _)| *t)
        .collect();
    let far = lands.iter().map(|(_, x)| *x).max().unwrap();
    assert!(
        far >= zr.floor_int() - 4,
        "farthest {far}, zone {}..{}: {lands:?}",
        zl.floor_int(),
        zr.floor_int()
    );
    assert!(inside.len() >= 6, "{inside:?} {lands:?}");
}

#[test]
fn sandbox_drag_is_pulled_back_inside_the_material_reach() {
    let s = Sandbox::new(5);
    let a = Vec2::ints(50, 30);
    // wood reaches 2: a drag 5 to the right ends 2 to the right, on the grid
    assert_eq!(s.drag_end(a, Vec2::ints(55, 30)), Vec2::ints(52, 30));
    // a short drag snaps to the nearest grid point
    assert_eq!(
        s.drag_end(
            a,
            Vec2::new(Fx::from_int(51) + Fx::ratio(1, 10), Fx::from_int(30))
        ),
        Vec2::ints(51, 30)
    );
}

#[test]
fn sandbox_selling_a_floor_takes_its_royal_with_it() {
    let mut s = Sandbox::new(5);
    let seat = s.royals[0];
    // past the royal's reach, still on its floor
    let at = s.seat_pos(&seat) + Vec2::new(Fx::ratio(2, 5), Fx::ZERO);
    let (beams, royals) = (s.beams.len(), s.royals.len());
    s.sell(at);
    assert_eq!(s.beams.len(), beams - 1);
    assert_eq!(s.royals.len(), royals - 1);
}

#[test]
fn sandbox_space_twice_throws_one_shot() {
    let mut s = Sandbox::new(5);
    s.toggle_mode();
    s.pump(0, KEY_SPACE);
    s.pump(16, 0);
    for _ in 0..39 {
        s.tick();
    }
    assert!(s.world.projectiles.is_empty());
    s.pump(700, KEY_SPACE);
    assert_eq!(s.world.projectiles.len(), 1);
}
