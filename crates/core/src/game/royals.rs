//! The royals: who arrives when, what each is worth, where they may stand.

use super::dice::Dice;
use super::tune;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoyalKind {
    Squire,
    Knight,
    Princess,
    Prince,
    Queen,
    King,
}

impl RoyalKind {
    pub fn base_damage(self) -> u32 {
        tune::ROYAL_DAMAGE
            .iter()
            .find(|(k, _)| *k == self)
            .map(|(_, d)| *d)
            .unwrap_or(0)
    }
}

/// The royals that arrive at the start of `round`'s build.
pub fn arrivals(round: u32) -> Vec<RoyalKind> {
    tune::ROYAL_SCHEDULE
        .iter()
        .filter(|(r, _)| *r == round)
        .map(|(_, k)| *k)
        .collect()
}

/// Damage multiplier in percent for a round (tune::DAMAGE_CURVE).
pub fn damage_pct(round: u32) -> u32 {
    tune::DAMAGE_CURVE
        .iter()
        .rev()
        .find(|(r, _)| *r <= round)
        .map(|(_, p)| *p)
        .unwrap_or(100)
}

/// Health a defender loses for these royals killed in `round`. Each royal
/// deals at least 1.
pub fn damage(kills: &[RoyalKind], round: u32) -> u32 {
    let pct = damage_pct(round);
    kills.iter().map(|k| (k.base_damage() * pct / 100).max(1)).sum()
}

/// A standing spot in a plot, in grid cells: x from the plot's left edge,
/// y up from its floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plot {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Royal {
    pub kind: RoyalKind,
    pub spot: Option<Spot>,
}

pub fn spacing_cells() -> i32 {
    tune::ROYAL_SPACING_SEGMENTS * tune::GRID_PER_SEGMENT
}

pub fn in_plot(s: Spot, plot: Plot) -> bool {
    s.x >= 0 && s.y >= 0 && s.x <= plot.width && s.y <= plot.height
}

/// True when `s` is at least the royal spacing from every other spot.
pub fn spaced(s: Spot, others: &[Spot]) -> bool {
    let min = spacing_cells() as i64;
    others.iter().all(|o| {
        let dx = (s.x - o.x) as i64;
        let dy = (s.y - o.y) as i64;
        dx * dx + dy * dy >= min * min
    })
}

/// Can royal `index` stand at `s`, given where the others stand?
pub fn valid_spot(royals: &[Royal], index: usize, s: Spot, plot: Plot) -> bool {
    let others: Vec<Spot> = royals
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .filter_map(|(_, r)| r.spot)
        .collect();
    in_plot(s, plot) && spaced(s, &others)
}

/// Spots on the plot floor, one per grid cell: the fallback when the build
/// layer has not said where floors are.
pub fn floor_spots(plot: Plot) -> Vec<Spot> {
    (0..=plot.width).map(|x| Spot { x, y: 0 }).collect()
}

/// Place every unplaced royal at a random valid spot from `candidates`.
/// When no candidate keeps the spacing, the royal takes the candidate
/// farthest from the others, so it is placed all the same.
pub fn auto_place(royals: &mut [Royal], candidates: &[Spot], plot: Plot, dice: &mut Dice) {
    let pool: Vec<Spot> = if candidates.is_empty() {
        floor_spots(plot)
    } else {
        candidates.iter().copied().filter(|s| in_plot(*s, plot)).collect()
    };
    let pool = if pool.is_empty() { floor_spots(plot) } else { pool };
    for i in 0..royals.len() {
        if royals[i].spot.is_some() {
            continue;
        }
        let good: Vec<Spot> = pool
            .iter()
            .copied()
            .filter(|s| valid_spot(royals, i, *s, plot))
            .collect();
        let pick = if good.is_empty() {
            let others: Vec<Spot> = royals.iter().filter_map(|r| r.spot).collect();
            *pool
                .iter()
                .max_by_key(|s| nearest_sq(**s, &others))
                .expect("pool is never empty")
        } else {
            good[dice.below(good.len() as u32) as usize]
        };
        royals[i].spot = Some(pick);
    }
}

fn nearest_sq(s: Spot, others: &[Spot]) -> i64 {
    others
        .iter()
        .map(|o| {
            let dx = (s.x - o.x) as i64;
            let dy = (s.y - o.y) as i64;
            dx * dx + dy * dy
        })
        .min()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_brings_twelve_royals_with_the_king_on_thirteen() {
        let all: Vec<RoyalKind> = (1..=20).flat_map(arrivals).collect();
        assert_eq!(all.len(), 12);
        assert_eq!(arrivals(13), vec![RoyalKind::King]);
        assert_eq!(arrivals(6), vec![]);
        assert_eq!(RoyalKind::Queen.base_damage(), 6);
    }

    #[test]
    fn royal_damage_scales_with_the_round() {
        assert_eq!(damage(&[RoyalKind::Prince], 1), 4);
        assert_eq!(damage(&[RoyalKind::Prince], 8), 4);
        assert_eq!(damage(&[RoyalKind::Prince], 9), 6);
        assert_eq!(damage(&[RoyalKind::King, RoyalKind::Squire], 13), 22);
        // a squire never deals less than 1
        assert_eq!(damage(&[RoyalKind::Squire], 1), 1);
    }

    #[test]
    fn spacing_needs_four_segment_lengths() {
        let a = Spot { x: 0, y: 0 };
        assert!(!spaced(Spot { x: 15, y: 0 }, &[a]));
        assert!(spaced(Spot { x: 16, y: 0 }, &[a]));
        assert!(!spaced(Spot { x: 11, y: 11 }, &[a]));
        assert!(spaced(Spot { x: 12, y: 12 }, &[a]));
    }

    #[test]
    fn unplaced_royals_are_auto_placed_at_valid_spots() {
        let plot = Plot {
            width: 64,
            height: 32,
        };
        let mut royals = vec![
            Royal {
                kind: RoyalKind::Squire,
                spot: Some(Spot { x: 30, y: 0 }),
            },
            Royal {
                kind: RoyalKind::Knight,
                spot: None,
            },
            Royal {
                kind: RoyalKind::Squire,
                spot: None,
            },
        ];
        auto_place(&mut royals, &[], plot, &mut Dice::new(3));
        for i in 0..royals.len() {
            let s = royals[i].spot.expect("placed");
            assert!(valid_spot(&royals, i, s, plot), "royal {i} at {s:?}");
        }
        assert_eq!(royals[0].spot, Some(Spot { x: 30, y: 0 }));
    }

    #[test]
    fn a_crowded_plot_still_places_every_royal() {
        let plot = Plot { width: 8, height: 8 };
        let mut royals = vec![
            Royal {
                kind: RoyalKind::Squire,
                spot: None
            };
            3
        ];
        auto_place(&mut royals, &[], plot, &mut Dice::new(1));
        assert!(royals.iter().all(|r| r.spot.is_some()));
    }
}
