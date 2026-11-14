use super::rng::Rng;
use super::tune::*;
use super::*;

/// Every problem with one plot, as text; empty when the plot is playable.
fn problems(p: &Plot) -> Vec<String> {
    let mut out = Vec::new();
    let s = p.seed;
    if p.water.cols.is_empty() {
        out.push(format!("seed {s}: no water"));
    }
    if p.water.cols.start < p.pad.end || p.water.cols.end > p.plot.start {
        out.push(format!("seed {s}: water outside the gap"));
    }
    if p.heights.iter().any(|&h| h <= 0) {
        out.push(format!("seed {s}: ground at or below zero"));
    }
    // the plot is buildable: gentle slopes except at its one cliff
    let cliff_cols: Vec<usize> = p
        .features
        .iter()
        .filter_map(|f| match f {
            Feature::Cliff { col, .. } => Some(*col),
            _ => None,
        })
        .collect();
    for c in p.plot.start + 1..p.plot.end {
        let rise = (p.heights[c] - p.heights[c - 1]).abs();
        if rise > PLOT_STEEP && !cliff_cols.contains(&c) {
            out.push(format!("seed {s}: column {c} too steep"));
        }
    }
    for l in 1..=MAX_LEVEL {
        let span = p.buildable(l);
        let gentle = (span.start + 1..span.end)
            .filter(|&c| (p.heights[c] - p.heights[c - 1]).abs() <= MAX_STEP)
            .count();
        if gentle * 100 < (span.len() - 1) * BUILDABLE_PCT {
            out.push(format!(
                "seed {s}: level {l} has {gentle} gentle columns of {}",
                span.len()
            ));
        }
    }
    for c in p.between.start..p.between.end {
        if p.heights[c] > p.arc_cap(c) {
            out.push(format!("seed {s}: column {c} too high to shoot over"));
        }
    }
    let dist = (p.plot.start - p.launch_col) as i32;
    if dist < DIST.0 || dist > DIST.1 || p.between.end != p.gap.start {
        out.push(format!("seed {s}: plot front {dist} columns from the trebuchet"));
    }
    if cliff_cols
        .iter()
        .filter(|&&c| c > p.plot.start && p.plot.contains(c))
        .count()
        > 1
    {
        out.push(format!("seed {s}: more than one cliff in the plot"));
    }
    for (i, sp) in p.spots.iter().enumerate() {
        let c = sp.cols;
        if c.start < p.plot.start || c.end > p.plot.end {
            out.push(format!("seed {s}: {:?} outside the plot", sp.kind));
            continue;
        }
        if p.water.cols.overlaps(&c) {
            out.push(format!("seed {s}: {:?} in water", sp.kind));
        }
        let h = &p.heights[c.start..c.end];
        let lo = h.iter().min().copied().unwrap_or(0);
        let hi = h.iter().max().copied().unwrap_or(0);
        let flat = if sp.kind == SpotKind::Ruins {
            RUIN_FLAT
        } else {
            SPOT_FLAT
        };
        if hi - lo > flat || lo < p.water.level {
            out.push(format!("seed {s}: {:?} not on flat dry ground", sp.kind));
        }
        for other in &p.spots[i + 1..] {
            if other.cols.overlaps(&c) {
                out.push(format!("seed {s}: {:?} overlaps {:?}", sp.kind, other.kind));
            }
        }
        for seg in &sp.segments {
            for n in [seg.a, seg.b] {
                let col = (n.x / COL_W) as usize;
                if !c.contains(col) || n.y < p.heights[col] {
                    out.push(format!("seed {s}: ruin node outside its spot or underground"));
                }
            }
        }
    }
    for &(kind, min, max, _) in SPOTS {
        let n = p.spots.iter().filter(|sp| sp.kind == kind).count();
        let hill = p.features.iter().any(|f| matches!(f, Feature::Hill { .. }));
        let min = if kind == SpotKind::Watchtower && !hill {
            0
        } else {
            min
        };
        if n < min || n > max || (kind == SpotKind::Watchtower && n > 0 && !hill) {
            out.push(format!("seed {s}: {n} of {kind:?}"));
        }
    }
    out
}

#[test]
fn terrain_rng_matches_splitmix64() {
    // the published splitmix64 stream from seed 0
    let mut r = Rng::new(0);
    assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
}

#[test]
fn terrain_same_seed_same_plot() {
    assert_eq!(Plot::generate(42), Plot::generate(42));
    assert_ne!(Plot::generate(42).heights, Plot::generate(43).heights);
}

#[test]
fn terrain_water_lies_between_pad_and_plot() {
    let p = Plot::generate(7);
    assert!(!p.water.cols.is_empty());
    assert!(p.water.cols.start >= p.pad.end && p.water.cols.end <= p.plot.start);
    let mid = (p.water.cols.start + p.water.cols.end) / 2;
    let x = mid as i32 * COL_W;
    assert!(p.in_water(x, p.water.level - 1));
    assert!(!p.in_water(p.launch_col as i32 * COL_W, p.water.level - 1));
}

#[test]
fn terrain_buildable_span_grows_with_level() {
    let p = Plot::generate(1);
    assert_eq!(p.buildable(1).len(), 40);
    assert_eq!(p.buildable(9).len(), 96);
    assert_eq!(p.buildable(9), p.plot);
    for l in 1..MAX_LEVEL {
        let (a, b) = (p.buildable(l), p.buildable(l + 1));
        assert_eq!(a.start, b.start);
        assert_eq!(b.len(), a.len() + 7);
    }
    assert!(Plot::build_height(9) > Plot::build_height(1));
}

#[test]
fn terrain_spots_sit_on_flat_dry_ground_apart() {
    let p = Plot::generate(3);
    assert!(p.spots.len() >= 4);
    let bad = problems(&p);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn terrain_a_steep_spot_is_refused() {
    let mut p = Plot::generate(3);
    p.spots.clear();
    let c = p.plot.start + 10;
    p.heights[c] += 2 * CELL;
    let cols = ColRange {
        start: c - 2,
        end: c + 2,
    };
    assert!(!spots::fits(&p, cols));
    // the same span with the bump flattened back fits
    p.heights[c] = p.heights[c - 1];
    p.heights[c + 1] = p.heights[c - 1];
    p.heights[c - 2] = p.heights[c - 1];
    assert!(spots::fits(&p, cols));
}

#[test]
fn terrain_hundred_seeds_all_playable() {
    let bad: Vec<String> = (0..100u64).flat_map(|s| problems(&Plot::generate(s))).collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn terrain_first_spots_open_at_level_1() {
    for seed in 0..100u64 {
        let p = Plot::generate(seed);
        let early = p
            .spots
            .iter()
            .filter(|s| s.kind != SpotKind::Watchtower && s.level == 1)
            .count();
        assert!(early >= SPOTS_AT_LEVEL_1, "seed {seed}: {early}");
    }
}

#[test]
fn terrain_open_country_stays_under_the_arc_cap() {
    let p = Plot::generate(5);
    assert!(!p.between.is_empty());
    let (l, f) = (p.launch_col, p.plot.start);
    // the cap is the chord at both ends and rises above it in between
    assert_eq!(p.arc_cap(l), p.heights[l]);
    assert_eq!(p.arc_cap(f), p.heights[f]);
    let mid = (l + f) / 2;
    let chord = (p.heights[l] + p.heights[f]) / 2;
    // the plot is about half as far since R1.3700, so the arc is half as tall
    assert!(p.arc_cap(mid) > chord + 4 * CELL);
    assert!(p.heights[p.between.start..p.between.end]
        .iter()
        .zip(p.between.start..)
        .all(|(&h, c)| h <= p.arc_cap(c)));
}

#[test]
fn terrain_relief_is_strong_and_varies_by_seed() {
    let mut above = 0;
    let mut below = 0;
    for seed in 0..100u64 {
        let p = Plot::generate(seed);
        let hi = p.heights.iter().copied().max().unwrap_or(0);
        let lo = p.heights.iter().copied().min().unwrap_or(0);
        // R1.3700: the hills shrank with the distance ("make the distance less")
        assert!(hi - lo >= 8 * CELL, "seed {seed}: relief {}", (hi - lo) / CELL);
        let pad = p.heights[p.launch_col];
        let front = p.heights[p.plot.start];
        if pad > front {
            above += 1;
        } else {
            below += 1;
        }
    }
    assert!(above >= 25 && below >= 25, "{above} above, {below} below");
}

#[test]
fn terrain_plot_front_is_far_from_the_trebuchet() {
    let d: Vec<usize> = (0..100u64)
        .map(|s| {
            let p = Plot::generate(s);
            p.plot.start - p.launch_col
        })
        .collect();
    let (lo, hi) = (d.iter().min().copied(), d.iter().max().copied());
    assert!(lo >= Some(DIST.0 as usize) && hi <= Some(DIST.1 as usize));
    assert!(hi.unwrap_or(0) - lo.unwrap_or(0) >= 15, "{lo:?}..{hi:?}");
}

#[test]
fn terrain_trees_stand_apart_in_open_country() {
    let mut any = 0;
    for seed in 1..60u64 {
        let p = Plot::generate(seed);
        let t = p.trees();
        assert_eq!(
            t,
            Plot::generate(seed).trees(),
            "seed {seed}: same seed, same trees"
        );
        any += t.len();
        for w in t.windows(2) {
            assert!(w[1].0 >= w[0].0 + 10, "seed {seed}: trees {:?} too close", w);
        }
        for &(c, look) in &t {
            assert!(
                c > p.between.start && c + 4 < p.between.end,
                "seed {seed}: tree at {c} outside open country"
            );
            assert!(!p.water.cols.contains(c), "seed {seed}: tree in water");
            assert!(look < 3);
        }
    }
    assert!(any > 59, "about one tree a plot at least, got {any}");
}

#[test]
fn terrain_ruins_stand_only_in_the_plot_front_band() {
    // R1.5000: every ruin node lies within RUIN_FRONT_W columns of the
    // plot's front edge, and every seed has a ruin there
    for seed in 0..200u64 {
        let p = Plot::generate(seed);
        let lo = p.plot.start as i32 * COL_W;
        let hi = (p.plot.start + RUIN_FRONT_W) as i32 * COL_W;
        let ruins: Vec<&Spot> = p.spots.iter().filter(|s| s.kind == SpotKind::Ruins).collect();
        assert!(!ruins.is_empty(), "seed {seed}: no ruin");
        for sp in ruins {
            for seg in &sp.segments {
                for n in [seg.a, seg.b] {
                    assert!(
                        n.x >= lo && n.x <= hi,
                        "seed {seed}: ruin node at {} outside {lo}..{hi}",
                        n.x
                    );
                }
            }
        }
    }
}
