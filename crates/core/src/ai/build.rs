//! Castle plans for bots: one keep of braced rooms on a site fixed for the
//! match, scaled to the level, the gold and stockpile, the buildable span
//! and height, then a royal to each room it can hold and producers put on
//! their map spots (R1.4000).
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

/// A one-room hall `bays` wide from terrain column `c` (R1.2500), its roof
/// the head room over the highest ground across `top_bays` bays, so a hall
/// that widens keeps its roof (R1.4000): a post
/// (in `post` material) at every bay edge, the roof between their tops and
/// a strut from each end post's top out to the ground, each a whole beam;
/// `None` where the ground makes a beam too long. Returns the segments and
/// which are the roof.
fn frame(
    terrain: &physics::Terrain,
    c: usize,
    bays: i64,
    top_bays: i64,
    post: Material,
) -> Option<(Vec<SegPlan>, Vec<usize>)> {
    let q = |n: i64| Fx::ratio(n, 4);
    let x = |i: i64| units::col_x(c) + Fx::from_int(tune::BAY_UNITS) * i;
    let ground = |x: Fx| Vec2::new(x, snap(terrain.height_at(x)));
    // level roof the head room over every bay's middle, on the grid; the
    // posts reach down to the ground each side, of different lengths on a
    // slope (R1.2502)
    let need = (0..bays.max(top_bays))
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
        let reaches = (1..=tune::HALL_STRUT_QUARTERS)
            .rev()
            .map(|k| k * out)
            .chain((1..=tune::HALL_STRUT_QUARTERS).map(|k| -k * out));
        let mut ok = false;
        for k in reaches {
            if push_seg(&mut segs, ground(x(end) + q(k)), at(end), post).is_some() {
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

/// A keep (R1.4000): its segments, its floors with their row (the hall's
/// roof is row `ROOM_ROWS`), and the spots a royal may stand on, each with
/// the floor under it (`None`: the ground) and whether a floor covers it.
struct Keep {
    segs: Vec<SegPlan>,
    floors: Vec<(usize, i64)>,
    spots: Vec<RoyalPlan>,
    /// The hall's roof height.
    roof: Fx,
}

/// The keep: a hall `w` bays wide on the ground (posts, roof, end struts),
/// and `f - 1` storeys of rooms on its roof (R1.4000). A storey alternates
/// braced bays (posts in two pieces, a middle beam, two diagonals) with
/// clear bays (posts and floor only), braced bays at both ends, so every
/// post is held and a royal stands in each clear bay with no beam through
/// it. Royals stand on the hall's ground in its even bays and on each
/// storey's floor in its clear bays, two apart. `None` where the hall's
/// frame does not fit the ground.
fn keep(
    terrain: &physics::Terrain,
    (col0, site_bays): (usize, i64),
    w: i64,
    f: i64,
    u: i64,
    column: Material,
) -> Option<Keep> {
    let (mut segs, roofs) =
        frame(terrain, col0, w, site_bays, column).or_else(|| frame(terrain, col0, w, w, column))?;
    let roof = segs[roofs[0]].a.y;
    let x = |i: i64| units::col_x(col0) + Fx::from_int(tune::BAY_UNITS) * i;
    let mid = |i: i64| x(i) + Fx::from_int(tune::BAY_UNITS) / 2;
    let mut floors: Vec<(usize, i64)> = roofs.iter().map(|&r| (r, tune::ROOM_ROWS)).collect();
    let mut spots: Vec<RoyalPlan> = Vec::new();
    for i in (0..w).step_by(2) {
        let at = Vec2::new(mid(i), snap(terrain.height_at(mid(i))));
        spots.push(RoyalPlan {
            segment: None,
            u: Fx::HALF,
            at,
            covered: true,
        });
    }
    // storeys: `u` bays from the hall's second bay, clear and braced in
    // turn from a clear one, so their clear bays sit over the hall's odd
    // bays, two from every royal on the ground; they stay put as the hall
    // widens
    if f > 1 && (u < 2 || u > w - 1) {
        return None;
    }
    let o = 1;
    let mut under: Vec<usize> = roofs.get(o as usize..).map(|r| r.to_vec()).unwrap_or_default();
    let row = Fx::from_int(tune::FLOOR_UNITS);
    let braced = |i: i64| i % 2 == 1;
    for s in 1..f {
        let y0 = roof + row * (tune::ROOM_ROWS * (s - 1));
        let p = |i: i64, k: i64| Vec2::new(x(o + i), y0 + row * k);
        // a post beside a braced bay is in pieces for the braces; an end
        // post of a clear bay is one beam, held by the floor
        for e in 0..=u {
            if (e >= 1 && braced(e - 1)) || (e < u && braced(e)) {
                for k in 0..tune::ROOM_ROWS {
                    push_seg(&mut segs, p(e, k), p(e, k + 1), column)?;
                }
            } else {
                push_seg(&mut segs, p(e, 0), p(e, tune::ROOM_ROWS), column)?;
            }
        }
        let mut next = Vec::new();
        for i in 0..u {
            let fl = push_seg(
                &mut segs,
                p(i, tune::ROOM_ROWS),
                p(i + 1, tune::ROOM_ROWS),
                Material::Wood,
            )?;
            next.push(fl);
            floors.push((fl, tune::ROOM_ROWS * (s + 1)));
            if braced(i) {
                for k in 1..tune::ROOM_ROWS {
                    push_seg(&mut segs, p(i, k), p(i + 1, k), Material::Wood)?;
                }
                for k in 0..tune::ROOM_ROWS {
                    let (a, b) = if (i / 2 + k) % 2 == 0 {
                        (p(i, k), p(i + 1, k + 1))
                    } else {
                        (p(i + 1, k), p(i, k + 1))
                    };
                    push_seg(&mut segs, a, b, Material::Wood)?;
                }
            } else {
                spots.push(RoyalPlan {
                    segment: under.get(i as usize).copied(),
                    u: Fx::HALF,
                    at: Vec2::new(mid(o + i), y0),
                    covered: true,
                });
            }
        }
        under = next;
    }
    // the highest spots first, for the most valuable royals
    spots.sort_by_key(|r| std::cmp::Reverse(r.at.y));
    Some(Keep {
        segs,
        floors,
        spots,
        roof,
    })
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

/// Where the keep stands for the whole match (R1.4000), and how wide its
/// hall can grow: the column, past the setback, where the widest hall frame
/// (up to `KEEP_MAX_BAYS`) fits the ground, nearest the front among equals.
/// Found on the first level's span, so the site never moves as the plot
/// widens, and the hall's roof is set for its widest, so it never moves as
/// the hall grows.
fn keep_site(plot: &Plot, setback: usize) -> (usize, i64) {
    let span = plot.buildable(1);
    // behind the old ruins at the plot's front, so no plan lays a segment
    // over one (R1.906, R1.5001)
    let behind_ruins = plot
        .spots
        .iter()
        .filter(|s| !s.segments.is_empty())
        .map(|s| s.cols.end)
        .max()
        .unwrap_or(span.start);
    let first = (span.start + setback).max(behind_ruins);
    let terrain = units::physics_terrain(plot);
    let widest = |c: usize| {
        (1..=tune::KEEP_MAX_BAYS)
            .rev()
            .find(|&w| c + w as usize + 2 <= span.end && frame(&terrain, c, w, w, Material::Wood).is_some())
            .unwrap_or(0)
    };
    (first..span.end)
        .map(|c| (widest(c), c))
        .max_by_key(|&(w, c)| (w, usize::MAX - c))
        .map(|(w, c)| (c, w.max(1)))
        .unwrap_or((first, 1))
}

/// Segments of `segs` in a material the level has not unlocked fit the
/// stockpile (the ledger places those from stock only).
fn unlocked_or_stocked(segs: &[SegPlan], level: u8, stock: Stock, have: &[Have]) -> bool {
    let top = column_material(level);
    let mut left = stock;
    segs.iter()
        .filter(|s| s.material > top && !have.iter().any(|h| same(s, &h.seg)))
        .all(|s| {
            let n = left.get_mut(s.material);
            let ok = *n > 0;
            *n = n.saturating_sub(1);
            ok
        })
}

/// The bot's castle plan: one keep of braced rooms (R1.4000) on a site fixed
/// for the match, as many storeys and bays as the gold and stockpile allow,
/// a royal to a room. Every royal's room is its own bay of a storey, so the
/// spacing keeps them in separate rooms.
pub fn plan_castle(ask: &BuildAsk) -> CastlePlan {
    let level = ask.level.clamp(1, 9);
    let span = ask.plot.buildable(level);
    let mut gold = ask
        .gold
        .saturating_sub(tune::reserve_gold(ask.difficulty, ask.round));
    // producers first: they pay every round. Each keeps back its own price
    // and a one-bay wooden ground floor, in case no keep floor covers its
    // spot, before the keep is sized (R1.4700)
    let floor = ledger::price(Piece::Segment(Material::Wood), &Modifiers::default());
    let mut want = Vec::new();
    let mut producers = 0;
    for &k in ask.buildings {
        let row = ledger::building_row(k);
        let need = row.cost + floor;
        if row.level <= level && need <= gold {
            gold -= need;
            producers += row.cost;
            want.push(k);
        }
    }
    let max_rows = tune::MAX_FLOORS[level as usize - 1]
        .min(Plot::build_height(level) as i64 / crate::terrain::CELL as i64 - 1);
    let (col0, site_bays) = keep_site(ask.plot, tune::FRONT_SETBACK);
    let max_w = (1..=tune::KEEP_MAX_BAYS)
        .rev()
        .find(|&w| w <= site_bays && col0 + w as usize + 1 < span.end)
        .unwrap_or(1);
    // column materials: what the level has unlocked, and stronger ones the
    // stockpile holds
    let mats: Vec<Material> = {
        let top = column_material(level);
        let mut st = ask.stock;
        [Material::Metal, Material::Stone, Material::Wood]
            .into_iter()
            .filter(|m| *m <= top || *st.get_mut(*m) > 0)
            .collect()
    };
    let terrain = units::physics_terrain(ask.plot);
    // the best few plans with each column material, strongest first
    type Cand = (u64, Vec<SegPlan>, Vec<(usize, i64)>, Stock, Vec<RoyalPlan>);
    let mut best: [Vec<Cand>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    // the gold producers leave, then, while a royal is out of a room, all of it
    for budget in [gold, ask.gold] {
        for f in (1..=max_rows / tune::ROOM_ROWS).rev() {
            for (w, u) in (1..=max_w)
                .rev()
                .flat_map(|w| (1..w.max(2)).rev().map(move |u| (w, u)))
            {
                if (f > 1) != (u >= 2) {
                    continue;
                }
                for &m in &mats {
                    let Some(Keep {
                        mut segs,
                        floors,
                        spots,
                        roof,
                    }) = keep(&terrain, (col0, site_bays), w, f, u, m)
                    else {
                        continue;
                    };
                    let (x0, x1) = (units::col_x(col0), units::col_x(col0 + w as usize));
                    // never a narrower hall than the one standing: no roof
                    // beam of the castle's past this hall's end
                    let wider = ask
                        .have
                        .iter()
                        .any(|h| h.seg.a.y == roof && h.seg.b.y == roof && h.seg.a.x.min(h.seg.b.x) >= x1);
                    if wider {
                        continue;
                    }
                    // a piece standing where the plan has one, in a material
                    // at least as strong, stays (stockpiled stone kept)
                    for s in segs.iter_mut() {
                        if let Some(h) = ask.have.iter().find(|h| {
                            h.seg.material >= s.material
                                && ((h.seg.a == s.a && h.seg.b == s.b) || (h.seg.a == s.b && h.seg.b == s.a))
                        }) {
                            *s = h.seg;
                        }
                    }
                    // a slanted piece under the hall's roof: an old end strut
                    let inside = |s: &SegPlan| {
                        s.a.x != s.b.x
                            && s.a.y != s.b.y
                            && [s.a, s.b].iter().all(|p| x0 <= p.x && p.x <= x1 && p.y <= roof)
                    };
                    // never smaller than the castle standing: a segment the
                    // plan has no place for stays (R1.2302)
                    for h in ask.have {
                        let at = |s: &SegPlan| {
                            (s.a == h.seg.a && s.b == h.seg.b) || (s.a == h.seg.b && s.b == h.seg.a)
                        };
                        // except inside the keep, where it is in the way (a
                        // widened hall's old end strut)
                        if !segs.iter().any(at) && !inside(&h.seg) {
                            segs.push(h.seg);
                        }
                    }
                    let fits = segs.iter().all(|s| {
                        [s.a, s.b]
                            .iter()
                            .all(|p| crate::play::castle::point_fits(ask.plot, level, units::grid_pt(*p)))
                    });
                    if !fits || !unlocked_or_stocked(&segs, level, ask.stock, ask.have) {
                        continue;
                    }
                    let Some((_, used)) = cost(&segs, budget, ask.stock, ask.have) else {
                        continue;
                    };
                    let mut royals: Vec<RoyalPlan> = Vec::new();
                    for r in spots.iter().filter(|r| roofed(&segs, r.at)) {
                        if royals.len() < ask.royals && apart(ask.plot, r.at, &royals) {
                            royals.push(RoyalPlan { covered: true, ..*r });
                        }
                    }
                    // royals in rooms first, then spaced, then worth
                    // (stronger material wins), then pieces
                    let worth = cost(&segs, u32::MAX, Stock::default(), &[]).map_or(0, |c| c.0);
                    let (spaced, covered) = spaced(ask.plot, &royals);
                    let size = ((covered as u64) << 44)
                        + ((spaced as u64) << 40)
                        + ((worth as u64) << 20)
                        + segs.len() as u64;
                    let slot = &mut best[mats.iter().position(|x| *x == m).unwrap_or(0)];
                    if slot.len() < tune::KEEP_CANDIDATES || slot.last().is_some_and(|b| size > b.0) {
                        let at = slot.partition_point(|b| b.0 >= size);
                        slot.insert(at, (size, segs, floors, used, royals));
                        slot.truncate(tune::KEEP_CANDIDATES);
                    }
                }
            }
        }
        if best
            .iter()
            .flat_map(|b| b.first())
            .any(|b| (b.0 >> 44) as usize >= ask.royals)
        {
            break;
        }
    }
    // the strongest material whose castle is worth at least half the best,
    // and stands under the static test; else the next best (R1.4002)
    let mut pick: Option<Cand> = None;
    while pick.is_none() {
        let top = best.iter().flat_map(|b| b.first()).map(|b| b.0).max();
        let Some(top) = top else {
            break;
        };
        let Some(i) = (0..best.len()).find(|&i| {
            best[i].first().is_some_and(|b| {
                b.0 >> 40 == top >> 40
                    && b.0 * tune::STRONGER_SHARE.1 as u64 >= top * tune::STRONGER_SHARE.0 as u64
            })
        }) else {
            break;
        };
        let c = best[i].remove(0);
        let plan = CastlePlan {
            segments: c.1.clone(),
            ..CastlePlan::default()
        };
        let (w, _) = to_world(&plan, ask.plot);
        if w.static_test(physics::TUNE.settle_ticks + tune::KEEP_TEST_TICKS)
            .broken
            .is_empty()
        {
            pick = Some(c);
        }
    }
    let (_, mut segments, floors, stock, royals) = pick.unwrap_or_default();
    // producers with the gold the keep left
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
    let kept = segments.len();
    let mut buildings = place_buildings(ask.plot, level, &want, &mut segments, &floors);
    // a producer's own ground floor costs too: drop the last producers (and
    // the floors laid for them) until the plan fits the gold (R1.4305)
    let price = |segs: &[SegPlan], bs: &[BuildingPlan]| {
        let (spent, stock) = cost(segs, u32::MAX, ask.stock, ask.have).unwrap_or((0, stock));
        (
            spent + bs.iter().map(|b| ledger::building_row(b.kind).cost).sum::<u32>(),
            stock,
        )
    };
    while price(&segments, &buildings).0 > ask.gold {
        let Some(b) = buildings.pop() else { break };
        if b.segment >= kept && b.segment + 1 == segments.len() {
            segments.pop();
        }
    }
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
    // the match also measures between feet, as the build view draws it
    let near = others.iter().any(|r| (r.at - at).len() < units::royal_spacing());
    crate::game::royals::spaced(spot(at), &taken) && !near
}

/// A floor passes over `at` at least a hut's height above it (R1.2500): a
/// level segment strictly across its x, so the royal's figure fits under it.
fn roofed(segs: &[SegPlan], at: Vec2) -> bool {
    let room = Fx::ratio(tune::HUT_ROOM_EIGHTHS, 8);
    segs.iter()
        .any(|s| s.a.y == s.b.y && s.a.x.min(s.b.x) < at.x && at.x < s.a.x.max(s.b.x) && s.a.y - at.y >= room)
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
                    died: None,
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
