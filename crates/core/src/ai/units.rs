//! The one place the AI converts between terrain, rules and physics units.
//! The units are still being reconciled (SECOND-ORDER R1.206, R1.606,
//! R1.900): this assumes one terrain column (`terrain::CELL` = 1024) is one
//! physics world unit, and one "segment length" for royal spacing is one
//! world unit (a floor height). Change these helpers, not the callers.

use crate::physics::{Fx, Terrain};
use crate::terrain::{Plot, CELL};

/// Physics units per terrain column.
pub fn col_width() -> Fx {
    Fx::ONE
}

/// Terrain fixed-point (1024 a cell) to a physics number.
pub fn cells_to_fx(v: i32) -> Fx {
    Fx(v as i64 * Fx::ONE.raw() / CELL as i64)
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
    Terrain {
        col_width: col_width(),
        heights: plot.heights.iter().map(|h| cells_to_fx(*h)).collect(),
    }
}

/// Royal spacing as a physics distance: `ROYAL_SPACING_SEGMENTS` segment
/// lengths of one unit each.
pub fn royal_spacing() -> Fx {
    Fx::from_int(crate::game::tune::ROYAL_SPACING_SEGMENTS as i64)
}

/// Physics trebuchet power (0..=8 adds eighths of speed) from the level's
/// shot power percent (100..140).
pub fn treb_power(shot_power_pct: u32) -> u8 {
    (shot_power_pct.saturating_sub(100) * 8 / 100).min(255) as u8
}
