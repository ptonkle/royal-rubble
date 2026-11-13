//! The ground: pad, open country with hills and dips, gap with water, plot
//! with a tilt, a mound, a terrace, a cliff and high ground.

use super::rng::Rng;
use super::tune::*;
use super::{arc_cap, ColRange, Feature, GapKind, Plot, Water, CELL};

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

/// Limits the rise per column to `max`, left to right, starting from `from`.
fn clamp_slope_from(h: &mut [i32], from: i32, max: i32) {
    let mut prev = from;
    for v in h.iter_mut() {
        *v = (*v).clamp(prev - max, prev + max);
        prev = *v;
    }
}

/// Adds a rounded bump of height `top` (negative digs), centered on `c`,
/// `hw` columns to each side of a flat top `flat` columns to each side.
fn bump(h: &mut [i32], c: i32, hw: i32, flat: i32, top: i32) {
    for (i, v) in h.iter_mut().enumerate() {
        let d = ((i as i32 - c).abs() - flat).max(0);
        if d < hw {
            // smoothstep: flat at the top and the foot, steepest halfway
            let (d, w) = (d as i64, hw as i64);
            let k = w * w * w - 3 * d * d * w + 2 * d * d * d;
            *v += (top as i64 * k / (w * w * w)) as i32;
        }
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
    let tilt = rng.range(PLOT_TILT.0, PLOT_TILT.1) * CELL;
    let points: Vec<i32> = (0..=n / step + 1)
        .map(|k| {
            let lean = lerp(0, tilt, (k * step).min(n) as i32, n as i32);
            if k == 0 {
                front
            } else {
                front + lean + rng.range(-3 * CELL, 3 * CELL)
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

    // watchtower hill (a mound with a flat top a few columns wide)
    let mut hill = None;
    if rng.chance(HILL_CHANCE_OF_3, 3) {
        let hw = rng.range(HILL_HALF_W.0, HILL_HALF_W.1);
        let c = rng.range(
            (hw + HILL_TOP_HALF_W + 4).max(HILL_MIN_COL),
            PLOT_MAX_W as i32 - hw - HILL_TOP_HALF_W - 4,
        );
        let top = rng.range(HILL_H.0, HILL_H.1) * CELL;
        bump(&mut h, c, hw, HILL_TOP_HALF_W, top);
        hill = Some((c, hw));
    }
    let clear_of_hill = |c: i32, room: i32| match hill {
        Some((hc, hw)) => (c - hc).abs() > hw + HILL_TOP_HALF_W + room,
        None => true,
    };

    // a terrace: a steep ramp up or down to a second shelf
    let mut terrace = None;
    if rng.chance(TERRACE_CHANCE_OF_3, 3) {
        let c = rng.range(PLOT_MAX_W as i32 * 20 / 100, PLOT_MAX_W as i32 * 85 / 100);
        if clear_of_hill(c, 6) {
            let up = rng.chance(1, 2);
            let rise = rng.range(TERRACE_RISE.0, TERRACE_RISE.1) * CELL;
            let rise = if up { rise } else { -rise };
            let ramp = (rise.abs() / (PLOT_STEEP * 3 / 4)).max(1);
            for (i, v) in h.iter_mut().enumerate().skip(c as usize) {
                *v += lerp(0, rise, (i as i32 - c + 1).min(ramp), ramp);
            }
            terrace = Some(c);
        }
    }
    smooth(&mut h, 2);
    smooth(&mut h, 1);
    // the front column stays where the gap's bank meets it
    h[0] = front;
    clamp_slope_from(&mut h, front, PLOT_STEEP);

    // a cliff up to high ground, kept clear of the hill and the plot's ends
    let mut cliff = None;
    if rng.chance(CLIFF_CHANCE_OF_3, 3) {
        let lo = PLOT_MAX_W as i32 * 30 / 100;
        let hi = PLOT_MAX_W as i32 * 75 / 100;
        let c = rng.range(lo, hi);
        let clear = clear_of_hill(c, 4) && terrace.is_none_or(|t| (c - t).abs() > 10);
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

/// The open country from the pad's end to `gap_start`: the line from the
/// pad down or up to the plot front, with one big hill, two hills or a long
/// ridge on it, maybe a dip, kept under [`super::Plot::arc_cap`].
fn between_ground(
    rng: &mut Rng,
    pad: &[i32],
    launch_col: usize,
    front: i32,
    gap_start: usize,
    plot_start: usize,
) -> Vec<i32> {
    let pad_end = *pad.last().unwrap_or(&BASE_H);
    let n = gap_start - PAD_W;
    let ni = n as i32;
    let bank = front + rng.range(-3, 3) * CELL;
    let mut h: Vec<i32> = (0..ni).map(|i| lerp(pad_end, bank, i + 1, ni)).collect();
    let hill_h = |rng: &mut Rng| rng.range(BETWEEN_HILL_H.0, BETWEEN_HILL_H.1) * CELL;
    let hill_w = |rng: &mut Rng| rng.range(BETWEEN_HILL_HALF_W.0, BETWEEN_HILL_HALF_W.1);
    match rng.pick(3) {
        0 => {
            // one big hill
            let hw = hill_w(rng);
            let c = rng.range(ni * 30 / 100, ni * 70 / 100);
            let top = hill_h(rng);
            bump(&mut h, c, hw, 1, top);
        }
        1 => {
            // two hills, one on each half
            for half in 0..2 {
                let hw = hill_w(rng) * 3 / 4;
                let c = rng.range(ni * (10 + 45 * half) / 100, ni * (45 + 45 * half) / 100);
                let top = hill_h(rng) * 3 / 4;
                bump(&mut h, c, hw, 0, top);
            }
        }
        _ => {
            // a long ridge with a flat crest
            let hw = hill_w(rng) / 2;
            let flat = rng.range(ni * 10 / 100, ni * 20 / 100);
            let c = rng.range(ni * 35 / 100, ni * 65 / 100);
            let top = hill_h(rng) * 3 / 4;
            bump(&mut h, c, hw, flat, top);
        }
    }
    if rng.chance(2, 3) {
        let hw = rng.range(6, 12);
        let c = rng.range(hw, ni - hw);
        let dip = rng.range(BETWEEN_DIP_H.0, BETWEEN_DIP_H.1) * CELL;
        bump(&mut h, c, hw, 0, -dip);
    }
    smooth(&mut h, 3);
    smooth(&mut h, 2);
    clamp_slope_from(&mut h, pad_end, BETWEEN_STEP);
    // capped, then blurred so a capped crest stays round, then capped again
    let launch_h = pad[launch_col];
    let cap = |i: usize| arc_cap(launch_col, launch_h, plot_start, front, PAD_W + i);
    for _ in 0..2 {
        for (i, v) in h.iter_mut().enumerate() {
            *v = (*v).min(cap(i)).max(4 * CELL);
        }
        smooth(&mut h, 2);
    }
    for (i, v) in h.iter_mut().enumerate() {
        *v = (*v).min(cap(i)).max(4 * CELL);
    }
    h
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

    let launch_col = PAD_W / 2;
    // the castle side stands well above or below the pad
    let rise = rng.range(FRONT_RISE.0, FRONT_RISE.1) * CELL;
    let front = if rng.chance(1, 2) {
        pad_h + rise
    } else {
        pad_h - rise
    };
    let (plot, hill, cliff) = plot_ground(&mut rng, front);

    // the gap right in front of the plot
    let kind = match rng.pick(3) {
        0 => GapKind::Moat,
        1 => GapKind::River,
        _ => GapKind::Valley,
    };
    let (gw, bank_l, bank_r, depth, water_depth) = match kind {
        GapKind::Moat => {
            let gw = rng.range(10, 14);
            (gw, rng.range(1, 2), rng.range(1, 2), rng.range(6, 9), 3)
        }
        GapKind::River => {
            let gw = rng.range(16, 22);
            (gw, rng.range(4, 6), rng.range(4, 6), rng.range(4, 6), 2)
        }
        GapKind::Valley => {
            let gw = rng.range(24, 30);
            let bank = (gw - 6) / 2;
            (gw, bank, gw - 6 - bank, rng.range(8, 12), 1)
        }
    };

    // open country from the pad to the gap
    let dist = rng.range(DIST.0, DIST.1) as usize;
    let gap_start = launch_col + dist - gw as usize;
    let plot_start = gap_start + gw as usize;
    let between = between_ground(&mut rng, &pad, launch_col, front, gap_start, plot_start);

    let left = *between.last().unwrap_or(&pad_h);
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

    let mut heights = pad;
    heights.extend_from_slice(&between);
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
        between: ColRange {
            start: PAD_W,
            end: gap_start,
        },
        launch_col,
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
