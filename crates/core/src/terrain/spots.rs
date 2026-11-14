//! Spots to defend, placed on flat dry ground inside the plot, and the old
//! ruins' free stone segments.

use super::rng::Rng;
use super::tune::*;
use super::{ColRange, Feature, Plot, CELL, COL_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpotKind {
    StoneDeposit,
    OreDeposit,
    GoldVein,
    HolyWell,
    Forest,
    Ruins,
    Watchtower,
    ArmoryRuin,
}

/// The ruins' own material list; the physics maps it onto its segment types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuinMaterial {
    Stone,
}

/// A point in fixed-point units: `x` from the map's left edge, `y` up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub x: i32,
    pub y: i32,
}

/// A pre-built segment between two nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub a: Node,
    pub b: Node,
    pub material: RuinMaterial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spot {
    pub kind: SpotKind,
    pub cols: ColRange,
    /// The lowest level whose buildable plot holds the whole spot.
    pub level: u8,
    /// Pre-built segments (old ruins only).
    pub segments: Vec<Segment>,
}

/// Ruin shapes in cells: (x from the spot's left edge, height); height 0 is
/// the ground at that column's left edge (where the physics ground is that
/// column's height, so the node is anchored), any other height is above the
/// spot's highest ground. Every shape is braced into triangles between two
/// or more ground nodes, so it stands under its own weight (R1.1450).
type RuinShape = &'static [((i32, i32), (i32, i32))];
const RUINS: &[RuinShape] = &[
    // broken wall: two braced bays, the right one a floor lower
    &[
        ((1, 0), (1, 1)),
        ((1, 1), (1, 2)),
        ((2, 0), (2, 1)),
        ((2, 1), (2, 2)),
        ((3, 0), (3, 1)),
        ((1, 1), (2, 1)),
        ((1, 2), (2, 2)),
        ((2, 1), (3, 1)),
        ((1, 0), (2, 1)),
        ((2, 1), (1, 2)),
        ((3, 0), (2, 1)),
    ],
    // arch: two braced piers, a lintel and a pointed top
    &[
        ((1, 0), (1, 1)),
        ((1, 1), (1, 2)),
        ((2, 0), (2, 1)),
        ((2, 1), (2, 2)),
        ((1, 1), (2, 1)),
        ((1, 2), (2, 2)),
        ((1, 0), (2, 1)),
        ((2, 1), (1, 2)),
        ((4, 0), (4, 1)),
        ((4, 1), (4, 2)),
        ((5, 0), (5, 1)),
        ((5, 1), (5, 2)),
        ((4, 1), (5, 1)),
        ((4, 2), (5, 2)),
        ((5, 0), (4, 1)),
        ((4, 1), (5, 2)),
        ((2, 2), (4, 2)),
        ((2, 2), (3, 3)),
        ((3, 3), (4, 2)),
    ],
    // tower stub: two braced bays, one post broken off a floor short
    &[
        ((2, 0), (2, 1)),
        ((2, 1), (2, 2)),
        ((2, 2), (2, 3)),
        ((3, 0), (3, 1)),
        ((3, 1), (3, 2)),
        ((3, 2), (3, 3)),
        ((4, 0), (4, 1)),
        ((4, 1), (4, 2)),
        ((2, 1), (3, 1)),
        ((2, 2), (3, 2)),
        ((2, 3), (3, 3)),
        ((3, 1), (4, 1)),
        ((3, 2), (4, 2)),
        ((2, 0), (3, 1)),
        ((3, 1), (2, 2)),
        ((2, 2), (3, 3)),
        ((4, 0), (3, 1)),
        ((3, 1), (4, 2)),
    ],
    // rubble: low braced stubs
    &[
        ((1, 0), (1, 1)),
        ((1, 1), (2, 0)),
        ((3, 0), (3, 1)),
        ((4, 0), (4, 1)),
        ((3, 1), (4, 1)),
        ((3, 0), (4, 1)),
    ],
];

/// True when `cols` is flat, dry, inside the plot and clear of other spots
/// (with a free column between).
pub(super) fn fits(plot: &Plot, cols: ColRange) -> bool {
    fits_within(plot, cols, SPOT_FLAT)
}

/// [`fits`] with the ground allowed to rise `flat` across `cols`.
fn fits_within(plot: &Plot, cols: ColRange, flat: i32) -> bool {
    if cols.start < plot.plot.start || cols.end > plot.plot.end || plot.water.cols.overlaps(&cols) {
        return false;
    }
    let h = &plot.heights[cols.start..cols.end];
    let (lo, hi) = (h.iter().min(), h.iter().max());
    if let (Some(lo), Some(hi)) = (lo, hi) {
        if hi - lo > flat {
            return false;
        }
    }
    let padded = ColRange {
        start: cols.start.saturating_sub(1),
        end: cols.end + 1,
    };
    !plot.spots.iter().any(|s| s.cols.overlaps(&padded))
}

fn ruin_segments(plot: &Plot, cols: ColRange, shape: usize) -> Vec<Segment> {
    let top = plot.heights[cols.start..cols.end]
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let node = |(x, y): (i32, i32)| {
        let col = cols.start + x as usize;
        Node {
            x: col as i32 * COL_W,
            y: if y == 0 { plot.heights[col] } else { top + y * CELL },
        }
    };
    RUINS[shape]
        .iter()
        .map(|&(a, b)| Segment {
            a: node(a),
            b: node(b),
            material: RuinMaterial::Stone,
        })
        .collect()
}

fn push(plot: &mut Plot, rng: &mut Rng, kind: SpotKind, cols: ColRange) {
    let segments = if kind == SpotKind::Ruins {
        let total: u64 = RUIN_SHAPE_WEIGHTS.iter().sum();
        let mut roll = rng.next_u64() % total;
        let mut shape = 0;
        while roll >= RUIN_SHAPE_WEIGHTS[shape] {
            roll -= RUIN_SHAPE_WEIGHTS[shape];
            shape += 1;
        }
        ruin_segments(plot, cols, shape)
    } else {
        Vec::new()
    };
    let level = plot.level_for(cols);
    plot.spots.push(Spot {
        kind,
        cols,
        level,
        segments,
    });
}

pub fn place(plot: &mut Plot, seed: u64) {
    // its own stream, so the ground and the spots can change separately
    let mut rng = Rng::new(seed ^ 0x5350_4F54_5350_4F54);
    let peak = plot.features.iter().find_map(|f| match f {
        Feature::Hill { peak, .. } => Some(*peak),
        _ => None,
    });
    // the old ruins first, so they hold the plot's front band (R1.5000)
    for &(kind, min, max, w) in SPOTS.iter().filter(|s| s.0 == SpotKind::Ruins) {
        let count = rng.range(min as i32, max as i32) as usize;
        let band = plot.plot.start..=(plot.plot.start + RUIN_FRONT_W).saturating_sub(w);
        for _ in 0..count {
            let starts: Vec<usize> = band
                .clone()
                .filter(|&s| fits_within(plot, ColRange { start: s, end: s + w }, RUIN_FLAT))
                .collect();
            if starts.is_empty() {
                break;
            }
            let s = starts[rng.pick(starts.len())];
            push(plot, &mut rng, kind, ColRange { start: s, end: s + w });
        }
    }
    let mut placed_free = 0usize;
    for &(kind, min, max, w) in SPOTS.iter().filter(|s| s.0 != SpotKind::Ruins) {
        let count = rng.range(min as i32, max as i32) as usize;
        for _ in 0..count {
            if kind == SpotKind::Watchtower {
                // on the hilltop, or nowhere
                let Some(peak) = peak else { break };
                let at = [0i32, -1, 1, -2, 2, -3, 3].iter().find_map(|&d| {
                    let start = peak as i32 + d - w as i32 / 2;
                    let cols = ColRange {
                        start: start.max(0) as usize,
                        end: start.max(0) as usize + w,
                    };
                    fits(plot, cols).then_some(cols)
                });
                if let Some(cols) = at {
                    push(plot, &mut rng, kind, cols);
                }
                continue;
            }
            let near = placed_free < SPOTS_AT_LEVEL_1;
            let mut span = plot.plot;
            if near {
                span = plot.buildable(1);
            }
            let starts: Vec<usize> = (span.start..=span.end.saturating_sub(w))
                .filter(|&s| fits(plot, ColRange { start: s, end: s + w }))
                .collect();
            let starts = if starts.is_empty() && near {
                (plot.plot.start..=plot.plot.end - w)
                    .filter(|&s| fits(plot, ColRange { start: s, end: s + w }))
                    .collect()
            } else {
                starts
            };
            if starts.is_empty() {
                continue;
            }
            let s = starts[rng.pick(starts.len())];
            push(plot, &mut rng, kind, ColRange { start: s, end: s + w });
            placed_free += 1;
        }
    }
    plot.spots.sort_by_key(|s| s.cols.start);
}
