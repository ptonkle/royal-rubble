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
