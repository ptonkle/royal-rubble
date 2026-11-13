//! Every bot-intelligence number in one table: how hard each difficulty
//! searches, how much it misses on purpose, how a shot is scored, and the
//! castle templates' sizes and budgets.

use crate::game::flow::Difficulty;

/// How one difficulty aims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AimTune {
    /// Candidates checked with the full physics after the quick flight pass.
    pub verified: usize,
    /// Air-effect trigger ticks tried every this many ticks of flight.
    pub trigger_stride: u32,
    /// Chance (num, den) of a deliberate miss on the release tick.
    pub miss_chance: (u32, u32),
    /// Largest deliberate release error, in ticks.
    pub miss_ticks: i32,
    /// Also check the best plan with each owned air effect.
    pub each_effect: bool,
    /// Plain shots whose release is then tried at every sixteenth of a
    /// tick either side (R1.2100).
    pub refine: usize,
}

pub fn aim(d: Difficulty) -> AimTune {
    match d {
        Difficulty::Easy => AimTune {
            verified: 1,
            trigger_stride: 8,
            miss_chance: (1, 2),
            miss_ticks: 2,
            each_effect: false,
            refine: 0,
        },
        Difficulty::Normal => AimTune {
            verified: 1,
            trigger_stride: 4,
            miss_chance: (1, 4),
            miss_ticks: 1,
            each_effect: false,
            refine: 1,
        },
        Difficulty::Hard => AimTune {
            verified: 2,
            trigger_stride: 2,
            miss_chance: (0, 1),
            miss_ticks: 0,
            each_effect: true,
            refine: 3,
        },
    }
}

/// The aimer tries every release tick up to this one (from the swing start)
/// that throws forward with the real trebuchet, so a retune of the arm moves
/// the window by itself.
pub const RELEASE_SCAN_MAX: u32 = crate::physics::SWING_END;

/// Longest flight the quick pass follows, in ticks.
pub const FLIGHT_MAX: u32 = 360;
/// Air effects are tried only while the shot is within this many world
/// units, sideways, of a target.
pub const TRIGGER_REACH_UNITS: i64 = 10;
/// Full-physics ticks after the shot reaches the castle.
pub const VERIFY_TICKS: u32 = 40;
/// Full-physics ticks after every shot is spent, for falls and crushes.
pub const AFTER_TICKS: u32 = 10;
/// The castle's box is widened by this many raw Q16.16 units (one world unit)
/// when deciding where the full physics takes over.
pub const BOX_MARGIN_RAW: i64 = 1 << 16;

/// Work units the sliced aimer charges (R1.2000): one free-flight tick of
/// one shot costs 1 plus 1 per target it is measured against; one
/// full-physics step of the castle costs `WORLD_STEP_COST`; cloning the
/// castle's world costs `CLONE_COST`. Calibrated on the 201-segment probe
/// castle (MEASUREMENTS.md), where one unit is roughly 15-25 ns.
pub const WORLD_STEP_COST: u64 = 9_000;
pub const CLONE_COST: u64 = 1_500;

/// Score of a verified shot: per point of royal damage killed, per segment
/// broken near a royal, and a bonus that falls off with the closest approach.
pub const KILL_SCORE: i64 = 1_000;
pub const NEAR_BREAK_SCORE: i64 = 20;
/// "Near a royal": within this many world units.
pub const NEAR_UNITS: i64 = 2;
/// Closeness bonus starts at this many world units and reaches 0 at the
/// distance; scored per 1/16 unit.
pub const CLOSE_UNITS: i64 = 8;

// ---- Castle building ------------------------------------------------------

/// Level from which a bot builds in stone, and in metal.
pub const STONE_LEVEL: u8 = 4;
pub const METAL_LEVEL: u8 = 7;

/// World units between two columns of a castle, and one floor's height.
pub const BAY_UNITS: i64 = 1;
pub const FLOOR_UNITS: i64 = 1;

/// Columns kept clear between the plot's front edge and the castle.
pub const FRONT_SETBACK: usize = 4;
/// Most the ground may rise under a castle, raw Q16.16 (one unit).
pub const MAX_RISE_RAW: i64 = 1 << 16;
/// Widest a template's width parameter goes, in bays.
pub const MAX_BAYS: i64 = 20;

/// A stronger column material is chosen when its best castle is worth at
/// least this share (num, den) of the best castle in any material.
pub const STRONGER_SHARE: (u32, u32) = (1, 2);

/// A royal's radius (the physics tests' body size).
pub fn royal_radius() -> crate::physics::Fx {
    crate::physics::Fx::ratio(1, 4)
}

/// Floors a template may rise to, by level (index level - 1).
pub const MAX_FLOORS: [i64; 9] = [3, 3, 4, 4, 5, 5, 6, 6, 7];

/// Gold a bot keeps banked for interest, by difficulty and round: nothing
/// in the first rounds, so round 1 builds a castle (R1.1452).
pub fn reserve_gold(d: Difficulty, round: u32) -> u32 {
    let steps = match d {
        Difficulty::Easy => 0,
        Difficulty::Normal => u32::from(round >= 3),
        Difficulty::Hard => u32::from(round >= 2) + u32::from(round >= 6),
    };
    steps * crate::game::tune::INTEREST_STEP
}

/// A bot buys producers only once its castle has this many segments, so
/// the first rounds' gold goes into walls.
pub const PRODUCERS_AFTER_SEGMENTS: usize = 12;

/// Producers a bot buys when unlocked and affordable, in order.
pub const BOT_PRODUCERS: [crate::game::ledger::BuildingKind; 4] = {
    use crate::game::ledger::BuildingKind::*;
    [Carpenter, Quarry, Armory, Mine]
};
