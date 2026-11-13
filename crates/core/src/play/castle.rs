//! A player's castle as physics: the ledger says what is owned, the castle
//! says where each piece is. A physics `World` is rebuilt from the terrain,
//! the ruins still owned, the placed pieces and the bodies standing on them.

use std::collections::BTreeMap;

use super::tune;
use super::units::{self, GridPt, STEPS_PER_CELL};
use crate::game::flow::Player;
use crate::game::ledger::{self, Piece};
use crate::game::royals::{Plot, Spot};
use crate::physics::{self, Body, Fx, Vec2, World, TUNE};
use crate::terrain;

/// Where one ledger piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geo {
    Beam {
        a: GridPt,
        b: GridPt,
    },
    /// A building's feet.
    Building {
        at: GridPt,
    },
}

/// Who a physics body is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyOwner {
    /// Index into the player's royals.
    Royal(usize),
    /// Ledger id of a building.
    Building(u32),
}

/// A player's placed geometry, keyed by ledger id, and where royals stand.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Castle {
    pub geo: BTreeMap<u32, Geo>,
    /// Exact feet of each royal placed by hand; `None` uses the rule spot.
    pub royal_at: Vec<Option<GridPt>>,
}

/// A built castle world and what each segment and body is.
#[derive(Clone, Debug)]
pub struct CastleWorld {
    pub world: World,
    /// Ledger id of each physics segment (a long ruin may be several).
    pub seg_ids: Vec<u32>,
    pub bodies: Vec<BodyOwner>,
}

/// Where a point can stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Ground,
    /// A floor segment and where along it (0..1).
    Floor(usize, Fx),
}

pub fn physics_material(piece: Piece) -> Option<physics::Material> {
    match piece {
        Piece::Segment(ledger::Material::Wood) => Some(physics::Material::Wood),
        Piece::Segment(ledger::Material::Stone) => Some(physics::Material::Stone),
        Piece::Segment(ledger::Material::Metal) => Some(physics::Material::Metal),
        Piece::Rope => Some(physics::Material::Rope),
        Piece::Building(_) => None,
    }
}

/// Floor-like: not a rope, more across than up.
fn is_floor(w: &World, s: usize) -> bool {
    let seg = &w.segments[s];
    if seg.broken || TUNE.material(seg.material).rope {
        return false;
    }
    let d = w.nodes[seg.b].pos - w.nodes[seg.a].pos;
    d.x != Fx::ZERO && d.x.abs() >= d.y.abs()
}

/// The surface at physics point `p`: a floor whose top is within the
/// tolerance, else the ground, else nothing.
pub fn surface_at(w: &World, p: Vec2) -> Option<Surface> {
    for s in 0..w.segments.len() {
        if !is_floor(w, s) {
            continue;
        }
        let seg = &w.segments[s];
        let (pa, pb) = (w.nodes[seg.a].pos, w.nodes[seg.b].pos);
        let u = (p.x - pa.x) / (pb.x - pa.x);
        if u < Fx::ZERO || u > Fx::ONE {
            continue;
        }
        let y = pa.y + (pb.y - pa.y) * u;
        if (p.y - y).abs() <= tune::STAND_TOLERANCE {
            return Some(Surface::Floor(s, u));
        }
    }
    let g = w.terrain.height_at(p.x);
    ((p.y - g).abs() <= tune::STAND_TOLERANCE).then_some(Surface::Ground)
}

/// The highest surface at or below `p`: where a royal whose floor was sold lands.
fn surface_below(w: &World, p: Vec2) -> Surface {
    let mut best: Option<(Fx, usize, Fx)> = None;
    for s in 0..w.segments.len() {
        if !is_floor(w, s) {
            continue;
        }
        let seg = &w.segments[s];
        let (pa, pb) = (w.nodes[seg.a].pos, w.nodes[seg.b].pos);
        let u = (p.x - pa.x) / (pb.x - pa.x);
        if u < Fx::ZERO || u > Fx::ONE {
            continue;
        }
        let y = pa.y + (pb.y - pa.y) * u;
        if y <= p.y + tune::STAND_TOLERANCE && best.is_none_or(|(by, _, _)| y > by) {
            best = Some((y, s, u));
        }
    }
    match best {
        Some((_, s, u)) => Surface::Floor(s, u),
        None => Surface::Ground,
    }
}

/// Puts a body on a surface: its foot circle and its upright (`tall`).
fn stand(w: &mut World, surface: Surface, x: Fx, radius: Fx, tall: Fx) -> usize {
    let i = match surface {
        Surface::Floor(s, u) => w.add_body(s, u, radius),
        Surface::Ground => {
            let pos = Vec2::new(x, w.terrain.height_at(x) + radius);
            w.bodies.push(Body {
                pos,
                prev: pos,
                radius,
                tall: Fx::ZERO,
                attached: None,
                peak_y: pos.y,
                alive: true,
            });
            w.bodies.len() - 1
        }
    };
    w.bodies[i].tall = tall;
    i
}

/// Adds a straight beam as pieces no longer than the material allows,
/// each tagged with the ledger id.
fn add_long_beam(cw: &mut CastleWorld, a: Vec2, b: Vec2, m: physics::Material, id: u32) {
    let max = TUNE.material(m).max_len;
    let len = (b - a).len();
    let mut n = 1i64;
    while len > max * n {
        n += 1;
    }
    let mut prev = a;
    for k in 1..=n {
        let next = Vec2::lerp(a, b, Fx::ratio(k, n));
        if cw.world.add_beam(prev, next, m).is_ok() {
            cw.seg_ids.push(id);
        }
        prev = next;
    }
}

/// The rule-spot of a feet point: cells from the span's first column, and
/// cells above the ground under it, rounded to the nearest cell.
pub fn rule_spot(plot: &terrain::Plot, level: u8, at: GridPt) -> Spot {
    let span = plot.buildable(level);
    let ground = units::terrain_to_fx(plot.heights[at.col().min(plot.heights.len() - 1)]);
    let up = at.to_fx().y - ground;
    Spot {
        x: at.col() as i32 - span.start as i32,
        y: (up + Fx::HALF).floor_int().max(0) as i32,
    }
}

/// The plot as the royal rules see it, from the terrain (R1.606).
pub fn rule_plot(plot: &terrain::Plot, level: u8) -> Plot {
    Plot {
        width: plot.buildable(level).len() as i32,
        height: terrain::Plot::build_height(level) / terrain::CELL,
    }
}

impl Castle {
    /// Builds the physics world for `player`'s castle with every piece the
    /// ledger still holds, royals and buildings standing where they were put.
    /// `with_royals` false leaves royals out (the build-phase strain test).
    pub fn world(&self, player: &Player, with_royals: bool) -> CastleWorld {
        let plot = &player.terrain;
        let mut cw = CastleWorld {
            world: World::new(units::physics_terrain(plot)),
            seg_ids: Vec::new(),
            bodies: Vec::new(),
        };
        let owned = |id: u32| player.ledger.entries.iter().find(|e| e.id == id);
        for (id, seg) in &player.ruin_pieces {
            if owned(*id).is_none() {
                continue;
            }
            let a = Vec2::new(units::terrain_to_fx(seg.a.x), units::terrain_to_fx(seg.a.y));
            let b = Vec2::new(units::terrain_to_fx(seg.b.x), units::terrain_to_fx(seg.b.y));
            add_long_beam(&mut cw, a, b, physics::Material::Stone, *id);
        }
        for e in &player.ledger.entries {
            let Some(Geo::Beam { a, b }) = self.geo.get(&e.id) else {
                continue;
            };
            let Some(m) = physics_material(e.piece) else {
                continue;
            };
            add_long_beam(&mut cw, a.to_fx(), b.to_fx(), m, e.id);
        }
        // buildings: placed geometry, or the ground at the rule column (bot seats)
        for e in &player.ledger.entries {
            let Piece::Building(_) = e.piece else { continue };
            let at = match self.geo.get(&e.id) {
                Some(Geo::Building { at }) => at.to_fx(),
                _ => match e.col {
                    Some(c) => {
                        let x = Fx::from_int(c as i64) + Fx::HALF;
                        Vec2::new(x, cw.world.terrain.height_at(x))
                    }
                    None => continue,
                },
            };
            let s = surface_at(&cw.world, at).unwrap_or_else(|| surface_below(&cw.world, at));
            stand(&mut cw.world, s, at.x, tune::BUILDING_RADIUS, Fx::ZERO);
            cw.bodies.push(BodyOwner::Building(e.id));
        }
        if with_royals {
            for (i, r) in player.royals.iter().enumerate() {
                let at = match self.royal_at.get(i).copied().flatten() {
                    Some(g) => g.to_fx(),
                    None => match r.spot {
                        Some(sp) => self.spot_point(&cw.world, player, sp),
                        None => continue,
                    },
                };
                let s = surface_at(&cw.world, at).unwrap_or_else(|| surface_below(&cw.world, at));
                stand(&mut cw.world, s, at.x, tune::ROYAL_RADIUS, tune::ROYAL_UPRIGHT);
                cw.bodies.push(BodyOwner::Royal(i));
            }
        }
        cw.world.build();
        strengthen(&mut cw.world, &player.mods);
        cw
    }

    /// The feet point of a rule spot: the column's middle, at the floor
    /// whose height above the ground rounds to the spot's, else the ground.
    fn spot_point(&self, w: &World, player: &Player, sp: Spot) -> Vec2 {
        let span = player.terrain.buildable(player.purse.level);
        let col = span.start as i64 + sp.x as i64;
        let x = Fx::from_int(col) + Fx::HALF;
        let level = player.purse.level;
        for g in standing_points(w, &player.terrain, level) {
            if g.x == x && rule_spot(&player.terrain, level, to_grid(g)) == sp {
                return g;
            }
        }
        Vec2::new(x, w.terrain.height_at(x))
    }
}

/// Material strength upgrades (Seasoned Oak, Dressed Stone, Tempered Iron,
/// Citadel, rope strength) as hit points and load the physics carries.
pub fn strengthen(w: &mut World, mods: &crate::game::upgrades::Modifiers) {
    for i in 0..w.segments.len() {
        let pct = match w.segments[i].material {
            physics::Material::Wood => mods.strength(ledger::Material::Wood),
            physics::Material::Stone => mods.strength(ledger::Material::Stone),
            physics::Material::Metal => mods.strength(ledger::Material::Metal),
            physics::Material::Rope => mods.rope_pct,
        };
        if pct > 0 {
            w.strengthen(i, pct);
        }
    }
}

fn to_grid(p: Vec2) -> GridPt {
    let g = TUNE.grid.0;
    GridPt::new((p.x.0 / g) as i32, (p.y.0 / g) as i32)
}

/// Every place a royal could stand: the middle of each buildable column,
/// on the ground and on every floor crossing it.
pub fn standing_points(w: &World, plot: &terrain::Plot, level: u8) -> Vec<Vec2> {
    let span = plot.buildable(level);
    let mut out = Vec::new();
    for c in span.start..span.end {
        let x = Fx::from_int(c as i64) + Fx::HALF;
        // snap to the grid so the point is one a player could pick
        let snap = |y: Fx| Fx(y.0.div_euclid(TUNE.grid.0) * TUNE.grid.0);
        out.push(Vec2::new(x, w.terrain.height_at(x)));
        for s in 0..w.segments.len() {
            if !is_floor(w, s) {
                continue;
            }
            let seg = &w.segments[s];
            let (pa, pb) = (w.nodes[seg.a].pos, w.nodes[seg.b].pos);
            let u = (x - pa.x) / (pb.x - pa.x);
            if u < Fx::ZERO || u > Fx::ONE {
                continue;
            }
            let y = pa.y + (pb.y - pa.y) * u;
            if (snap(y) - y).abs() <= tune::STAND_TOLERANCE {
                out.push(Vec2::new(x, snap(y)));
            }
        }
    }
    out
}

/// The rule spots for every standing point, for `Match::set_standing_spots`.
pub fn standing_spots(w: &World, plot: &terrain::Plot, level: u8) -> Vec<Spot> {
    let mut v: Vec<Spot> = standing_points(w, plot, level)
        .into_iter()
        .map(|p| rule_spot(plot, level, to_grid(p)))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Can a segment go from `a` to `b` on this plot at this level? Both ends
/// inside the buildable span, not under the ground, not above the build
/// height, and no longer than the material's longest.
pub fn segment_fits(plot: &terrain::Plot, level: u8, a: GridPt, b: GridPt, m: physics::Material) -> bool {
    if a == b {
        return false;
    }
    let len = (b.to_fx() - a.to_fx()).len();
    if len > TUNE.material(m).max_len {
        return false;
    }
    point_fits(plot, level, a) && point_fits(plot, level, b)
}

/// A point inside the buildable span (its right edge included), at or
/// above the ground and at or below the build height.
pub fn point_fits(plot: &terrain::Plot, level: u8, p: GridPt) -> bool {
    let span = plot.buildable(level);
    let x0 = span.start as i32 * STEPS_PER_CELL;
    let x1 = span.end as i32 * STEPS_PER_CELL;
    if p.x < x0 || p.x > x1 {
        return false;
    }
    let ground = units::physics_terrain(plot).height_at(p.to_fx().x);
    let front = units::terrain_to_fx(plot.heights[span.start]);
    let top = front + units::terrain_to_fx(terrain::Plot::build_height(level));
    let y = p.to_fx().y;
    y + tune::STAND_TOLERANCE >= ground && y <= top
}
