//! The trebuchet as a pure function: from the tick the player releases
//! (counted from the swing start) and the power level, where the shot leaves
//! and how fast. The arm starts pointing back and down and swings over the
//! top, speeding up; the shot leaves along the arm tip's path. Early release
//! lobs high, later is flatter, too late throws into the ground, and too
//! early throws backward.

use super::fixed::{brads_to_radians, cos, sin, Fx, Vec2, TURN};
use super::tune::TUNE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    pub pos: Vec2,
    pub vel: Vec2,
}

/// Arm angle (brads, 0 = pointing forward, counterclockwise up) and arm
/// speed (brads per tick, the arm turns clockwise) `t` ticks into the swing.
pub fn arm_at(t: u32) -> (i64, i64) {
    let t = t as i64;
    let top = TUNE.treb_top_speed;
    let ramp = TUNE.treb_ramp.max(1);
    let (swept, speed) = if t <= ramp {
        (top * t * t / (2 * ramp), top * t / ramp)
    } else {
        (top * ramp / 2 + top * (t - ramp), top)
    };
    // the arm stops pointing straight down; a release after that throws backward
    let last = TUNE.treb_start_angle + TURN / 4;
    (TUNE.treb_start_angle - swept.min(last), speed)
}

/// Where and how fast a shot leaves a trebuchet standing at `base`.
/// `facing_left` mirrors it for a trebuchet throwing toward smaller x.
pub fn trebuchet_launch(base: Vec2, release_tick: u32, power: u8, facing_left: bool) -> Launch {
    let (angle, speed) = arm_at(release_tick);
    let (c, s) = (cos(angle), sin(angle));
    let pivot = base + Vec2::new(Fx::ZERO, TUNE.treb_pivot_height);
    let tip = Vec2::new(c, s) * TUNE.treb_arm;
    let scale = Fx::ratio(TUNE.treb_power_base + power as i64, TUNE.treb_power_base);
    let tip_speed = brads_to_radians(speed) * TUNE.treb_arm * scale;
    // clockwise turning: the tip moves along (sin, -cos)
    let vel = Vec2::new(s, -c) * tip_speed;
    let (tip, vel) = if facing_left {
        (Vec2::new(-tip.x, tip.y), Vec2::new(-vel.x, vel.y))
    } else {
        (tip, vel)
    };
    Launch {
        pos: pivot + tip,
        vel,
    }
}
