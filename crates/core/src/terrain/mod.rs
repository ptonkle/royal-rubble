//! Seeded terrain: one plot layout, in profile, that every player gets a copy
//! of. From the left: the launch pad (flat-ish, where the trebuchet stands),
//! open country with hills and dips the shot must clear, the gap (a moat,
//! river or valley with water in it), then the castle plot, which is
//! generated at its widest (level 9) and opened up by level from its front
//! edge at the gap.
//!
//! Units: the ground is a list of columns, each [`COL_W`] wide; heights are in
//! the same fixed-point units, [`CELL`] to one grid cell. No floats.

mod gen;
pub mod rng;
mod spots;
#[cfg(test)]
mod tests;

pub use spots::{Node, RuinMaterial, Segment, Spot, SpotKind};

/// Fixed-point units in one grid cell.
pub const CELL: i32 = 1024;
/// Width of one terrain column, in fixed-point units.
pub const COL_W: i32 = CELL;
/// Highest level; the plot is generated at this level's width.
pub const MAX_LEVEL: u8 = 9;

/// The tune table. Widths are in columns, heights in cells unless named.
pub mod tune {
    use super::CELL;
    /// Launch pad width.
    pub const PAD_W: usize = 28;
    /// Ground beyond the widest plot, so the plot's back edge is not a wall.
    pub const MARGIN_W: usize = 6;
    /// Buildable width at level 1, and what each level adds.
    pub const PLOT_MIN_W: usize = 40;
    pub const PLOT_STEP_W: usize = 7;
    /// Widest plot (level 9).
    pub const PLOT_MAX_W: usize = PLOT_MIN_W + 8 * PLOT_STEP_W;
    /// Build height above the plot's front ground at level 1, and per level.
    pub const BUILD_H_MIN: i32 = 12 * CELL;
    pub const BUILD_H_STEP: i32 = 2 * CELL;
    /// Ground height the pad is centered on.
    pub const BASE_H: i32 = 40 * CELL;
    /// Launch column to the plot's front edge, in columns (about 130, give or take 14%).
    pub const DIST: (i32, i32) = (112, 148);
    /// The plot's front ground sits this many cells above or below the pad (either way, by seed).
    pub const FRONT_RISE: (i32, i32) = (6, 15);
    /// Ground between the pad and the gap: a bump's height in cells (negative is a dip),
    /// its half-width in columns, and the steepest rise per column.
    pub const BETWEEN_HILL_H: (i32, i32) = (8, 20);
    pub const BETWEEN_HILL_HALF_W: (i32, i32) = (14, 24);
    pub const BETWEEN_DIP_H: (i32, i32) = (4, 10);
    pub const BETWEEN_STEP: i32 = CELL * 5 / 4;
    /// The between-ground never rises above the line from the launch column to the plot
    /// front by more than this percent of a 45-degree arc's height over it.
    pub const ARC_CAP_PCT: i64 = 55;
    /// Steepest buildable slope: rise per column inside the plot, away from a cliff.
    pub const MAX_STEP: i32 = CELL / 2;
    /// Steepest any plot column may be (a terrace ramp, a mound's flank), away from a cliff.
    pub const PLOT_STEEP: i32 = CELL;
    /// At least this percent of each level's columns stay within [`MAX_STEP`].
    pub const BUILDABLE_PCT: usize = 60;
    /// The plot's overall tilt from front to back, in cells.
    pub const PLOT_TILT: (i32, i32) = (-6, 8);
    /// A spot's ground may rise at most this much across its width.
    pub const SPOT_FLAT: i32 = CELL;
    /// A step at least this tall in one column is a cliff.
    pub const CLIFF_MIN: i32 = 2 * CELL;
    /// Plot cliff (a step up to high ground): chance in 3, rise range in cells.
    pub const CLIFF_CHANCE_OF_3: u64 = 2;
    pub const CLIFF_RISE: (i32, i32) = (4, 9);
    /// Terrace: a steep ramp up (or down) to a second shelf: chance in 3, rise in cells.
    pub const TERRACE_CHANCE_OF_3: u64 = 1;
    pub const TERRACE_RISE: (i32, i32) = (3, 5);
    /// Watchtower hill: chance in 3, half-width in columns, peak height in cells.
    pub const HILL_CHANCE_OF_3: u64 = 2;
    pub const HILL_HALF_W: (i32, i32) = (12, 16);
    pub const HILL_H: (i32, i32) = (5, 8);
    /// The hill's center is at least this many columns into the plot.
    pub const HILL_MIN_COL: i32 = 30;
    /// Half-width of the hill's flat top, in columns.
    pub const HILL_TOP_HALF_W: i32 = 2;
    /// Spot counts per match and widths in columns: (kind, min, max, width).
    /// The watchtower's count holds only when the map has a hill.
    pub const SPOTS: &[(super::SpotKind, usize, usize, usize)] = {
        use super::SpotKind::*;
        &[
            (Watchtower, 1, 1, 5),
            (StoneDeposit, 1, 1, 5),
            (GoldVein, 1, 1, 4),
            (Forest, 1, 1, 8),
            (Ruins, 1, 2, 6),
            (OreDeposit, 0, 1, 5),
            (HolyWell, 0, 1, 3),
            (ArmoryRuin, 0, 1, 5),
        ]
    };
    /// The first this many spots placed after the watchtower go inside the level-1 plot.
    pub const SPOTS_AT_LEVEL_1: usize = 2;
}

/// Columns `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColRange {
    pub start: usize,
    pub end: usize,
}

impl ColRange {
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
    pub fn contains(&self, col: usize) -> bool {
        col >= self.start && col < self.end
    }
    pub fn overlaps(&self, other: &ColRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// What lies between the pad and the plot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapKind {
    /// Narrow and deep with steep banks.
    Moat,
    /// Wider, gentle banks.
    River,
    /// Wide and deep, long slopes down to a stream.
    Valley,
}

/// Water: columns whose ground lies below `level` are water. A shot that
/// lands in water is spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Water {
    pub cols: ColRange,
    pub level: i32,
}

/// Named terrain features, for the AI, the debug picture and later rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// The ground steps by `rise` (signed) between column `col - 1` and `col`.
    Cliff { col: usize, rise: i32 },
    /// Raised ground behind a plot cliff.
    HighGround { cols: ColRange },
    /// A hill with its highest column.
    Hill { cols: ColRange, peak: usize },
}

/// One match's terrain. Every player's plot is a copy of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plot {
    pub seed: u64,
    /// Ground height of each column.
    pub heights: Vec<i32>,
    pub pad: ColRange,
    /// Open country between the pad and the gap.
    pub between: ColRange,
    /// The column the trebuchet stands on.
    pub launch_col: usize,
    pub gap: ColRange,
    pub gap_kind: GapKind,
    pub water: Water,
    /// The widest (level 9) plot.
    pub plot: ColRange,
    pub features: Vec<Feature>,
    pub spots: Vec<Spot>,
}

impl Plot {
    pub fn generate(seed: u64) -> Plot {
        let mut plot = gen::ground(seed);
        spots::place(&mut plot, seed);
        plot
    }

    /// The buildable columns at `level` (clamped to 1..=9). The plot is
    /// anchored at its front edge by the gap and grows away from it.
    pub fn buildable(&self, level: u8) -> ColRange {
        let level = level.clamp(1, MAX_LEVEL) as usize;
        let w = tune::PLOT_MIN_W + (level - 1) * tune::PLOT_STEP_W;
        ColRange {
            start: self.plot.start,
            end: self.plot.start + w,
        }
    }

    /// How high above the plot's front ground a player may build at `level`.
    pub fn build_height(level: u8) -> i32 {
        let level = level.clamp(1, MAX_LEVEL) as i32;
        tune::BUILD_H_MIN + (level - 1) * tune::BUILD_H_STEP
    }

    /// The lowest level whose buildable span holds all of `cols`.
    pub fn level_for(&self, cols: ColRange) -> u8 {
        (1..=MAX_LEVEL)
            .find(|&l| cols.end <= self.buildable(l).end)
            .unwrap_or(MAX_LEVEL)
    }

    /// Ground height under fixed-point `x`; the edge columns extend outward.
    pub fn ground_at(&self, x: i32) -> i32 {
        let col = (x.max(0) / COL_W) as usize;
        self.heights[col.min(self.heights.len() - 1)]
    }

    /// The highest the open country may stand at `col`: the line from the
    /// launch column's ground to the plot front's ground, plus
    /// [`tune::ARC_CAP_PCT`] percent of a 45-degree arc's height over that line.
    pub fn arc_cap(&self, col: usize) -> i32 {
        arc_cap(
            self.launch_col,
            self.heights[self.launch_col],
            self.plot.start,
            self.heights[self.plot.start],
            col,
        )
    }

    /// True when fixed-point point (`x`, `y`) is in the water.
    pub fn in_water(&self, x: i32, y: i32) -> bool {
        let col = (x.max(0) / COL_W) as usize;
        self.water.cols.contains(col) && y <= self.water.level
    }
}

/// [`Plot::arc_cap`] from its parts: launch column and ground, plot front column and ground.
pub(crate) fn arc_cap(l: usize, lh: i32, f: usize, fh: i32, col: usize) -> i32 {
    let d = (f - l) as i64;
    let x = (col as i64 - l as i64).clamp(0, d);
    let chord = lh as i64 + (fh as i64 - lh as i64) * x / d;
    // a 45-degree arc rises x * (d - x) / d columns above the chord
    let arc = x * (d - x) * COL_W as i64 / d;
    (chord + arc * tune::ARC_CAP_PCT / 100) as i32
}
