//! One grid for terrain, physics and rules (R1.700).
//!
//! - A **cell** is one terrain column wide: `terrain::CELL` (1024) terrain
//!   units, exactly one physics unit (`Fx::ONE`), about one floor height.
//! - A **grid step** is a quarter cell, the physics snap grid (`TUNE.grid`).
//!   Every point a player gives (a segment end, a building, a royal) is a
//!   [`GridPt`] in grid steps, absolute: x from the terrain's left edge, y up
//!   from height 0.
//! - Rules count in whole cells: a royal `Spot` is cells from the buildable
//!   span's first column and cells above the ground under it; one segment
//!   length (wood's longest) is 2 cells (`game::tune::GRID_PER_SEGMENT`).
//! - The plot's size comes from the terrain only (`Plot::buildable`,
//!   `Plot::build_height`).

use crate::physics::{Fx, Terrain, Vec2, TUNE};
use crate::terrain::{self, CELL};

/// Physics raw units per terrain unit: 65536 / 1024.
pub const FX_PER_TERRAIN: i64 = (1 << 16) / CELL as i64;
/// Grid steps per cell.
pub const STEPS_PER_CELL: i32 = 4;

/// A point on the build grid, in grid steps (quarter cells), absolute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GridPt {
    pub x: i32,
    pub y: i32,
}

impl GridPt {
    pub const fn new(x: i32, y: i32) -> GridPt {
        GridPt { x, y }
    }

    /// The physics position of this grid point.
    pub fn to_fx(self) -> Vec2 {
        Vec2::new(Fx(self.x as i64 * TUNE.grid.0), Fx(self.y as i64 * TUNE.grid.0))
    }

    /// The terrain column this point lies in.
    pub fn col(self) -> usize {
        self.x.max(0).div_euclid(STEPS_PER_CELL) as usize
    }
}

/// A terrain height or x (terrain units) as a physics value.
pub fn terrain_to_fx(v: i32) -> Fx {
    Fx(v as i64 * FX_PER_TERRAIN)
}

/// A physics value in terrain units, rounded down.
pub fn fx_to_terrain(f: Fx) -> i32 {
    f.0.div_euclid(FX_PER_TERRAIN) as i32
}

/// The physics ground for a plot. Physics samples ground at `x = i`, so
/// column `i`'s height sits at its left edge and the ground slopes across
/// a column to the next one's height (a cliff is a one-cell slope).
pub fn physics_terrain(plot: &terrain::Plot) -> Terrain {
    Terrain {
        col_width: Fx::ONE,
        heights: plot.heights.iter().map(|h| terrain_to_fx(*h)).collect(),
    }
}

/// Where a trebuchet stands: on the ground at the left edge of the launch column.
pub fn trebuchet_base(plot: &terrain::Plot) -> Vec2 {
    let col = plot.launch_col;
    Vec2::new(Fx::from_int(col as i64), terrain_to_fx(plot.heights[col]))
}

/// True when a physics point is in the plot's water.
pub fn in_water(plot: &terrain::Plot, p: Vec2) -> bool {
    plot.in_water(fx_to_terrain(p.x), fx_to_terrain(p.y))
}
