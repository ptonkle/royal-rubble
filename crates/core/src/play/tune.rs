//! Every number the glue between rules and physics needs. Physics numbers
//! stay in `physics::tune`, rule numbers in `game::tune`.

use crate::physics::Fx;

/// Body radius of a royal and of a building, in cells. A royal's body is
/// its figure's shape (R1.2500): every point within `ROYAL_RADIUS` of an
/// upright `ROYAL_UPRIGHT` tall, its feet on the bottom of the lower
/// circle, so it reaches `FIGURE_TALL` and is as wide as the figure's torso.
pub const ROYAL_RADIUS: Fx = Fx::ratio(5, 16);
pub const ROYAL_UPRIGHT: Fx = Fx::ratio(7, 8);
pub const BUILDING_RADIUS: Fx = Fx::ratio(1, 2);

/// Head room under cover (R1.2500): a room one segment (2 cells) tall,
/// the bots' `ai::tune::ROOM_UNITS`, less half the thickest beam (stone)
/// below the royal's feet and half above its head.
pub const HEAD_ROOM: Fx = Fx::ratio(7, 4);
/// A royal's figure as drawn, in cells: its feet on the bottom of the body,
/// centered on it, at most this tall, and its body this wide (an arm held
/// out may reach past an upright and is drawn in front of it), so it stands
/// under any floor one room above it (R1.2500, R1.2400).
pub const FIGURE_TALL: Fx = Fx::ratio(3, 2);
pub const FIGURE_WIDE: Fx = Fx::ratio(3, 4);
/// The least a figure is drawn, in pixels, when the view is zoomed far out
/// (the corner inset); never more than the head room at that zoom.
pub const FIGURE_MIN_PX: i32 = 12;

/// A point stands on a floor when it is within this of the floor's top,
/// and on the ground within this of the ground (half a grid step).
pub const STAND_TOLERANCE: Fx = Fx::ratio(1, 8);

/// Ticks the strain colors' static test runs (after the physics settle).
pub const STATIC_TEST_TICKS: u32 = 120;

/// Wind per rule step (`game::tune::WIND_MAX` steps each way), raw Q16.16
/// units per tick squared. Gravity is 182.
pub const WIND_PER_STEP: i64 = 4;

/// After the siege timer, shots and debris in the air finish for at most
/// this many ticks before the outcome is counted.
pub const DRAIN_TICKS: u32 = 240;

/// After the siege timer, a player's swing or shot still moving holds the
/// round open for at most this many ticks before the drain (R1.3203).
pub const SHOT_WAIT_TICKS: u32 = 1200;

/// Second Volley: each extra copy of the first shot leaves this many
/// percent slower than the one before, so the copies land apart.
pub const VOLLEY_SLOWER_PCT: i64 = 6;

/// The stand-in shot plan for bot trebuchets until the aiming AI lands:
/// release this many ticks into the swing, one shot every `BOT_SHOT_EVERY`.
pub const BOT_RELEASE_TICK: u32 = 45;
pub const BOT_SHOT_EVERY: u32 = 150;
pub const BOT_FIRST_SWING: u32 = 30;
/// Bot shot planning per tick, across every field, in the aimer's work
/// units (`ai::tune::WORLD_STEP_COST`): about 4 ms on the 201-segment probe
/// castle (R1.2000, MEASUREMENTS.md).
pub const BOT_PLAN_BUDGET: u64 = 160_000;
