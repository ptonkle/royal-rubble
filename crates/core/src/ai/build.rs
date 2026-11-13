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
use crate::game::flow::{Difficulty, Seat};
use crate::game::ledger::{self, BuildingKind, Material, Piece, Stock};
use crate::game::sites;
use crate::game::upgrades::Modifiers;
use crate::game::PlayerId;
use crate::physics::world::snap;
use crate::physics::{self, Fx, Vec2, World};
use crate::play::castle::Geo;
use crate::play::units::GridPt;
use crate::play::Input;
use crate::play::Play;
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
/// `u` along it, or on the ground inside the castle (`None`); `at` is the
/// point under its feet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoyalPlan {
    pub segment: Option<usize>,
    pub u: Fx,
    pub at: Vec2,
    /// Under at least one floor.
    pub covered: bool,
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

/// A segment the castle already has: its ledger id, where it is, and the
/// gold selling it gives back. A plan keeps it for free or sells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Have {
    pub id: u32,
    pub seg: SegPlan,
    pub refund: u32,
}

/// What a bot builds with.
#[derive(Clone, Copy, Debug)]
pub struct BuildAsk<'a> {
    /// Segments already standing (grow-on-top, R1.1451).
    pub have: &'a [Have],
    pub plot: &'a Plot,
    pub level: u8,
    pub gold: u32,
    pub stock: Stock,
    /// Royals to place.
    pub royals: usize,
    /// Producers to put on spots, in order; skipped when not affordable or locked.
    pub buildings: &'a [BuildingKind],
    pub difficulty: Difficulty,
    /// The round, for how much gold the bot banks.
    pub round: u32,
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

/// The blocks of a template for width parameter `w` and height `f`, in
/// rooms of `ROOM_ROWS` rows each (R1.2500).
fn blocks(t: Template, w: i64, f: i64) -> Vec<Block> {
    let mut bl = rooms(t, w, f);
    for b in &mut bl {
        b.floors *= tune::ROOM_ROWS;
    }
    bl
}

fn rooms(t: Template, w: i64, f: i64) -> Vec<Block> {
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
                floors: (f - 1).max(1),
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

fn same(a: &SegPlan, b: &SegPlan) -> bool {
    a.material == b.material && ((a.a == b.a && a.b == b.b) || (a.a == b.b && a.b == b.a))
}

/// Gold and stockpile to buy `segs`, or `None` past `gold`. Segments the
/// castle already has (`have`) cost nothing; those the plan drops are sold
/// and their refund counts toward `gold`.
fn cost(segs: &[SegPlan], gold: u32, stock: Stock, have: &[Have]) -> Option<(u32, Stock)> {
    let mods = Modifiers::default();
    let mut left = stock;
    let mut used = Stock::default();
    let mut spent = 0u32;
    let refund: u32 = have
        .iter()
        .filter(|h| !segs.iter().any(|s| same(s, &h.seg)))
        .map(|h| h.refund)
        .sum();
    let gold = gold.saturating_add(refund);
    for s in segs {
        if have.iter().any(|h| same(s, &h.seg)) {
            continue;
        }
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
        // a floor every room (R1.2500) and on the top row
        let floored = |r: i64| r % tune::ROOM_ROWS == 0 || r == b.floors;
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
                if floored(k + 1) {
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
                }
                // on a slope the preferred diagonal can be too long for one
                // beam; the other one, from the higher ground, then braces
                let left = (lo(i, xl), Vec2::new(xr, row(k + 1)));
                let right = (lo(i + 1, xr), Vec2::new(xl, row(k + 1)));
                let (first, second) = if (i + k) % 2 == 0 {
                    (left, right)
                } else {
                    (right, left)
                };
                // no floor on top: braced both ways, so the uprights' middle
                // knots hold without one
                if push_seg(&mut segs, first.0, first.1, Material::Wood).is_none() || !floored(k + 1) {
                    push_seg(&mut segs, second.0, second.1, Material::Wood);
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
    let mut gold = ask
        .gold
        .saturating_sub(tune::reserve_gold(ask.difficulty, ask.round));
    // producers first: they pay every round
    let mut want = Vec::new();
    let mut producers = 0;
    for &k in ask.buildings {
        let row = ledger::building_row(k);
        if row.level <= level && row.cost <= gold {
            gold -= row.cost;
            producers += row.cost;
            want.push(k);
        }
    }
    // sheltering the royals comes before producers and may spend the
    // banked gold too (R1.2301)
    let shelter_gold = ask.gold;
    let max_floors = tune::MAX_FLOORS[level as usize - 1]
        .min(Plot::build_height(level) as i64 / crate::terrain::CELL as i64 - 1);
    let max_rise = tune::MAX_RISE_RAW;
    let setback = tune::FRONT_SETBACK;
    let start = rng.below(TEMPLATES.len() as u32) as usize;
    // the best plan with each column material, strongest first
    type Best = Option<(u64, Vec<SegPlan>, Vec<(usize, i64)>, Stock, Vec<RoyalPlan>)>;
    let mut best: [Best; 3] = [None, None, None];
    let mats: Vec<Material> = {
        let top = column_material(level);
        [Material::Metal, Material::Stone, Material::Wood]
            .into_iter()
            .filter(|m| *m <= top)
            .collect()
    };
    // the keep with the gold producers leave, then, while a royal is left
    // out of cover, with the producers' gold too (rooms cost more than the
    // old one-row floors, R1.2500)
    for (pass, n) in (0..2).flat_map(|p| (0..TEMPLATES.len()).map(move |n| (p, n))) {
        if pass == 1 && n == 0 && best.iter().flatten().any(|b| (b.0 >> 44) as usize >= ask.royals) {
            break;
        }
        let budget = if pass == 0 { gold } else { shelter_gold };
        let t = TEMPLATES[(start + n) % TEMPLATES.len()];
        for f in (1..=max_floors / tune::ROOM_ROWS).rev() {
            for w in (1..=tune::MAX_BAYS).rev() {
                let bl = blocks(t, w, f);
                let bays = width(&bl);
                // one room: a framed hall (posts, roof, an end strut each
                // side) where the ground is level enough, as a hut is
                // (R1.2500); else braced rows
                let terrain = units::physics_terrain(ask.plot);
                let sites =
                    || (span.start + setback..span.end).take_while(|c| c + bays as usize + 1 < span.end);
                let hall = (f == 1)
                    .then(|| sites().find_map(|c| frame(&terrain, c, bays, Material::Wood).map(|_| c)))
                    .flatten();
                // nearest the front with ground level enough
                let Some(col0) = hall.or_else(|| sites().find(|&c| rise(ask.plot, c, bays) <= max_rise))
                else {
                    continue;
                };
                for &m in &mats {
                    let (main, main_floors) = match hall.and_then(|c| frame(&terrain, c, bays, m)) {
                        Some((segs, roofs)) => {
                            (segs, roofs.into_iter().map(|r| (r, tune::ROOM_ROWS)).collect())
                        }
                        None => lay_out(ask.plot, col0, &bl, m),
                    };
                    if cost(&main, budget, ask.stock, ask.have).is_none() {
                        continue;
                    }
                    // huts for the royals the keep cannot shelter, as many
                    // as the gold allows (R1.2300)
                    let (kept, huts) = huts(ask.plot, span, setback, &main, &main_floors, ask.royals);
                    for h in (0..=huts.len()).rev() {
                        let mut segs = main.clone();
                        let mut floors = main_floors.clone();
                        let mut royals = kept.clone();
                        for hut in &huts[..h] {
                            let base = segs.len();
                            segs.extend(hut.segs.iter().copied());
                            floors.push((base + hut.floor, 1));
                            royals.push(hut.royal);
                        }
                        // never smaller than the castle already standing: a
                        // segment the plan has no place for stays where it
                        // is, unless the plan puts another material there
                        // (R1.2302)
                        for h in ask.have {
                            let at = |s: &SegPlan| {
                                (s.a == h.seg.a && s.b == h.seg.b) || (s.a == h.seg.b && s.b == h.seg.a)
                            };
                            if !segs.iter().any(at) {
                                segs.push(h.seg);
                            }
                        }
                        let Some((_, used)) = cost(&segs, shelter_gold, ask.stock, ask.have) else {
                            continue;
                        };
                        // any other sheltered spot (an older hut, a bay of an
                        // older keep), then the uncovered royals the keep had
                        for r in shelters(ask.plot, span, &segs) {
                            if royals.len() < ask.royals && apart(ask.plot, r.at, &royals) {
                                royals.push(r);
                            }
                        }
                        for r in place_royals(ask.plot, &main, &main_floors, ask.royals) {
                            if royals.len() < ask.royals && !r.covered && apart(ask.plot, r.at, &royals) {
                                royals.push(r);
                            }
                        }
                        // royals under cover first, then royals kept apart,
                        // then worth (stronger material wins), then pieces
                        let worth = cost(&segs, u32::MAX, Stock::default(), &[]).map_or(0, |c| c.0);
                        let (spaced, covered) = spaced(ask.plot, &royals);
                        let size = ((covered as u64) << 44)
                            + ((spaced as u64) << 40)
                            + ((worth as u64) << 20)
                            + segs.len() as u64;
                        let slot = &mut best[mats.iter().position(|x| *x == m).unwrap_or(0)];
                        if slot.as_ref().is_none_or(|b| size > b.0) {
                            *slot = Some((size, segs, floors, used, royals));
                        }
                        break;
                    }
                }
            }
        }
        // the seed's template wins unless another keeps more royals apart
        if best.iter().flatten().any(|b| (b.0 >> 44) as usize >= ask.royals) {
            break;
        }
    }
    // no keep affordable (a room costs more than the first round's gold):
    // huts alone, as many as the gold allows (R1.2500)
    if best.iter().all(|b| b.is_none()) {
        let (_, huts) = huts(ask.plot, span, setback, &[], &[], ask.royals);
        for h in (1..=huts.len()).rev() {
            let mut segs: Vec<SegPlan> = huts[..h].iter().flat_map(|x| x.segs.iter().copied()).collect();
            let mut floors = Vec::new();
            let mut base = 0;
            for hut in &huts[..h] {
                floors.push((base + hut.floor, 1));
                base += hut.segs.len();
            }
            for hv in ask.have {
                if !segs
                    .iter()
                    .any(|s| same(s, &hv.seg) || (s.a == hv.seg.a && s.b == hv.seg.b))
                {
                    segs.push(hv.seg);
                }
            }
            let Some((_, used)) = cost(&segs, shelter_gold, ask.stock, ask.have) else {
                continue;
            };
            let royals: Vec<RoyalPlan> = huts[..h].iter().map(|x| x.royal).collect();
            best[mats.len() - 1] = Some((0, segs, floors, used, royals));
            break;
        }
    }
    // the strongest material whose castle is worth at least half the best
    let top = best.iter().flatten().map(|b| b.0).max().unwrap_or(0);
    let pick = best.into_iter().flatten().find(|b| {
        b.0 >> 40 == top >> 40 && b.0 * tune::STRONGER_SHARE.1 as u64 >= top * tune::STRONGER_SHARE.0 as u64
    });
    let (_, mut segments, floors, stock, royals) = pick.unwrap_or_default();
    // producers with the gold the shelter left
    let mut left = ask
        .gold
        .saturating_sub(cost(&segments, u32::MAX, ask.stock, ask.have).map_or(0, |c| c.0));
    want.retain(|k| {
        let c = ledger::building_row(*k).cost;
        let ok = c <= left && producers > 0;
        if ok {
            left -= c;
        }
        ok
    });
    let buildings = place_buildings(ask.plot, level, &want, &mut segments, &floors);
    let (spent, stock) = cost(&segments, u32::MAX, ask.stock, ask.have).unwrap_or((0, stock));
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
fn place_royals(plot: &Plot, segs: &[SegPlan], floors: &[(usize, i64)], n: usize) -> Vec<RoyalPlan> {
    let terrain = units::physics_terrain(plot);
    // every floor, and the ground in the middle of each bay under the lowest
    // floor (floor 0: covered, and no fall)
    let mut spots: Vec<(Option<usize>, i64, Vec2)> = floors
        .iter()
        .map(|&(s, k)| (Some(s), k, Vec2::lerp(segs[s].a, segs[s].b, Fx::HALF)))
        .collect();
    for &(s, k) in floors {
        if k > tune::ROOM_ROWS {
            continue;
        }
        let mid = Vec2::lerp(segs[s].a, segs[s].b, Fx::HALF);
        spots.push((None, 0, Vec2::new(mid.x, snap(terrain.height_at(mid.x)))));
    }
    let mut out: Vec<RoyalPlan> = Vec::new();
    for _ in 0..n {
        let pick = spots
            .iter()
            .enumerate()
            .filter(|(_, (_, _, at))| !out.iter().any(|r| r.at == *at))
            .map(|(s, &(_, k, at))| {
                let gap = out
                    .iter()
                    .map(|r| (r.at - at).len())
                    .min()
                    .unwrap_or(Fx::from_int(1_000));
                // inside: some segment passes over it (as the siege sees it)
                let inside = roofed(segs, at);
                (apart(plot, at, &out), inside, k, gap, s, at)
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
        let Some((_, inside, _, _, s, at)) = pick else {
            break;
        };
        out.push(RoyalPlan {
            segment: spots[s].0,
            u: Fx::HALF,
            at,
            covered: inside,
        });
    }
    out
}

/// How many royals stand at least the spacing from every royal placed
/// before, and how many of those are under cover.
fn spaced(plot: &Plot, royals: &[RoyalPlan]) -> (usize, usize) {
    let ok: Vec<&RoyalPlan> = royals
        .iter()
        .enumerate()
        .filter(|(i, r)| apart(plot, r.at, &royals[..*i]))
        .map(|(_, r)| r)
        .collect();
    (ok.len(), ok.iter().filter(|r| r.covered).count())
}

/// The royal rules' spacing between a royal at `at` and `others`, measured
/// as the match measures it (columns across, cells above the ground), so a
/// plan never offers a spot the match refuses.
fn apart(plot: &Plot, at: Vec2, others: &[RoyalPlan]) -> bool {
    let spot = |p: Vec2| crate::play::castle::rule_spot(plot, 1, units::grid_pt(p));
    let taken: Vec<crate::game::royals::Spot> = others.iter().map(|r| spot(r.at)).collect();
    crate::game::royals::spaced(spot(at), &taken)
}

/// Every sheltered spot of `segs`: the ground in the middle of each column
/// of the span and the middle of each level segment, back of the plot first.
fn shelters(plot: &Plot, span: crate::terrain::ColRange, segs: &[SegPlan]) -> Vec<RoyalPlan> {
    let terrain = units::physics_terrain(plot);
    let mut out: Vec<RoyalPlan> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let at = Vec2::lerp(s.a, s.b, Fx::HALF);
        if s.a.y == s.b.y && units::grid_pt(at).to_fx() == at && roofed(segs, at) {
            out.push(RoyalPlan {
                segment: Some(i),
                u: Fx::HALF,
                at,
                covered: true,
            });
        }
    }
    for c in span.start..span.end {
        let mid = units::col_x(c) + units::col_width() / 2;
        let at = Vec2::new(mid, snap(terrain.height_at(mid)));
        if roofed(segs, at) {
            out.push(RoyalPlan {
                segment: None,
                u: Fx::HALF,
                at,
                covered: true,
            });
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.at.x));
    out
}

/// A floor passes over `at` at least a hut's height above it (R1.2500): a
/// level segment strictly across its x, so the royal's figure fits under it.
fn roofed(segs: &[SegPlan], at: Vec2) -> bool {
    let room = Fx::ratio(tune::HUT_ROOM_EIGHTHS, 8);
    segs.iter()
        .any(|s| s.a.y == s.b.y && s.a.x.min(s.b.x) < at.x && at.x < s.a.x.max(s.b.x) && s.a.y - at.y >= room)
}

/// A one-bay braced hut on its own patch of ground: its segments, which of
/// them is its roof, and the royal on the ground under it.
#[derive(Clone, Debug)]
struct Hut {
    segs: Vec<SegPlan>,
    floor: usize,
    royal: RoyalPlan,
}

/// The royals the keep `segs` shelters (kept apart), and huts for as many
/// of the other royals as there are sites: a bay whose ground rises at
/// most `MAX_RISE_RAW`, clear of the keep and of each other, its royal the
/// spacing from every other. Sites are taken from the back of the first
/// level's plot forward, so they stay put while the keep grows from the
/// front and the plot widens.
fn huts(
    plot: &Plot,
    span: crate::terrain::ColRange,
    setback: usize,
    segs: &[SegPlan],
    floors: &[(usize, i64)],
    n: usize,
) -> (Vec<RoyalPlan>, Vec<Hut>) {
    let mut kept: Vec<RoyalPlan> = Vec::new();
    for r in place_royals(plot, segs, floors, n) {
        if r.covered && apart(plot, r.at, &kept) {
            kept.push(r);
        }
    }
    let lo = segs.iter().map(|s| s.a.x.min(s.b.x)).min().unwrap_or(Fx::ZERO);
    let hi = segs.iter().map(|s| s.a.x.max(s.b.x)).max().unwrap_or(Fx::ZERO);
    let terrain = units::physics_terrain(plot);
    let mut out: Vec<Hut> = Vec::new();
    let mut placed = kept.clone();
    // back to front over the first level's span, then out over the land
    // later levels add, so a site never moves when the plot widens
    let first = span.start + crate::terrain::tune::PLOT_MIN_W.min(span.len());
    // with no keep yet, its front site stays free for the keep to come
    let front = span.start
        + setback
        + if segs.is_empty() {
            crate::game::royals::spacing_cells() as usize
        } else {
            0
        };
    let order = (front..first.saturating_sub(2))
        .rev()
        .chain(first.saturating_sub(2)..span.end.saturating_sub(2));
    // framed huts first (five beams), then braced rooms on slopes
    let order: Vec<usize> = order.collect();
    for (slope, &c) in [false, true]
        .into_iter()
        .flat_map(|p| order.iter().map(move |c| (p, c)))
    {
        if placed.len() >= n {
            break;
        }
        let (xl, xr) = (units::col_x(c), units::col_x(c + 1));
        let clear = segs.is_empty() || xr + Fx::ONE <= lo || xl >= hi + Fx::ONE;
        let mid = xl + units::col_width() / 2;
        let at = Vec2::new(mid, snap(terrain.height_at(mid)));
        if clear && apart(plot, at, &placed) {
            if let Some((hs, floor)) = hut_frame(plot, &terrain, c, slope) {
                if roofed(&hs, at) {
                    let royal = RoyalPlan {
                        segment: None,
                        u: Fx::HALF,
                        at,
                        covered: true,
                    };
                    placed.push(royal);
                    out.push(Hut {
                        segs: hs,
                        floor,
                        royal,
                    });
                }
            }
        }
    }
    (kept, out)
}

/// A hut over terrain column `c` (R1.2500): a one-bay `frame`, or on a
/// slope too steep for one, a braced room one bay wide. Returns the
/// segments and which is the roof.
fn hut_frame(
    plot: &Plot,
    terrain: &physics::Terrain,
    c: usize,
    slope: bool,
) -> Option<(Vec<SegPlan>, usize)> {
    if let Some((segs, roofs)) = frame(terrain, c, 1, Material::Wood) {
        return (!slope).then_some((segs, roofs[0]));
    }
    if !slope || rise(plot, c, 1) > tune::MAX_RISE_RAW {
        return None;
    }
    let room = [Block {
        x0: 0,
        bays: 1,
        floors: tune::ROOM_ROWS,
    }];
    let (segs, floors) = lay_out(plot, c, &room, Material::Wood);
    floors.first().map(|&(f, _)| (segs, f))
}

/// A one-room hall `bays` wide from terrain column `c` (R1.2500): a post
/// (in `post` material) at every bay edge, the roof between their tops and
/// a strut from each end post's top out to the ground, each a whole beam;
/// `None` where the ground makes a beam too long. Returns the segments and
/// which are the roof.
fn frame(
    terrain: &physics::Terrain,
    c: usize,
    bays: i64,
    post: Material,
) -> Option<(Vec<SegPlan>, Vec<usize>)> {
    let q = |n: i64| Fx::ratio(n, 4);
    let x = |i: i64| units::col_x(c) + Fx::from_int(tune::BAY_UNITS) * i;
    let ground = |x: Fx| Vec2::new(x, snap(terrain.height_at(x)));
    // level roof the head room over every bay's middle, on the grid; the
    // posts reach down to the ground each side, of different lengths on a
    // slope (R1.2502)
    let need = (0..bays)
        .map(|i| ground(x(i) + Fx::from_int(tune::BAY_UNITS) / 2).y)
        .max()?
        + Fx::ratio(tune::HUT_ROOM_EIGHTHS, 8);
    let roof = if snap(need) < need {
        snap(need) + q(1)
    } else {
        snap(need)
    };
    let at = |i: i64| Vec2::new(x(i), roof);
    let mut segs = Vec::new();
    let mut roofs = Vec::new();
    for i in 0..=bays {
        push_seg(&mut segs, ground(x(i)), at(i), post)?;
    }
    for i in 0..bays {
        roofs.push(push_seg(&mut segs, at(i), at(i + 1), Material::Wood)?);
    }
    // a strut from each end post's top to the ground: outward as far as a
    // beam reaches, else inward under the roof
    for (end, out) in [(0, -1i64), (bays, 1)] {
        let reaches = (1..=tune::HUT_STRUT_QUARTERS)
            .rev()
            .map(|k| k * out)
            .chain((1..=tune::HUT_STRUT_QUARTERS).map(|k| -k * out));
        let mut ok = false;
        for k in reaches {
            if push_seg(&mut segs, ground(x(end) + q(k)), at(end), Material::Wood).is_some() {
                ok = true;
                break;
            }
        }
        if !ok {
            return None;
        }
    }
    Some((segs, roofs))
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
        match r.segment {
            Some(seg) => {
                if let Some(s) = ids[seg] {
                    let b = w.add_body(s, r.u, tune::royal_radius());
                    w.bodies[b].tall = crate::play::tune::ROYAL_UPRIGHT;
                    royals.push(b);
                }
            }
            None => {
                let radius = tune::royal_radius();
                let pos = Vec2::new(r.at.x, w.terrain.height_at(r.at.x) + radius);
                w.bodies.push(physics::Body {
                    pos,
                    prev: pos,
                    radius,
                    tall: crate::play::tune::ROYAL_UPRIGHT,
                    attached: None,
                    peak_y: pos.y,
                    alive: true,
                });
                royals.push(w.bodies.len() - 1);
            }
        }
    }
    for b in &plan.buildings {
        if let Some(s) = ids[b.segment] {
            w.add_body(s, Fx::HALF, crate::play::tune::BUILDING_RADIUS);
        }
    }
    (w, royals)
}

/// The plan as match inputs, in the order to feed them through
/// `play::Play::apply` during a build: segments, then producers, then royals
/// (royal `i` of the plan is royal index `royal_index[i]`, or `i` when the
/// list is short).
pub fn to_inputs(plan: &CastlePlan, royal_index: &[u8]) -> Vec<Input> {
    let mut out: Vec<Input> = plan
        .segments
        .iter()
        .map(|s| Input::PlaceSegment {
            piece: Piece::Segment(s.material),
            a: units::grid_pt(s.a),
            b: units::grid_pt(s.b),
        })
        .collect();
    for b in &plan.buildings {
        let s = plan.segments[b.segment];
        // the floor's left end stands in the building's column
        let at = if s.a.x <= s.b.x { s.a } else { s.b };
        out.push(Input::PlaceBuilding {
            kind: b.kind,
            at: units::grid_pt(at),
        });
    }
    for (i, r) in plan.royals.iter().enumerate() {
        out.push(Input::PlaceRoyal {
            index: royal_index.get(i).copied().unwrap_or(i as u8),
            at: units::grid_pt(r.at),
        });
    }
    out
}

/// Everything bot `p` does with its castle at the start of a build phase,
/// as inputs to feed through `Play::apply` in order: sell what the new plan
/// drops (an older material, a moved wall), place what it adds, put
/// producers on their spots, then stand the royals, the most valuable in
/// the best-covered places (R1.1451). The plan keeps every segment it can
/// for free, so the castle grows each round with the gold left after the
/// bank (`tune::reserve_gold`) and the stockpile.
pub fn bot_turn(play: &Play, p: PlayerId) -> Vec<Input> {
    let Some(pl) = play.rules.player(p) else {
        return Vec::new();
    };
    let castle = &play.castles[p as usize];
    let round = play.rules.round;
    let difficulty = match pl.seat {
        Seat::Bot(d) => d,
        Seat::Human => Difficulty::Normal,
    };
    let have: Vec<Have> = pl
        .ledger
        .entries
        .iter()
        .filter_map(|e| {
            let Piece::Segment(material) = e.piece else {
                return None;
            };
            let Some(Geo::Beam { a, b }) = castle.geo.get(&e.id) else {
                return None;
            };
            let refund = if e.round == round {
                e.paid_gold
            } else {
                e.value * crate::game::tune::SELL_LATER_NUM / crate::game::tune::SELL_LATER_DEN
            };
            Some(Have {
                id: e.id,
                seg: SegPlan {
                    a: a.to_fx(),
                    b: b.to_fx(),
                    material,
                },
                refund,
            })
        })
        .collect();
    let level = pl.purse.level;
    let wanted: Vec<BuildingKind> = tune::BOT_PRODUCERS
        .iter()
        .copied()
        .filter(|k| !pl.ledger.has_building(*k) && level >= ledger::unlock_level(Piece::Building(*k)))
        .filter(|_| have.len() >= tune::PRODUCERS_AFTER_SEGMENTS)
        .collect();
    let ask = BuildAsk {
        have: &have,
        plot: &pl.terrain,
        level,
        gold: pl.purse.gold,
        stock: pl.ledger.stock,
        royals: pl.royals.len(),
        buildings: &wanted,
        difficulty,
        round,
        // the round stays out, so the template holds from round to round
        seed: play.rules.settings.seed ^ (u64::from(p) << 32),
    };
    let plan = plan_castle(&ask);
    if plan.segments.is_empty() && !have.is_empty() {
        // no plan fits: keep the castle as it stands rather than sell it
        return Vec::new();
    }
    let mut out: Vec<Input> = have
        .iter()
        .filter(|h| !plan.segments.iter().any(|s| same(s, &h.seg)))
        .map(|h| Input::Sell(h.id))
        .collect();
    // royals by worth, most valuable first, onto the plan's best spots first
    let mut order: Vec<u8> = (0..pl.royals.len() as u8).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(pl.royals[i as usize].kind.base_damage()));
    let spot = |g: GridPt| crate::play::castle::rule_spot(&pl.terrain, level, g);
    let mut taken: Vec<crate::game::royals::Spot> = Vec::new();
    let mut moves: Vec<(u8, GridPt)> = Vec::new();
    // a royal already on a spot of the plan stays there, so two royals
    // never wait on each other's places (R1.2303)
    let mut spots: Vec<GridPt> = Vec::new();
    for r in &plan.royals {
        let g = units::grid_pt(r.at);
        if spots.len() < order.len() && crate::game::royals::spaced(spot(g), &taken) {
            taken.push(spot(g));
            spots.push(g);
        }
    }
    let mut by: Vec<Option<u8>> = spots
        .iter()
        .map(|g| {
            order
                .iter()
                .copied()
                .find(|&i| castle.royal_at.get(i as usize).copied().flatten() == Some(*g))
        })
        .collect();
    let free: Vec<u8> = order
        .iter()
        .copied()
        .filter(|i| !by.contains(&Some(*i)))
        .collect();
    let mut rest = free.into_iter();
    for b in by.iter_mut() {
        if b.is_none() {
            *b = rest.next();
        }
    }
    let order: Vec<u8> = by.iter().flatten().copied().chain(rest).collect();
    let k = spots.len();
    for (i, g) in spots.iter().enumerate() {
        moves.push((order[i], *g));
    }
    // the rest stand on open ground toward the back of the plot, apart
    // from the others, so they never block a royal headed for cover
    let terrain = units::physics_terrain(&pl.terrain);
    let span = pl.terrain.buildable(level);
    let mut away: Vec<(u8, GridPt)> = Vec::new();
    for &index in &order[k..] {
        let found = (span.start..span.end).rev().find_map(|c| {
            let x = units::col_x(c) + units::col_width() / 2;
            let g = units::grid_pt(Vec2::new(x, snap(terrain.height_at(x))));
            crate::game::royals::spaced(spot(g), &taken).then_some(g)
        });
        if let Some(g) = found {
            taken.push(spot(g));
            away.push((index, g));
        }
    }
    let inputs = to_inputs(
        &CastlePlan {
            royals: Vec::new(),
            ..plan
        },
        &[],
    );
    out.extend(inputs.into_iter().filter(|i| match i {
        Input::PlaceSegment { piece, a, b } => {
            let seg = SegPlan {
                a: a.to_fx(),
                b: b.to_fx(),
                material: match piece {
                    Piece::Segment(m) => *m,
                    _ => Material::Wood,
                },
            };
            !have.iter().any(|h| same(&seg, &h.seg))
        }
        _ => true,
    }));
    // a royal refused for standing too near another's old place takes its
    // spot once the other has moved
    for _ in 0..pl.royals.len().max(1) {
        out.extend(
            away.iter()
                .chain(moves.iter())
                .map(|&(index, at)| Input::PlaceRoyal { index, at }),
        );
    }
    out
}
