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

/// A beam may come this close inside a royal's body without counting as
/// through it, so a floor at its feet or a slope beside it is allowed.
pub const BODY_SLACK: Fx = Fx::ratio(1, 32);

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

/// The player's build grid, in cells: segment ends snap to half a cell,
/// twice the physics grid (`TUNE.grid`, which stays the unit of `GridPt`
/// for ruins, bots and terrain) (R1.4300).
pub const SNAP_STEP: Fx = Fx::ratio(1, 2);
/// A press within this of a placed node picks that node, so a ruin's or a
/// bot's quarter-cell node can still be built from (R1.4300).
pub const NODE_PULL: Fx = Fx::ratio(1, 4);
/// Placed nodes within this of the cursor are ringed while you build.
pub const NODE_NEAR: Fx = Fx::from_int(3);

/// The Test button runs your castle's physics live for this many ticks
/// (3 seconds) so you see what sags or falls, then drops the copy (R1.4303).
pub const TEST_RUN_TICKS: u32 = 180;

/// The player's own view in build and siege (R1.4800): each wheel notch
/// zooms by this ratio (num, den) at the cursor, the view is never narrower
/// or wider than these (world units), held keys pan it this many screen
/// pixels a second (at most this many milliseconds of a slow frame count),
/// and its bottom rises at most this far above the auto view's.
pub const VIEW_ZOOM_STEP: (i64, i64) = (9, 8);
pub const VIEW_MIN_W: i64 = 12;
pub const VIEW_MAX_W: i64 = 240;
pub const VIEW_PAN_PX_PER_S: i64 = 1200;
pub const VIEW_PAN_MAX_MS: i64 = 100;
pub const VIEW_ROOF: i64 = 60;
/// A right press pans once it has moved this many pixels; short of that
/// its release lets go of a segment's start or sells (R1.4801).
pub const VIEW_DRAG_PX: i32 = 6;
