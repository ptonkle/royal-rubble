//! The trebuchet as a pure function: from the tick the player releases
//! (counted from the swing start) and the power level, where the shot leaves
//! and how fast (R1.1601).
//!
//! The long arm starts pointing back and down, with the sling lying forward
//! along the ground under the frame. The counterweight drops: the arm speeds
//! up once and slows once (`TUNE.treb_swing`, no stalls). The sling turns
//! faster than the arm and whips over the top; the shot leaves the pouch
//! along the pouch's path, so the sling's angle sets the arc and its whip
//! adds to the arm tip's speed. An early release throws backward or lobs
//! high, a later one is flatter, and too late throws into the ground.
//!
//! The swing is shown slowed down so it can be timed: the shot leaves at
//! `treb_time_scale` percent of the pouch's speed (R1.1602).
//!
//! [`pose`] gives every point the drawing needs, including the follow-through
//! after release and the counterweight swinging on its hinge.

use super::fixed::{brads_to_radians, cos, sin, Fx, Vec2, TURN};
use super::tune::{weight, TUNE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    pub pos: Vec2,
    pub vel: Vec2,
}

/// Arm speed `t` ticks into the swing, from the `treb_swing` knots.
const fn arm_speed(t: i64) -> i64 {
    let knots = &TUNE.treb_swing;
    let mut k = 1;
    while k < knots.len() {
        let ((t0, v0), (t1, v1)) = (knots[k - 1], knots[k]);
        if t <= t1 {
            let t = if t < t0 { t0 } else { t };
            let d = if t1 - t0 > 1 { t1 - t0 } else { 1 };
            return v0 + (v1 - v0) * (t - t0) / d;
        }
        k += 1;
    }
    knots[knots.len() - 1].1
}

/// The tick the arm stops: a held swing throws by itself then.
pub const SWING_END: u32 = TUNE.treb_swing[TUNE.treb_swing.len() - 1].0 as u32;

/// Twice the angle the arm sweeps in its first `t` ticks: the speed summed
/// by trapezoids, one per tick.
const fn swept2(t: i64) -> i64 {
    let mut s = 0;
    let mut k = 1;
    while k <= t {
        s += arm_speed(k - 1) + arm_speed(k);
        k += 1;
    }
    s
}

/// The whole angle the arm sweeps, brads.
const SWEEP: i64 = swept2(SWING_END as i64) / 2;

/// Arm angle where the arm stops.
pub const END_ANGLE: i64 = TUNE.treb_start_angle - SWEEP;

/// Arm angle (brads, 0 = pointing forward, counterclockwise up) and arm
/// speed (brads per tick, the arm turns clockwise) `t` ticks into the swing.
pub fn arm_at(t: u32) -> (i64, i64) {
    let t = (t as i64).min(SWING_END as i64);
    (TUNE.treb_start_angle - swept2(t) / 2, arm_speed(t))
}

/// Arm angle where the counterweight's drop ends and the arm turns slowly.
const SLOW_ANGLE: i64 = TUNE.treb_start_angle - swept2(TUNE.treb_swing[2].0) / 2;

/// The sling's direction (tip to pouch, brads) at arm angle `arm`. It hangs
/// at a fixed angle to the arm while the counterweight drops, then swings
/// round faster than the arm and passes the arm's line by `treb_sling_past`
/// as the arm stops, so a release that late throws into the ground.
fn sling_angle(arm: i64) -> i64 {
    let lead = TUNE.treb_sling_lead;
    let past = TUNE.treb_sling_past;
    if arm >= SLOW_ANGLE {
        arm + lead
    } else {
        arm + (lead + past) * (arm - END_ANGLE) / (SLOW_ANGLE - END_ANGLE) - past
    }
}

/// How much faster than the arm the sling turns, in percent, at arm angle `arm`.
fn sling_pct(arm: i64) -> i64 {
    if arm >= SLOW_ANGLE {
        100
    } else {
        let sweep = TUNE.treb_sling_lead + TUNE.treb_sling_past;
        100 + 100 * sweep / (SLOW_ANGLE - END_ANGLE)
    }
}

fn unit(a: i64) -> Vec2 {
    Vec2::new(cos(a), sin(a))
}

/// Long-arm tip and pouch at swing tick `t`, for a trebuchet facing right
/// standing at the origin, and the pouch's velocity per radian the arm turns
/// with the arm's speed (brads per tick): the velocity is their product, kept
/// apart so a slow arm loses no precision in the direction.
fn sling_at(t: u32) -> (Vec2, Vec2, Vec2, i64) {
    let (arm, speed) = arm_at(t);
    let phi = sling_angle(arm);
    let pivot = Vec2::new(Fx::ZERO, TUNE.treb_pivot_height);
    let tip = pivot + unit(arm) * TUNE.treb_arm;
    let pouch = tip + unit(phi) * TUNE.treb_sling;
    // clockwise turning: a point at angle a moves along (sin a, -cos a)
    let along = |a: i64| Vec2::new(sin(a), -cos(a));
    let shape = along(arm) * TUNE.treb_arm + along(phi) * (TUNE.treb_sling * sling_pct(arm) / 100);
    (tip, pouch, shape, speed)
}

fn mirror(v: Vec2, facing_left: bool) -> Vec2 {
    if facing_left {
        Vec2::new(-v.x, v.y)
    } else {
        v
    }
}

/// Where and how fast a shot leaves a trebuchet standing at `base`.
/// `facing_left` mirrors it for a trebuchet throwing toward smaller x.
pub fn trebuchet_launch(base: Vec2, release_tick: u32, power: u8, facing_left: bool) -> Launch {
    let (_, pouch, shape, speed) = sling_at(release_tick);
    let base_pct = TUNE.treb_power_base * 100;
    let w = speed * (TUNE.treb_power_base + power as i64) * TUNE.treb_time_scale / base_pct;
    let vel = shape * brads_to_radians(w);
    Launch {
        pos: base + mirror(pouch, facing_left),
        vel: mirror(vel, facing_left),
    }
}

/// Every point of a trebuchet's drawing, in world units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    /// the two ends of the base beam, and the wheel centers under them
    pub base: [Vec2; 2],
    pub wheels: [Vec2; 2],
    pub wheel_r: Fx,
    pub pivot: Vec2,
    /// the long arm's tip and the short arm's end (the hinge)
    pub tip: Vec2,
    pub short: Vec2,
    /// the counterweight box, corners in order, hanging from `short`
    pub box_: [Vec2; 4],
    /// the brace across the two uprights
    pub brace: [Vec2; 2],
    /// the sling: tip, a middle point, pouch
    pub sling: [Vec2; 3],
    /// the shot sits in the pouch until release
    pub loaded: bool,
}

/// Where the swing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// cocked, waiting for the swing
    Rest,
    /// `t` ticks into the swing, shot in the pouch
    Swing(u32),
    /// released at swing tick `at`, `since` ticks ago
    After { at: u32, since: u32 },
}

/// Ticks the arm rocks after a release before it is winched back.
pub const ROCK_TICKS: u32 = 60;
/// Ticks the winch takes to pull the arm back to the cocked angle.
pub const WINCH_TICKS: u32 = 60;

/// A point on a rope or hinge, stepped by Verlet and kept `len` from its anchor.
#[derive(Clone, Copy)]
struct Bob {
    p: Vec2,
    prev: Vec2,
}

impl Bob {
    fn step(&mut self, anchor: Vec2, len: Fx) {
        let g = Vec2::new(Fx::ZERO, weight(1));
        // a little air drag so it settles
        let v = (self.p - self.prev) * Fx::ratio(63, 64);
        self.prev = self.p;
        let mut p = self.p + v - g;
        let d = p - anchor;
        let l = d.len();
        if l > Fx::ZERO {
            p = anchor + d * (len / l);
        }
        self.p = p;
    }
}

fn short_end(pivot: Vec2, arm: i64) -> Vec2 {
    pivot - unit(arm) * TUNE.treb_short_arm
}

/// The drawing of a trebuchet standing at `base` in `phase`.
pub fn pose(base: Vec2, phase: Phase, facing_left: bool) -> Pose {
    let pivot = Vec2::new(Fx::ZERO, TUNE.treb_pivot_height);
    let start = TUNE.treb_start_angle;
    let hang = TUNE.treb_hinge;
    let down = Vec2::new(Fx::ZERO, -hang);
    // the counterweight hangs straight down from its hinge through the
    // swing, so it drops through the middle of the frame (R1.1800); after
    // release it sways on the hinge
    let swing_to = match phase {
        Phase::Rest => 0,
        Phase::Swing(t) => t.min(SWING_END),
        Phase::After { at, .. } => at.min(SWING_END),
    };
    let s1 = short_end(pivot, arm_at(swing_to).0);
    let s0 = short_end(pivot, arm_at(swing_to.saturating_sub(1)).0);
    let mut cw = Bob {
        p: s1 + down,
        prev: s0 + down,
    };
    let (arm, tip, pouch, loaded) = match phase {
        Phase::Rest => {
            let (tip, pouch, _, _) = sling_at(0);
            (start, tip, pouch, true)
        }
        Phase::Swing(t) => {
            let (tip, pouch, _, _) = sling_at(t);
            (arm_at(t).0, tip, pouch, true)
        }
        Phase::After { at, since } => {
            let (mut arm, speed) = arm_at(at);
            let (_, pouch0, shape, w) = sling_at(at);
            let vel0 = shape * brads_to_radians(w);
            let mut w = -speed;
            let mut rope = Bob {
                p: pouch0,
                prev: pouch0 - vel0,
            };
            let rest = TURN / 4;
            for _ in 1..=since.min(ROCK_TICKS) {
                // a damped rock about straight up: the counterweight hangs below
                w = w - (arm - rest) / 96 - w / 12;
                arm += w;
                let s = short_end(pivot, arm);
                cw.step(s, hang);
                let tip = pivot + unit(arm) * TUNE.treb_arm;
                rope.step(tip, TUNE.treb_sling);
            }
            if since > ROCK_TICKS {
                // the winch pulls it back to the cocked angle, easing in
                // and out (smoothstep)
                let k = (since - ROCK_TICKS).min(WINCH_TICKS) as i64;
                if k == WINCH_TICKS as i64 {
                    return pose(base, Phase::Rest, facing_left);
                }
                let n = WINCH_TICKS as i64;
                let (num, den) = (k * k * (3 * n - 2 * k), n * n * n);
                arm += (start - arm) * num / den;
                let e = Fx::ratio(num, den);
                let tip = pivot + unit(arm) * TUNE.treb_arm;
                let lie = tip + Vec2::new(TUNE.treb_sling, Fx::ZERO);
                rope.p = Vec2::lerp(rope.p, lie, e);
                let s = short_end(pivot, arm);
                cw.p = Vec2::lerp(cw.p, s + down, e);
            }
            let tip = pivot + unit(arm) * TUNE.treb_arm;
            (arm, tip, rope.p, false)
        }
    };
    let short = short_end(pivot, arm);
    let dir = match phase {
        Phase::After { .. } => (cw.p - short).normalized(),
        _ => Vec2::new(Fx::ZERO, -Fx::ONE),
    };
    let side = TUNE.treb_weight_side;
    let across = Vec2::new(dir.y, -dir.x) * (side / 2);
    let top = short + dir * hang;
    let bottom = top + dir * side;
    let box_ = [top - across, top + across, bottom + across, bottom - across];
    let half = TUNE.treb_base_half;
    let wheel_r = TUNE.treb_wheel_r;
    let feet = [Vec2::new(-half, wheel_r), Vec2::new(half, wheel_r)];
    let mid = |a: Vec2, b: Vec2| Vec2::lerp(a, b, Fx::HALF);
    let brace = [mid(feet[0], pivot), mid(feet[1], pivot)];
    let sag = Vec2::new(Fx::ZERO, if loaded { Fx::ZERO } else { TUNE.treb_sling / 8 });
    let sling = [tip, mid(tip, pouch) - sag, pouch];
    let at = |v: Vec2| base + mirror(v, facing_left);
    Pose {
        base: feet.map(at),
        wheels: [Vec2::new(-half, wheel_r), Vec2::new(half, wheel_r)].map(at),
        wheel_r,
        pivot: at(pivot),
        tip: at(tip),
        short: at(short),
        box_: box_.map(at),
        brace: brace.map(at),
        sling: sling.map(at),
        loaded,
    }
}
