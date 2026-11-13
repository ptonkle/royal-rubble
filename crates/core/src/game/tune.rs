//! Every balance number of the match rules, in one place. Change a value
//! here, not in the rule that reads it. Why each curve has its shape is in
//! SECOND-ORDER.md (R1.200 onward).

use super::ledger::{BuildingKind, Material};
use super::royals::RoyalKind;
use crate::terrain::SpotKind;

// ---- Clock ----------------------------------------------------------------

/// Rule ticks per second. Phase timers count ticks, never wall time.
pub const TICKS_PER_SECOND: u32 = 60;
pub const UPGRADE_SECONDS: u32 = 20;
pub const BUILD_SECONDS: u32 = 60;
pub const SIEGE_SECONDS: u32 = 60;

// ---- Lobby ----------------------------------------------------------------

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 8;
pub const START_HEALTH: i32 = 100;
/// Wind strength runs from -WIND_MAX to +WIND_MAX (physics decides the unit).
pub const WIND_MAX: i32 = 5;

// ---- Rounds ---------------------------------------------------------------

/// An Upgrade phase opens rounds FIRST_UPGRADE_ROUND, +UPGRADE_EVERY, ...
pub const FIRST_UPGRADE_ROUND: u32 = 2;
pub const UPGRADE_EVERY: u32 = 2;

/// Opponents a player may not meet again, by alive count (index). TFT keeps
/// you off your last few opponents; 0 for two players, who can only meet
/// each other (R1.202).
pub const REMATCH_WINDOW: [usize; 9] = [0, 0, 0, 1, 1, 2, 2, 3, 3];

/// Royal damage multiplier in percent, as (first round, percent). The last
/// row whose round has come applies. Shape and reasons: R1.201.
pub const DAMAGE_CURVE: &[(u32, u32)] = &[
    (1, 100),
    (9, 150),
    (12, 200),
    (15, 250),
    (17, 350),
    (19, 500),
    (21, 800),
    (23, 1200),
];

// ---- Royals ---------------------------------------------------------------

/// Base damage of each royal before round scaling.
pub const ROYAL_DAMAGE: &[(RoyalKind, u32)] = &[
    (RoyalKind::Squire, 1),
    (RoyalKind::Knight, 2),
    (RoyalKind::Princess, 3),
    (RoyalKind::Prince, 4),
    (RoyalKind::Queen, 6),
    (RoyalKind::King, 10),
];

/// Which royal arrives on which round (BRIEF.md, Royals).
pub const ROYAL_SCHEDULE: &[(u32, RoyalKind)] = &[
    (1, RoyalKind::Squire),
    (2, RoyalKind::Squire),
    (3, RoyalKind::Knight),
    (4, RoyalKind::Squire),
    (5, RoyalKind::Princess),
    (7, RoyalKind::Squire),
    (8, RoyalKind::Prince),
    (9, RoyalKind::Knight),
    (11, RoyalKind::Queen),
    (12, RoyalKind::Princess),
    (13, RoyalKind::King),
    (15, RoyalKind::Prince),
];

/// Grid cells in one segment length: wood's longest, 2 physics units of
/// one cell each (R1.206, R1.700).
pub const GRID_PER_SEGMENT: i32 = 2;
/// Royals stand at least this many grid cells apart, so two never overlap;
/// the room rule keeps them from crowding one room (R1.3706: was 4 segment
/// lengths, which "encourages building a bunch of little huts").
pub const ROYAL_SPACING_CELLS: i32 = 2;

// ---- Economy --------------------------------------------------------------

pub const START_GOLD: u32 = 0;
/// The stockpile every player starts with, about a small keep's worth, so
/// building pays from round 1 (R1.3701: "start with a set amount of stone
/// and wood and zero gold to encourage early castle building").
pub const START_WOOD: u32 = 12;
pub const START_STONE: u32 = 6;
/// The first round with base income; round 1 builds from the stockpile.
pub const FIRST_INCOME_ROUND: u32 = 2;
pub const BASE_INCOME: u32 = 5;
pub const INTEREST_STEP: u32 = 10;
pub const INTEREST_CAP: u32 = 5;
pub const KILL_GOLD: u32 = 1;
pub const KING_KILL_GOLD: u32 = 3;

pub const XP_BUY_COST: u32 = 4;
pub const XP_BUY_AMOUNT: u32 = 4;
pub const FREE_XP_PER_ROUND: u32 = 2;
pub const MAX_LEVEL: u8 = 9;
/// XP from level i+1 to level i+2 (TFT's curve, R1.203).
pub const XP_TO_NEXT: [u32; 8] = [2, 2, 6, 10, 20, 36, 48, 72];

/// What each level gives. The plot's size comes from the terrain
/// (`terrain::Plot::buildable`, `build_height`; R1.606, R1.700).
pub struct LevelRow {
    pub level: u8,
    pub normal_shots: u32,
    /// Trebuchet power in percent of the level-1 throw.
    pub shot_power_pct: u32,
}

pub const LEVELS: [LevelRow; 9] = [
    LevelRow {
        level: 1,
        normal_shots: 3,
        shot_power_pct: 100,
    },
    LevelRow {
        level: 2,
        normal_shots: 3,
        shot_power_pct: 100,
    },
    LevelRow {
        level: 3,
        normal_shots: 4,
        shot_power_pct: 103,
    },
    LevelRow {
        level: 4,
        normal_shots: 4,
        shot_power_pct: 106,
    },
    LevelRow {
        level: 5,
        normal_shots: 4,
        shot_power_pct: 109,
    },
    LevelRow {
        level: 6,
        normal_shots: 5,
        shot_power_pct: 112,
    },
    LevelRow {
        level: 7,
        normal_shots: 5,
        shot_power_pct: 115,
    },
    LevelRow {
        level: 8,
        normal_shots: 5,
        shot_power_pct: 118,
    },
    LevelRow {
        level: 9,
        normal_shots: 6,
        shot_power_pct: 122,
    },
];

// ---- Building -------------------------------------------------------------

/// (material, gold per segment, level that unlocks it).
pub const MATERIALS: &[(Material, u32, u8)] = &[
    (Material::Wood, 1, 1),
    (Material::Stone, 3, 4),
    (Material::Metal, 5, 7),
];
/// Rope or cable, per segment, and the level that unlocks it (R1.204).
pub const ROPE_COST: u32 = 1;
pub const ROPE_LEVEL: u8 = 2;

/// What a producer gives per round it survives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yield {
    Stock(Material, u32),
    /// Extra normal shots next round.
    Shots(u32),
}

pub struct BuildingRow {
    pub kind: BuildingKind,
    pub cost: u32,
    pub level: u8,
    pub yields: Yield,
}

pub const BUILDINGS: &[BuildingRow] = &[
    BuildingRow {
        kind: BuildingKind::Carpenter,
        cost: 3,
        level: 2,
        yields: Yield::Stock(Material::Wood, 3),
    },
    BuildingRow {
        kind: BuildingKind::Quarry,
        cost: 6,
        level: 4,
        yields: Yield::Stock(Material::Stone, 1),
    },
    BuildingRow {
        kind: BuildingKind::Armory,
        cost: 6,
        level: 5,
        yields: Yield::Shots(1),
    },
    BuildingRow {
        kind: BuildingKind::Mine,
        cost: 10,
        level: 7,
        yields: Yield::Stock(Material::Metal, 1),
    },
];

/// Selling after the phase it was placed in pays value * NUM / DEN, rounded down.
pub const SELL_LATER_NUM: u32 = 1;
pub const SELL_LATER_DEN: u32 = 2;

/// Gold per extra normal shot bought in build, and how many a round.
pub const SHOT_COST: u32 = 2;
/// A discount never makes a bought shot cheaper than this.
pub const MIN_SHOT_COST: u32 = 1;
pub const MAX_SHOTS_BOUGHT: u32 = 3;

// ---- Map spots (R1.600) ----------------------------------------------------

/// A producer on its own deposit or forest yields this many times as much.
pub const SPOT_YIELD_MULT: u32 = 2;
/// Which spot boosts which producer.
pub const SPOT_BOOSTS: &[(SpotKind, BuildingKind)] = &[
    (SpotKind::StoneDeposit, BuildingKind::Quarry),
    (SpotKind::OreDeposit, BuildingKind::Mine),
    (SpotKind::Forest, BuildingKind::Carpenter),
];
/// Gold per round a building on the gold vein survives.
pub const GOLD_VEIN_GOLD: u32 = 2;
/// Health per round a building on the holy well survives (never above START_HEALTH).
pub const HOLY_WELL_HEAL: i32 = 3;
/// Extra normal shots per round a building on the armory ruin survives.
pub const ARMORY_RUIN_SHOTS: u32 = 1;

// ---- Upgrades -------------------------------------------------------------

pub const CARDS_OFFERED: usize = 3;
pub const FREE_REROLLS: u32 = 1;
pub const REROLL_COST: u32 = 2;
/// Air effects take the Q, W, E and R keys: four at most.
pub const MAX_AIR_SLOTS: usize = 4;
/// Tier weights (silver, gold, prismatic) for the 1st, 2nd, ... upgrade
/// round; the last row repeats.
pub const TIER_WEIGHTS: &[(u32, u32, u32)] = &[
    (70, 25, 5),
    (55, 35, 10),
    (40, 45, 15),
    (30, 45, 25),
    (20, 45, 35),
];
