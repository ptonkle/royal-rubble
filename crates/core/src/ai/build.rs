//! Castle plans for bots: a template (keep, twin towers, tower and wall)
//! made of braced blocks, scaled to the level, the gold and stockpile, the
//! buildable span and height, then royals spread over its upper floors and
//! producers put on their map spots.
//!
//! A block is a grid of columns one bay apart and floors one unit apart, every
//! cell braced on alternating diagonals (the physics probe's castle shape,
//! which stands under the static test). Columns are wood, then stone from
//! level 4 and metal from level 7 when affordable; floors and braces are wood.
//!
//! The output is a list of placements in physics units; play/ feeds them
//! through the ledger and into the world (`to_world` shows how).

use super::tune;
use super::units;
use crate::game::flow::Difficulty;
use crate::game::ledger::{self, BuildingKind, Material, Piece, Stock};
use crate::game::sites;
use crate::game::upgrades::Modifiers;
use crate::physics::world::snap;
use crate::physics::{self, Fx, Vec2, World};
use crate::rng::Rng;
use crate::terrain::Plot;

/// A segment from node `a` to node `b` (snapped to the physics grid).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegPlan {
    pub a: Vec2,
    pub b: Vec2,
    pub material: Material,
}

/// A royal on floor segment `segment` (index into the plan's segments) at
/// `u` along it; `at` is the floor point under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoyalPlan {
    pub segment: usize,
    pub u: Fx,
    pub at: Vec2,
}

/// A producer at terrain column `col`, standing on `segment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingPlan {
    pub kind: BuildingKind,
    pub col: usize,
    pub segment: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CastlePlan {
    pub segments: Vec<SegPlan>,
    pub royals: Vec<RoyalPlan>,
    pub buildings: Vec<BuildingPlan>,
    /// Gold it spends and stockpile it uses.
    pub gold: u32,
    pub stock: Stock,
}

/// What a bot builds with.
#[derive(Clone, Copy, Debug)]
pub struct BuildAsk<'a> {
    pub plot: &'a Plot,
    pub level: u8,
    pub gold: u32,
    pub stock: Stock,
    /// Royals to place.
    pub royals: usize,
    /// Producers to put on spots, in order; skipped when not affordable or locked.
    pub buildings: &'a [BuildingKind],
    pub difficulty: Difficulty,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Keep,
    TwinTowers,
    TowerAndWall,
}

pub const TEMPLATES: [Template; 3] = [Template::Keep, Template::TwinTowers, Template::TowerAndWall];

/// A braced block: `x0` bay offset from the castle's left, `bays` wide,
/// `floors` high.
#[derive(Clone, Copy, Debug)]
struct Block {
    x0: i64,
    bays: i64,
    floors: i64,
}

/// The blocks of a template for width parameter `w` and height `f`.
fn blocks(t: Template, w: i64, f: i64) -> Vec<Block> {
    match t {
        Template::Keep => vec![Block {
            x0: 0,
            bays: w,
            floors: f,
        }],
        Template::TwinTowers => {
            let tower = 2;
            vec![
                Block {
                    x0: 0,
                    bays: tower,
                    floors: f,
                },
                Block {
                    x0: tower,
                    bays: w,
                    floors: (f / 2).max(1),
                },
                Block {
                    x0: tower + w,
                    bays: tower,
                    floors: f,
                },
            ]
        }
        Template::TowerAndWall => vec![
            Block {
                x0: 0,
                bays: w,
                floors: (f - 2).max(1),
            },
            Block {
                x0: w,
                bays: 2,
                floors: f,
            },
        ],
    }
}

fn width(bl: &[Block]) -> i64 {
    bl.iter().map(|b| b.x0 + b.bays).max().unwrap_or(0)
}

fn to_physics(m: Material) -> physics::Material {
    match m {
        Material::Wood => physics::Material::Wood,
        Material::Stone => physics::Material::Stone,
        Material::Metal => physics::Material::Metal,
    }
}

/// Gold and stockpile to buy `segs`, or `None` past `gold`.
fn cost(segs: &[SegPlan], gold: u32, stock: Stock) -> Option<(u32, Stock)> {
    let mods = Modifiers::default();
    let mut left = stock;
    let mut used = Stock::default();
    let mut spent = 0u32;
    for s in segs {
        let have = left.get_mut(s.material);
        if *have > 0 {
            *have -= 1;
            *used.get_mut(s.material) += 1;
        } else {
            spent += ledger::price(Piece::Segment(s.material), &mods);
        }
    }
    (spent <= gold).then_some((spent, used))
}

fn push_seg(segs: &mut Vec<SegPlan>, a: Vec2, b: Vec2, m: Material) -> Option<usize> {
    let (a, b) = (Vec2::new(snap(a.x), snap(a.y)), Vec2::new(snap(b.x), snap(b.y)));
    if a == b || (b - a).len() > physics::TUNE.material(to_physics(m)).max_len {
        return None;
    }
    if let Some(i) = segs
        .iter()
        .position(|s| (s.a == a && s.b == b) || (s.a == b && s.b == a))
    {
        return Some(i);
    }
    segs.push(SegPlan { a, b, material: m });
    Some(segs.len() - 1)
}

/// Lays the blocks out from terrain column `col0`; returns the segments and
/// the floor segments with their floor number (1 = lowest).
fn lay_out(plot: &Plot, col0: usize, bl: &[Block], column: Material) -> (Vec<SegPlan>, Vec<(usize, i64)>) {
    let bay = Fx::from_int(tune::BAY_UNITS);
    let floor_h = Fx::from_int(tune::FLOOR_UNITS);
    let x_of = |i: i64| units::col_x(col0) + bay * i;
    let ground = |i: i64| {
        snap(units::cells_to_fx(
            plot.heights[(col0 + i as usize).min(plot.heights.len() - 1)],
        ))
    };
    let w = width(bl);
    let top = (0..=w).map(ground).max().unwrap_or(Fx::ZERO);
    let row = |k: i64| top + floor_h * k;
    let mut segs = Vec::new();
    let mut floors = Vec::new();
    for b in bl {
        for i in b.x0..=b.x0 + b.bays {
            let x = x_of(i);
            push_seg(&mut segs, Vec2::new(x, ground(i)), Vec2::new(x, row(1)), column);
            for k in 1..b.floors {
                push_seg(&mut segs, Vec2::new(x, row(k)), Vec2::new(x, row(k + 1)), column);
            }
        }
        for i in b.x0..b.x0 + b.bays {
            let (xl, xr) = (x_of(i), x_of(i + 1));
            for k in 0..b.floors {
                let lo = |j: i64, x: Fx| {
                    if k == 0 {
                        Vec2::new(x, ground(j))
                    } else {
                        Vec2::new(x, row(k))
                    }
                };
                if let Some(s) = push_seg(
                    &mut segs,
                    Vec2::new(xl, row(k + 1)),
                    Vec2::new(xr, row(k + 1)),
                    Material::Wood,
                ) {
                    if !floors.iter().any(|(f, _)| *f == s) {
                        floors.push((s, k + 1));
                    }
                }
                if (i + k) % 2 == 0 {
                    push_seg(&mut segs, lo(i, xl), Vec2::new(xr, row(k + 1)), Material::Wood);
                } else {
                    push_seg(
                        &mut segs,
                        lo(i + 1, xr),
                        Vec2::new(xl, row(k + 1)),
                        Material::Wood,
                    );
                }
            }
        }
    }
    (segs, floors)
}

/// Ground rise across `bays` from `col0`, raw.
fn rise(plot: &Plot, col0: usize, bays: i64) -> i64 {
    let hs: Vec<i32> = (col0..=col0 + bays as usize)
        .map(|c| plot.heights[c.min(plot.heights.len() - 1)])
        .collect();
    let (lo, hi) = (
        hs.iter().min().copied().unwrap_or(0),
        hs.iter().max().copied().unwrap_or(0),
    );
    units::cells_to_fx(hi - lo).raw()
}

/// Column material for a level and budget.
fn column_material(level: u8) -> Material {
    if level >= tune::METAL_LEVEL {
        Material::Metal
    } else if level >= tune::STONE_LEVEL {
        Material::Stone
    } else {
        Material::Wood
    }
}

/// The bot's castle plan.
pub fn plan_castle(ask: &BuildAsk) -> CastlePlan {
    let mut rng = Rng::new(ask.seed);
    let level = ask.level.clamp(1, 9);
    let span = ask.plot.buildable(level);
    let mut gold = ask.gold.saturating_sub(tune::reserve_gold(ask.difficulty));
    // producers first: they pay every round
    let mut want = Vec::new();
    for &k in ask.buildings {
        let row = ledger::building_row(k);
        if row.level <= level && row.cost <= gold {
            gold -= row.cost;
            want.push(k);
        }
    }
    let max_floors = tune::MAX_FLOORS[level as usize - 1]
        .min(Plot::build_height(level) as i64 / crate::terrain::CELL as i64 - 1);
    let max_rise = Fx::ONE.raw() / 2;
    let setback = tune::FRONT_SETBACK;
    let start = rng.below(TEMPLATES.len() as u32) as usize;
    // the best plan with each column material, strongest first
    type Best = Option<(u64, Vec<SegPlan>, Vec<(usize, i64)>, Stock)>;
    let mut best: [Best; 3] = [None, None, None];
    let mats: Vec<Material> = {
        let top = column_material(level);
        [Material::Metal, Material::Stone, Material::Wood]
            .into_iter()
            .filter(|m| *m <= top)
            .collect()
    };
    for n in 0..TEMPLATES.len() {
        let t = TEMPLATES[(start + n) % TEMPLATES.len()];
        for f in (1..=max_floors).rev() {
            for w in (1..=tune::MAX_BAYS).rev() {
                let bl = blocks(t, w, f);
                let bays = width(&bl);
                // nearest the front with ground level enough
                let Some(col0) = (span.start + setback..span.end)
                    .take_while(|c| c + bays as usize + 1 < span.end)
                    .find(|&c| rise(ask.plot, c, bays) <= max_rise)
                else {
                    continue;
                };
                for &m in &mats {
                    let (segs, floors) = lay_out(ask.plot, col0, &bl, m);
                    if let Some((_, used)) = cost(&segs, gold, ask.stock) {
                        // worth most first (stronger material wins), then more pieces
                        let worth = cost(&segs, u32::MAX, Stock::default()).map_or(0, |c| c.0);
                        // royals kept apart first, then worth, then pieces
                        let spaced = spaced(&place_royals(&segs, &floors, ask.royals)) as u64;
                        let size = (spaced << 40) + ((worth as u64) << 20) + segs.len() as u64;
                        let slot = &mut best[mats.iter().position(|x| *x == m).unwrap_or(0)];
                        if slot.as_ref().is_none_or(|b| size > b.0) {
                            *slot = Some((size, segs, floors, used));
                        }
                    }
                }
            }
        }
        if best.iter().any(|b| b.is_some()) {
            break;
        }
    }
    // the strongest material whose castle is worth at least half the best
    let top = best.iter().flatten().map(|b| b.0).max().unwrap_or(0);
    let pick = best.into_iter().flatten().find(|b| {
        b.0 >> 40 == top >> 40 && b.0 * tune::STRONGER_SHARE.1 as u64 >= top * tune::STRONGER_SHARE.0 as u64
    });
    let (_, mut segments, floors, stock) = pick.unwrap_or_default();
    let royals = place_royals(&segments, &floors, ask.royals);
    let buildings = place_buildings(ask.plot, level, &want, &mut segments, &floors);
    let (spent, stock) = cost(&segments, u32::MAX, ask.stock).unwrap_or((0, stock));
    let building_gold: u32 = buildings.iter().map(|b| ledger::building_row(b.kind).cost).sum();
    CastlePlan {
        segments,
        royals,
        buildings,
        gold: spent + building_gold,
        stock,
    }
}

/// Royals over the floors, upper floors first, each at least the royal
/// spacing from the others where the castle allows it, else as far as it can.
fn place_royals(segs: &[SegPlan], floors: &[(usize, i64)], n: usize) -> Vec<RoyalPlan> {
    let top = floors.iter().map(|f| f.1).max().unwrap_or(0);
    let spacing = units::royal_spacing();
    let mut out: Vec<RoyalPlan> = Vec::new();
    for _ in 0..n {
        let pick = floors
            .iter()
            .filter(|(s, _)| !out.iter().any(|r| r.segment == *s))
            .map(|&(s, k)| {
                let at = Vec2::lerp(segs[s].a, segs[s].b, Fx::HALF);
                let gap = out
                    .iter()
                    .map(|r| (r.at - at).len())
                    .min()
                    .unwrap_or(Fx::from_int(1_000));
                // inside: below the roof when there is more than one floor
                let inside = top > 1 && k < top;
                (gap >= spacing, inside, k, gap, s, at)
            })
            .max_by_key(|&(ok, inside, k, gap, s, _)| {
                (
                    ok,
                    if ok {
                        (inside, k, Fx::ZERO)
                    } else {
                        (false, 0, gap)
                    },
                    usize::MAX - s,
                )
            });
        let Some((_, _, _, _, s, at)) = pick else {
            break;
        };
        out.push(RoyalPlan {
            segment: s,
            u: Fx::HALF,
            at,
        });
    }
    out
}

/// How many royals stand at least the spacing from every royal placed before.
fn spaced(royals: &[RoyalPlan]) -> usize {
    let spacing = units::royal_spacing();
    royals
        .iter()
        .enumerate()
        .filter(|(i, r)| royals[..*i].iter().all(|o| (o.at - r.at).len() >= spacing))
        .count()
}

/// Producers on their spots: on the lowest castle floor over the column, or
/// on a one-bay wooden floor laid on the ground there.
fn place_buildings(
    plot: &Plot,
    level: u8,
    want: &[BuildingKind],
    segs: &mut Vec<SegPlan>,
    floors: &[(usize, i64)],
) -> Vec<BuildingPlan> {
    let mut taken: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for &kind in want {
        let col = sites::best_column(plot, level, kind, &taken);
        taken.push(col);
        let x = units::col_x(col);
        let on_floor = floors
            .iter()
            .filter(|(s, _)| segs[*s].a.x.min(segs[*s].b.x) <= x && x < segs[*s].a.x.max(segs[*s].b.x))
            .min_by_key(|f| f.1)
            .map(|f| f.0);
        let segment = match on_floor {
            Some(s) => Some(s),
            None => {
                let g = |c: usize| units::cells_to_fx(plot.heights[c.min(plot.heights.len() - 1)]);
                push_seg(
                    segs,
                    Vec2::new(x, g(col)),
                    Vec2::new(x + units::col_width(), g(col + 1)),
                    Material::Wood,
                )
            }
        };
        if let Some(segment) = segment {
            out.push(BuildingPlan { kind, col, segment });
        }
    }
    out
}

/// Builds a plan into a physics world on the plot's terrain; returns the
/// world and the royals' body indices. Buildings become bodies too, after
/// the royals.
pub fn to_world(plan: &CastlePlan, plot: &Plot) -> (World, Vec<usize>) {
    let mut w = World::new(units::physics_terrain(plot));
    let ids: Vec<Option<usize>> = plan
        .segments
        .iter()
        .map(|s| w.add_beam(s.a, s.b, to_physics(s.material)).ok())
        .collect();
    let mut royals = Vec::new();
    for r in &plan.royals {
        if let Some(s) = ids[r.segment] {
            royals.push(w.add_body(s, r.u, tune::royal_radius()));
        }
    }
    for b in &plan.buildings {
        if let Some(s) = ids[b.segment] {
            w.add_body(s, Fx::HALF, tune::royal_radius());
        }
    }
    (w, royals)
}
