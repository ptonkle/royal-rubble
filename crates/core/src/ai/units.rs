//! The one place the AI converts between terrain, rules and physics units.
//! Settled by play/ (R1.700): one terrain column is one cell is one physics
//! unit; points a player gives are `play::units::GridPt` in quarter cells;
//! royals stand `game::royals::spacing_cells()` (8) cells apart. These
//! helpers lean on `play::units` so the AI never disagrees with the match.

use crate::physics::{Fx, Terrain, Vec2, TUNE};
use crate::play::units::{self as pu, GridPt};
use crate::terrain::Plot;

/// Physics units per terrain column.
pub fn col_width() -> Fx {
    Fx::ONE
}

/// Terrain fixed-point (1024 a cell) to a physics number.
pub fn cells_to_fx(v: i32) -> Fx {
    pu::terrain_to_fx(v)
}

/// The physics x of a terrain column's left edge.
pub fn col_x(col: usize) -> Fx {
    col_width() * (col as i64)
}

/// The terrain column holding physics x.
pub fn x_col(x: Fx) -> usize {
    (x / col_width()).floor_int().max(0) as usize
}

/// The match terrain as the physics heightfield.
pub fn physics_terrain(plot: &Plot) -> Terrain {
    pu::physics_terrain(plot)
}

/// Royal spacing as a physics distance.
pub fn royal_spacing() -> Fx {
    Fx::from_int(crate::game::royals::spacing_cells() as i64)
}

/// A physics point already on the snap grid, as a grid point.
pub fn grid_pt(p: Vec2) -> GridPt {
    let g = TUNE.grid.raw();
    GridPt::new(p.x.raw().div_euclid(g) as i32, p.y.raw().div_euclid(g) as i32)
}
