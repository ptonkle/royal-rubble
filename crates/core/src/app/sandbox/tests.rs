use super::*;
use crate::app::field::Camera;

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
    let l = crate::physics::trebuchet_launch(s.engine_base(), t, power(), false);
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
    let lands: Vec<(u32, i64)> = (0..Sandbox::swing_end())
        .map(|t| (t, landing(&s, t).floor_int()))
        .collect();
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
fn reach_past_the_circle_gives_the_full_length_the_cursor_points() {
    let a = Vec2::ints(50, 30);
    let max = Fx::from_int(2);
    // two cursors on the line 3 across for 1 up, both past the 2-cell reach
    let near = super::reach_end(a, Vec2::ints(53, 31), max);
    let far = super::reach_end(a, Vec2::ints(59, 33), max);
    assert_eq!(near, far);
    let len = (far - a).len();
    assert!(len <= max, "{len:?}");
    // the full-length point is (51.90, 30.63): within a grid step of it
    assert!(len >= max - TUNE.grid, "{len:?}");
    // and never shorter than a cursor just inside the circle that way
    let inside = super::reach_end(a, Vec2::new(Fx::ratio(5180, 100), Fx::ratio(3060, 100)), max);
    assert!((inside - a).len() <= len, "{inside:?} {far:?}");
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
    s.pump(0, KEY_SPACE, None);
    s.pump(16, 0, None);
    for _ in 0..39 {
        s.tick();
    }
    assert!(s.world.projectiles.is_empty());
    s.pump(700, KEY_SPACE, None);
    assert_eq!(s.world.projectiles.len(), 1);
}

#[test]
fn sandbox_siege_camera_aims_at_the_trebuchet_then_settles_on_the_castle() {
    let mut s = Sandbox::new(5);
    s.toggle_mode();
    let on_screen = |cam: &Camera, p: Vec2| {
        let [x, y] = cam.px(p);
        x >= 0 && x <= cam.w && y >= 0 && y <= cam.h
    };
    let base = s.engine_base();
    let castle = s.beams[0].from;
    assert!(on_screen(&s.camera_target(), base));
    assert!(!on_screen(&s.camera_target(), castle));
    s.trail = vec![castle];
    assert!(on_screen(&s.camera_target(), castle));
    assert!(!on_screen(&s.camera_target(), base));
}

/// The shot's sideways speed when Space lets go `press_after_us` after the
/// start of the last tick run, 30 ticks into the swing.
fn sandbox_thrown_vx(press_after_us: i64) -> Fx {
    let mut s = Sandbox::new(5);
    s.toggle_mode();
    s.pump(1000, 0, None);
    s.pump(1000, KEY_SPACE, Some(1_000_000));
    s.pump(1500, 0, None);
    // 60 ticks a second: 30 ticks run by 1500 ms
    let basis = 1_000_000 + 30 * 1_000_000 / 60;
    s.pump(1500, KEY_SPACE, Some(basis + press_after_us));
    let i = s.shot.expect("the shot is in the air");
    s.world.projectiles[i].vel.x
}

#[test]
fn sandbox_release_press_time_throws_between_whole_ticks() {
    // R1.2202: presses 0, 8 and 16 sixteenths into the tick (a tick is
    // 16,667 microseconds): the middle one throws between the other two
    let (a, b, c) = (
        sandbox_thrown_vx(100),
        sandbox_thrown_vx(100 + 8_334),
        sandbox_thrown_vx(100 + 16_667),
    );
    assert!(a != c, "{a:?} {c:?}");
    assert!((a < b && b < c) || (a > b && b > c), "{a:?} {b:?} {c:?}");
}

/// A sandbox siege with a shot let go `at` ticks into the swing.
fn sandbox_thrown(at: u32) -> Sandbox {
    let mut s = Sandbox::new(5);
    s.toggle_mode();
    s.space(None);
    for _ in 0..at {
        s.tick();
    }
    s.space(None);
    s
}

#[test]
fn sandbox_view_eases_after_the_hold_instead_of_jumping() {
    // R1.2201: once the 0.4 s hold ends the view glides to the flight view
    let mut s = sandbox_thrown(crate::physics::SWING_END * 2 / 3);
    for _ in 0..crate::app::arena::CAM_HOLD + 1 {
        s.tick();
    }
    let before = s.camera();
    s.tick();
    let (eye, target) = (s.camera(), s.camera_target());
    assert!(eye != target, "snapped to {target:?}");
    let moved = (eye.left - before.left).abs();
    let wanted = (target.left - before.left).abs();
    assert!(moved < wanted, "moved {moved:?} of {wanted:?}");
}
