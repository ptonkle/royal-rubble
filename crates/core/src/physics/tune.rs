//! Every physics number in one table. A world unit is about one floor
//! height; a tick is 1/60 s. Speeds are units per tick, accelerations units
//! per tick squared. Loads and strengths share one unit: the per-tick
//! correction a constraint makes, so `weight(m)` is the load of holding up a
//! mass `m` against gravity.

use super::fixed::{Fx, TURN};

/// Ticks per second.
pub const TICKS_PER_SECOND: u32 = 60;

/// Most substeps a fast ball's move is cut into for swept collision.
pub const MAX_SUBSTEPS: i64 = 8;

/// Gravity: 10 units/s^2 at 60 Hz, as a raw Q16.16 per tick squared.
pub const G_RAW: i64 = 182;

/// The load of holding mass `m` (whole units) against gravity.
pub const fn weight(m: i64) -> Fx {
    Fx(G_RAW * m)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Material {
    Wood,
    Stone,
    Metal,
    Rope,
}

impl Material {
    pub const ALL: [Material; 4] = [Material::Wood, Material::Stone, Material::Metal, Material::Rope];
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialTune {
    /// Mass per unit of length.
    pub mass_per_unit: Fx,
    /// Load it carries pulled.
    pub tension: Fx,
    /// Load it carries pushed (ropes: unused, they go slack).
    pub compression: Fx,
    /// Bending load its joints carry.
    pub bending: Fx,
    pub max_len: Fx,
    /// Hit points against impacts and fire, in impulse units.
    pub hp: Fx,
    /// Thickness, for collisions.
    pub thickness: Fx,
    pub burns: bool,
    /// Ropes: tension only, no joints, projectiles pass through.
    pub rope: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Tune {
    /// The snap grid for nodes.
    pub grid: Fx,
    pub gravity: Fx,
    /// Fraction of velocity a node keeps each tick.
    pub damping: Fx,
    pub iterations: u32,
    /// After a build, ticks the castle settles before strain can break it.
    pub settle_ticks: u32,
    /// How hard joints hold their angle, per iteration (0..1).
    pub joint_stiffness: Fx,
    /// Strain is smoothed: new = (old * (n - 1) + now) / n.
    pub strain_smoothing: i64,
    pub wood: MaterialTune,
    pub stone: MaterialTune,
    pub metal: MaterialTune,
    pub rope: MaterialTune,
    /// Bounce of a shot off a beam.
    pub restitution: Fx,
    /// Fraction of horizontal speed kept on touching the ground.
    pub ground_friction: Fx,
    /// Fraction of vertical speed a ball keeps bouncing off the ground.
    pub ground_bounce: Fx,
    /// A ball slower than this on the ground stops and is removed.
    pub rest_speed: Fx,
    /// Impulses at or below this deal no damage (resting contact).
    pub min_damage_impulse: Fx,
    /// Debris pieces ignore beams for this many ticks after breaking off.
    pub debris_grace: u32,
    pub debris_max_radius: Fx,
    /// Debris disappears after this many ticks.
    pub debris_life: u32,
    /// Debris impulse that crushes a body.
    pub crush_impulse: Fx,
    /// A drop larger than this kills a body (about three floor heights).
    pub fall_kill: Fx,
    /// A body moving down faster than this is still falling.
    pub fall_speed: Fx,
    pub burn_ticks: u32,
    /// Hit points a burning segment loses per tick.
    pub burn_damage: Fx,
    /// Fire spreads to touching wood every this many ticks.
    pub burn_spread_every: u32,
    pub phase_ticks: u32,
    /// Hover: ticks a shot hangs still before it falls.
    pub hover_ticks: u32,
    /// Split: angle between the pieces, in brads.
    pub split_spread: i64,
    /// Boost: added speed along the flight path.
    pub boost_speed: Fx,
    /// Trebuchet: pivot height above its base, arm length.
    pub treb_pivot_height: Fx,
    pub treb_arm: Fx,
    /// Arm angle at swing start (brads, 0 = pointing forward).
    pub treb_start_angle: i64,
    /// Arm speed through the swing: (tick, brads per tick) knots, joined by
    /// straight lines. A short speed-up, then a steady natural speed, then a
    /// quick stop (R1.1803). The last knot is where it stops.
    pub treb_swing: [(i64, i64); 6],
    /// Launch speed multiplier is (treb_power_base + power) / treb_power_base.
    pub treb_power_base: i64,
    /// The shot leaves at this percent of the pouch's real speed: the one
    /// knob that sets the range (R1.1803).
    pub treb_speed_pct: i64,
    /// Sling length (tip to pouch).
    pub treb_sling: Fx,
    /// How far ahead of the arm's line the sling starts (brads): it lies
    /// forward along the ground and catches up with the arm as the arm stops.
    pub treb_sling_lead: i64,
    /// How far past the arm's line the sling ends (brads), when the arm stops.
    pub treb_sling_past: i64,
    /// Short arm (pivot to the counterweight's hinge), the hinge rod, and
    /// the counterweight box's side.
    pub treb_short_arm: Fx,
    pub treb_hinge: Fx,
    pub treb_weight_side: Fx,
    /// Half the base beam's length, and the wheels' radius.
    pub treb_base_half: Fx,
    pub treb_wheel_r: Fx,
    /// World edge margin past which balls are removed.
    pub edge_margin: Fx,
}

pub const TUNE: Tune = Tune {
    grid: Fx::ratio(1, 4),
    gravity: weight(1),
    damping: Fx::ratio(63, 64),
    iterations: 10,
    settle_ticks: 60,
    joint_stiffness: Fx::ratio(1, 32),
    strain_smoothing: 6,
    wood: MaterialTune {
        mass_per_unit: Fx::from_int(1),
        tension: weight(30),
        compression: weight(30),
        bending: weight(4),
        max_len: Fx::from_int(2),
        hp: Fx::ratio(1, 2),
        thickness: Fx::ratio(1, 8),
        burns: true,
        rope: false,
    },
    stone: MaterialTune {
        mass_per_unit: Fx::from_int(3),
        tension: weight(30),
        compression: weight(300),
        bending: weight(12),
        max_len: Fx::from_int(2),
        hp: Fx::from_int(2),
        thickness: Fx::ratio(1, 4),
        burns: false,
        rope: false,
    },
    metal: MaterialTune {
        mass_per_unit: Fx::from_int(2),
        tension: weight(300),
        compression: weight(300),
        bending: weight(40),
        max_len: Fx::from_int(3),
        hp: Fx::from_int(3),
        thickness: Fx::ratio(1, 8),
        burns: false,
        rope: false,
    },
    rope: MaterialTune {
        mass_per_unit: Fx::ratio(1, 4),
        tension: weight(40),
        compression: weight(100_000),
        bending: weight(100_000),
        max_len: Fx::from_int(4),
        hp: Fx::ratio(1, 2),
        thickness: Fx::ratio(1, 16),
        burns: false,
        rope: true,
    },
    restitution: Fx::ratio(1, 4),
    ground_friction: Fx::ratio(1, 2),
    ground_bounce: Fx::ratio(1, 4),
    rest_speed: Fx::ratio(1, 200),
    min_damage_impulse: Fx::ratio(1, 50),
    debris_grace: 6,
    debris_max_radius: Fx::ratio(1, 4),
    debris_life: 240,
    crush_impulse: Fx::ratio(1, 10),
    fall_kill: Fx::from_int(3),
    fall_speed: Fx::ratio(1, 100),
    burn_ticks: 300,
    burn_damage: Fx::ratio(1, 600),
    burn_spread_every: 60,
    phase_ticks: 30,
    hover_ticks: 60,
    split_spread: TURN / 36,
    boost_speed: Fx::ratio(1, 10),
    treb_pivot_height: Fx::ratio(7, 2),
    treb_arm: Fx::from_int(4),
    treb_start_angle: TURN * 5 / 8,
    treb_swing: [(0, 0), (15, 305), (50, 305), (80, 305), (102, 305), (110, 0)],
    treb_power_base: 100,
    treb_speed_pct: 234,
    treb_sling: Fx::from_int(4),
    treb_sling_lead: TURN * 3 / 8,
    treb_sling_past: TURN / 36,
    treb_short_arm: Fx::ratio(2, 3),
    treb_hinge: Fx::ratio(3, 2),
    treb_weight_side: Fx::ratio(4, 5),
    treb_base_half: Fx::from_int(3),
    treb_wheel_r: Fx::ratio(1, 4),
    edge_margin: Fx::from_int(64),
};

impl Tune {
    pub fn material(&self, m: Material) -> &MaterialTune {
        match m {
            Material::Wood => &self.wood,
            Material::Stone => &self.stone,
            Material::Metal => &self.metal,
            Material::Rope => &self.rope,
        }
    }
}
