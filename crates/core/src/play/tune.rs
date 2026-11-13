//! Every number the glue between rules and physics needs. Physics numbers
//! stay in `physics::tune`, rule numbers in `game::tune`.

use crate::physics::Fx;

/// Body radius of a royal and of a building, in cells.
pub const ROYAL_RADIUS: Fx = Fx::ratio(1, 4);
pub const BUILDING_RADIUS: Fx = Fx::ratio(1, 2);

/// A point stands on a floor when it is within this of the floor's top,
/// and on the ground within this of the ground (half a grid step).
pub const STAND_TOLERANCE: Fx = Fx::ratio(1, 8);

/// Ticks the strain colors' static test runs (after the physics settle).
pub const STATIC_TEST_TICKS: u32 = 120;

/// The level-1 throw, in percent of the physics trebuchet at power 0.
/// The physics throw alone reaches about 20 cells, short of a far plot
/// (R1.703). A level's `shot_power_pct` scales this.
pub const LAUNCH_BASE_PCT: i64 = 200;

/// Wind per rule step (`game::tune::WIND_MAX` steps each way), raw Q16.16
/// units per tick squared. Gravity is 182.
pub const WIND_PER_STEP: i64 = 4;

/// After the siege timer, shots and debris in the air finish for at most
/// this many ticks before the outcome is counted.
pub const DRAIN_TICKS: u32 = 240;

/// Shot kinds: radius and mass in cells and physics mass units.
pub struct ShotRow {
    pub radius: Fx,
    pub mass: Fx,
    pub fire: bool,
    /// Splits into three as it leaves the arm.
    pub spread: bool,
}

pub const NORMAL_SHOT: ShotRow = ShotRow {
    radius: Fx::ratio(1, 4),
    mass: Fx::from_int(2),
    fire: false,
    spread: false,
};
pub const HEAVY_SHOT: ShotRow = ShotRow {
    radius: Fx::ratio(3, 8),
    mass: Fx::from_int(5),
    fire: false,
    spread: false,
};
pub const FIRE_SHOT: ShotRow = ShotRow {
    radius: Fx::ratio(1, 4),
    mass: Fx::from_int(2),
    fire: true,
    spread: false,
};
/// No blast in the physics yet: a big heavy shot stands in (R1.705).
pub const EXPLODING_SHOT: ShotRow = ShotRow {
    radius: Fx::ratio(1, 2),
    mass: Fx::from_int(4),
    fire: false,
    spread: false,
};
pub const SPREAD_SHOT: ShotRow = ShotRow {
    radius: Fx::ratio(1, 4),
    mass: Fx::from_int(3),
    fire: false,
    spread: true,
};

/// The stand-in shot plan for bot trebuchets until the aiming AI lands:
/// release this many ticks into the swing, one shot every `BOT_SHOT_EVERY`.
pub const BOT_RELEASE_TICK: u32 = 38;
pub const BOT_SHOT_EVERY: u32 = 150;
pub const BOT_FIRST_SWING: u32 = 30;
