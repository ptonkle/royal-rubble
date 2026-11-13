//! The ground: pad, gap with water, plot with hills, a cliff and high ground.

use super::rng::Rng;
use super::tune::*;
use super::{ColRange, Feature, GapKind, Plot, Water, CELL};

/// Linear blend from `a` to `b`, step `i` of `n`.
fn lerp(a: i32, b: i32, i: i32, n: i32) -> i32 {
    if n <= 0 {
        return b;
    }
    (a as i64 + (b as i64 - a as i64) * i as i64 / n as i64) as i32
}

/// Box blur of half-width `r`, edges clamped.
fn smooth(h: &mut [i32], r: usize) {
    let src = h.to_vec();
    let n = src.len();
    for (i, out) in h.iter_mut().enumerate() {
        let lo = i.saturating_sub(r);
        let hi = (i + r).min(n - 1);
        let sum: i64 = src[lo..=hi].iter().map(|&v| v as i64).sum();
        *out = (sum / (hi - lo + 1) as i64) as i32;
    }
}

/// Limits the rise per column to `max`, left to right.
fn clamp_slope(h: &mut [i32], max: i32) {
    for i in 1..h.len() {
        h[i] = h[i].clamp(h[i - 1] - max, h[i - 1] + max);
    }
}

/// The plot plus the margin behind it, starting at `front`. Returns the
/// heights, and the hill and cliff features with columns relative to the plot.
/// Plot heights, the hill (columns, peak) and the cliff (column, rise).
type PlotGround = (Vec<i32>, Option<(ColRange, usize)>, Option<(usize, i32)>);

fn plot_ground(rng: &mut Rng, front: i32) -> PlotGround {
    let n = PLOT_MAX_W + MARGIN_W;
    // rolling ground: control points every 12 columns, blended and blurred
    let step = 12usize;
    let points: Vec<i32> = (0..=n / step + 1)
        .map(|k| {
            if k == 0 {
                front
            } else {
                front + rng.range(-3 * CELL, 3 * CELL)
            }
        })
        .collect();
    let mut h: Vec<i32> = (0..n)
        .map(|i| {
            lerp(
                points[i / step],
                points[i / step + 1],
                (i % step) as i32,
                step as i32,
            )
        })
        .collect();

    // watchtower hill
    let mut hill = None;
    if rng.chance(HILL_CHANCE_OF_3, 3) {
        let hw = rng.range(HILL_HALF_W.0, HILL_HALF_W.1);
        let c = rng.range(
            hw + HILL_TOP_HALF_W + 4,
            PLOT_MAX_W as i32 - hw - HILL_TOP_HALF_W - 4,
        );
        let top = rng.range(HILL_H.0, HILL_H.1) * CELL;
        for (i, v) in h.iter_mut().enumerate() {
            // a flat top a few columns wide, for the watchtower
            let d = ((i as i32 - c).abs() - HILL_TOP_HALF_W).max(0);
            if d < hw {
                *v += lerp(top, 0, d, hw);
            }
        }
        hill = Some((c, hw));
    }
    smooth(&mut h, 3);
    smooth(&mut h, 2);
    // the front column stays where the gap's bank meets it
    h[0] = front;
    clamp_slope(&mut h, MAX_STEP);

    // a cliff up to high ground, kept clear of the hill and the plot's ends
    let mut cliff = None;
    if rng.chance(CLIFF_CHANCE_OF_3, 3) {
        let lo = PLOT_MAX_W as i32 * 30 / 100;
        let hi = PLOT_MAX_W as i32 * 75 / 100;
        let c = rng.range(lo, hi);
        let clear = match hill {
            Some((hc, hw)) => (c - hc).abs() > hw + HILL_TOP_HALF_W + 4,
            None => true,
        };
        if clear {
            let rise = rng.range(CLIFF_RISE.0, CLIFF_RISE.1) * CELL;
            for v in h.iter_mut().skip(c as usize) {
                *v += rise;
            }
            cliff = Some((c as usize, rise));
        }
    }

    let hill = hill.map(|(c, hw)| {
        let cols = ColRange {
            start: (c - hw - HILL_TOP_HALF_W) as usize,
            end: (c + hw + HILL_TOP_HALF_W) as usize,
        };
        let mut peak = cols.start;
        for i in cols.start..cols.end {
            if h[i] > h[peak] {
                peak = i;
            }
        }
        (cols, peak)
    });
    (h, hill, cliff)
}

/// Generates everything but the spots.
pub fn ground(seed: u64) -> Plot {
    let mut rng = Rng::new(seed);

    // launch pad: nearly flat
    let pad_h = BASE_H + rng.range(-2, 2) * CELL;
    let mut pad: Vec<i32> = (0..PAD_W)
        .map(|_| pad_h + rng.range(-CELL / 8, CELL / 8))
        .collect();
    smooth(&mut pad, 2);

    // the castle side starts a little lower or higher than the pad
    let front = pad_h + rng.range(-2, 4) * CELL;
    let (plot, hill, cliff) = plot_ground(&mut rng, front);

    // the gap between them
    let kind = match rng.pick(3) {
        0 => GapKind::Moat,
        1 => GapKind::River,
        _ => GapKind::Valley,
    };
    let (gw, bank_l, bank_r, depth, water_depth) = match kind {
        GapKind::Moat => {
            let gw = rng.range(10, 14);
            (gw, rng.range(1, 2), rng.range(1, 2), rng.range(5, 7), 3)
        }
        GapKind::River => {
            let gw = rng.range(16, 22);
            (gw, rng.range(4, 6), rng.range(4, 6), rng.range(3, 4), 2)
        }
        GapKind::Valley => {
            let gw = rng.range(24, 30);
            let bank = (gw - 6) / 2;
            (gw, bank, gw - 6 - bank, rng.range(6, 9), 1)
        }
    };
    let left = *pad.last().unwrap_or(&pad_h);
    let right = plot[0];
    // below the lowest ground on either side, so no plot column lies under water
    let lowest = plot.iter().copied().min().unwrap_or(right).min(left).min(right);
    let bottom = lowest - depth * CELL;
    let level = bottom + water_depth * CELL;
    let gap: Vec<i32> = (0..gw)
        .map(|i| {
            if i < bank_l {
                lerp(left, bottom, i + 1, bank_l)
            } else if i >= gw - bank_r {
                lerp(bottom, right, i - (gw - bank_r) + 1, bank_r + 1)
            } else {
                bottom
            }
        })
        .collect();

    let gap_start = PAD_W;
    let plot_start = gap_start + gw as usize;
    let mut heights = pad;
    heights.extend_from_slice(&gap);
    heights.extend_from_slice(&plot);

    let wet: Vec<usize> = (gap_start..plot_start).filter(|&c| heights[c] < level).collect();
    let water = Water {
        cols: ColRange {
            start: wet.first().copied().unwrap_or(gap_start),
            end: wet.last().map(|c| c + 1).unwrap_or(gap_start),
        },
        level,
    };

    let mut features = Vec::new();
    for c in 1..heights.len() {
        let rise = heights[c] - heights[c - 1];
        if rise.abs() >= CLIFF_MIN {
            features.push(Feature::Cliff { col: c, rise });
        }
    }
    let plot_range = ColRange {
        start: plot_start,
        end: plot_start + PLOT_MAX_W,
    };
    if let Some((c, _)) = cliff {
        features.push(Feature::HighGround {
            cols: ColRange {
                start: plot_start + c,
                end: plot_range.end,
            },
        });
    }
    if let Some((cols, peak)) = hill {
        features.push(Feature::Hill {
            cols: ColRange {
                start: plot_start + cols.start,
                end: plot_start + cols.end,
            },
            peak: plot_start + peak,
        });
    }

    Plot {
        seed,
        heights,
        pad: ColRange { start: 0, end: PAD_W },
        launch_col: PAD_W / 2,
        gap: ColRange {
            start: gap_start,
            end: plot_start,
        },
        gap_kind: kind,
        water,
        plot: plot_range,
        features,
        spots: Vec::new(),
    }
}
