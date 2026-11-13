//! Every balance number of the match rules, in one place. Change a value
//! here, not in the rule that reads it. Why each curve has its shape is in
//! SECOND-ORDER.md (R1.200 onward).

use super::ledger::{BuildingKind, Material};
use super::royals::RoyalKind;

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
    (7, 150),
    (10, 200),
    (13, 300),
    (15, 400),
    (17, 600),
    (19, 800),
    (21, 1200),
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

/// Grid cells in one segment length (R1.206: to agree with physics).
pub const GRID_PER_SEGMENT: i32 = 4;
/// Royals stand at least this many segment lengths apart.
pub const ROYAL_SPACING_SEGMENTS: i32 = 4;

// ---- Economy --------------------------------------------------------------

pub const START_GOLD: u32 = 0;
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

/// What each level gives. Plot sizes are grid cells.
pub struct LevelRow {
    pub level: u8,
    pub plot_width: i32,
    pub plot_height: i32,
    pub normal_shots: u32,
    /// Trebuchet power in percent of the level-1 throw.
    pub shot_power_pct: u32,
}

pub const LEVELS: [LevelRow; 9] = [
    LevelRow {
        level: 1,
        plot_width: 48,
        plot_height: 32,
        normal_shots: 3,
        shot_power_pct: 100,
    },
    LevelRow {
        level: 2,
        plot_width: 52,
        plot_height: 36,
        normal_shots: 3,
        shot_power_pct: 100,
    },
    LevelRow {
        level: 3,
        plot_width: 56,
        plot_height: 40,
        normal_shots: 4,
        shot_power_pct: 105,
    },
    LevelRow {
        level: 4,
        plot_width: 64,
        plot_height: 44,
        normal_shots: 4,
        shot_power_pct: 110,
    },
    LevelRow {
        level: 5,
        plot_width: 72,
        plot_height: 48,
        normal_shots: 4,
        shot_power_pct: 115,
    },
    LevelRow {
        level: 6,
        plot_width: 80,
        plot_height: 52,
        normal_shots: 5,
        shot_power_pct: 120,
    },
    LevelRow {
        level: 7,
        plot_width: 88,
        plot_height: 56,
        normal_shots: 5,
        shot_power_pct: 125,
    },
    LevelRow {
        level: 8,
        plot_width: 100,
        plot_height: 60,
        normal_shots: 5,
        shot_power_pct: 130,
    },
    LevelRow {
        level: 9,
        plot_width: 112,
        plot_height: 64,
        normal_shots: 6,
        shot_power_pct: 140,
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
pub const MAX_SHOTS_BOUGHT: u32 = 3;

// ---- Upgrades -------------------------------------------------------------

pub const CARDS_OFFERED: usize = 3;
pub const FREE_REROLLS: u32 = 1;
pub const REROLL_COST: u32 = 2;
/// Tier weights (silver, gold, prismatic) for the 1st, 2nd, ... upgrade
/// round; the last row repeats.
pub const TIER_WEIGHTS: &[(u32, u32, u32)] = &[
    (70, 25, 5),
    (55, 35, 10),
    (40, 45, 15),
    (30, 45, 25),
    (20, 45, 35),
];
