//! The castle: nodes on a snap grid, segments between them, joints that hold
//! the angle between segments, and the solver that steps it all.
//!
//! The solver is position-based Verlet. Each tick every segment pulls its two
//! nodes back to its rest length and every joint turns its two segments back
//! to their rest angle; the corrections a constraint makes in one tick are
//! its load. Load over strength is strain; smoothed strain over 1 breaks it.

use super::balls::Ball;
use super::bodies::Body;
use super::fixed::{Fx, Vec2};
use super::tune::{Material, TUNE};

/// Ground as a heightfield: `heights[i]` is the ground at `x = i * col_width`.
#[derive(Clone, Debug)]
pub struct Terrain {
    pub col_width: Fx,
    pub heights: Vec<Fx>,
}

impl Terrain {
    pub fn flat(cols: usize, height: Fx) -> Terrain {
        Terrain {
            col_width: Fx::ONE,
            heights: vec![height; cols],
        }
    }

    /// Ground height at `x`, straight lines between columns, level past the ends.
    pub fn height_at(&self, x: Fx) -> Fx {
        let n = self.heights.len();
        if n == 0 {
            return Fx::ZERO;
        }
        if n == 1 || self.col_width <= Fx::ZERO {
            return self.heights[0];
        }
        let f = x / self.col_width;
        let i = f.floor_int();
        if i < 0 {
            return self.heights[0];
        }
        let i = i as usize;
        if i >= n - 1 {
            return self.heights[n - 1];
        }
        let t = Fx(f.0 & ((1 << 16) - 1));
        Fx::lerp(self.heights[i], self.heights[i + 1], t)
    }

    pub fn width(&self) -> Fx {
        self.col_width * (self.heights.len() as i64)
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub pos: Vec2,
    pub prev: Vec2,
    pub mass: Fx,
    pub inv_mass: Fx,
    /// Resting on the ground: never moves.
    pub anchored: bool,
}

impl Node {
    pub fn velocity(&self) -> Vec2 {
        self.pos - self.prev
    }

    fn moves(&self) -> bool {
        !self.anchored && self.mass > Fx::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakCause {
    Strain,
    Impact,
    Fire,
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub a: usize,
    pub b: usize,
    pub material: Material,
    pub rest: Fx,
    pub hp: Fx,
    pub broken: bool,
    /// Ticks of fire left; 0 is not burning.
    pub burn_ticks: u32,
    /// Smoothed strain: 1 is the breaking point.
    pub strain: Fx,
    pub max_strain: Fx,
    pub axial: Fx,
    pub bend: Fx,
}

impl Segment {
    pub fn mass(&self) -> Fx {
        TUNE.material(self.material).mass_per_unit * self.rest
    }

    pub fn burning(&self) -> bool {
        self.burn_ticks > 0
    }
}

/// Holds the angle between two non-rope segments meeting at node `j`.
#[derive(Clone, Debug)]
struct Joint {
    j: usize,
    p: usize,
    q: usize,
    sa: usize,
    sb: usize,
    rest_cos: Fx,
    rest_sin: Fx,
    /// Signed sum of this tick's corrections, and the arm lengths.
    acc: Fx,
    la: Fx,
    lb: Fx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    SameNode,
    TooLong,
    Duplicate,
    NoSuchNode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Broke { segment: usize, cause: BreakCause },
    BodyHit { body: usize, projectile: usize },
    BodyCrushed { body: usize },
    BodyFell { body: usize, drop: Fx },
}

/// What a recent impact threw up, for the page's dust and chips (R1.3608).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactKind {
    Dust,
    Wood,
    Stone,
    Metal,
}

/// A recent impact: where, on which tick, and what it threw up. Kept for
/// `IMPACT_LIFE` ticks; it changes nothing in the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Impact {
    pub pos: Vec2,
    pub tick: u32,
    pub kind: ImpactKind,
}

/// How many ticks an impact is kept for drawing.
pub const IMPACT_LIFE: u32 = 36;

#[derive(Clone, Debug)]
pub struct World {
    pub terrain: Terrain,
    pub nodes: Vec<Node>,
    pub segments: Vec<Segment>,
    joints: Vec<Joint>,
    pub(super) node_segs: Vec<Vec<usize>>,
    built: bool,
    /// Ticks left settling after a build: strain is measured but breaks nothing.
    pub settle_left: u32,
    pub projectiles: Vec<Ball>,
    pub debris: Vec<Ball>,
    pub bodies: Vec<Body>,
    /// Horizontal acceleration on shots, units per tick squared.
    pub wind: Fx,
    pub tick: u32,
    /// Recent impacts, newest last, for drawing only.
    pub impacts: Vec<Impact>,
}

/// What a static test found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticReport {
    pub broken: Vec<usize>,
    pub max_strain: Vec<Fx>,
}

pub fn snap(v: Fx) -> Fx {
    let g = TUNE.grid.0;
    Fx((v.0 + g / 2).div_euclid(g) * g)
}

impl World {
    pub fn new(terrain: Terrain) -> World {
        World {
            terrain,
            nodes: Vec::new(),
            segments: Vec::new(),
            joints: Vec::new(),
            node_segs: Vec::new(),
            built: false,
            settle_left: 0,
            projectiles: Vec::new(),
            debris: Vec::new(),
            bodies: Vec::new(),
            wind: Fx::ZERO,
            tick: 0,
            impacts: Vec::new(),
        }
    }

    /// Adds a node at the nearest grid point, or returns the node already there.
    pub fn add_node(&mut self, at: Vec2) -> usize {
        let p = Vec2::new(snap(at.x), snap(at.y));
        if let Some(i) = self.nodes.iter().position(|n| n.pos == p) {
            return i;
        }
        self.nodes.push(Node {
            pos: p,
            prev: p,
            mass: Fx::ZERO,
            inv_mass: Fx::ZERO,
            anchored: false,
        });
        self.built = false;
        self.nodes.len() - 1
    }

    pub fn add_segment(&mut self, a: usize, b: usize, material: Material) -> Result<usize, BuildError> {
        if a >= self.nodes.len() || b >= self.nodes.len() {
            return Err(BuildError::NoSuchNode);
        }
        if a == b {
            return Err(BuildError::SameNode);
        }
        if self
            .segments
            .iter()
            .any(|s| (s.a == a && s.b == b) || (s.a == b && s.b == a))
        {
            return Err(BuildError::Duplicate);
        }
        let rest = (self.nodes[b].pos - self.nodes[a].pos).len();
        let m = TUNE.material(material);
        if rest > m.max_len {
            return Err(BuildError::TooLong);
        }
        self.segments.push(Segment {
            a,
            b,
            material,
            rest,
            hp: m.hp,
            broken: false,
            burn_ticks: 0,
            strain: Fx::ZERO,
            max_strain: Fx::ZERO,
            axial: Fx::ZERO,
            bend: Fx::ZERO,
        });
        self.built = false;
        Ok(self.segments.len() - 1)
    }

    /// Adds two nodes (snapped) and a segment between them.
    pub fn add_beam(&mut self, from: Vec2, to: Vec2, material: Material) -> Result<usize, BuildError> {
        let a = self.add_node(from);
        let b = self.add_node(to);
        self.add_segment(a, b, material)
    }

    /// Masses, anchors and joints, worked out from the build. Runs on the
    /// first step after any change.
    pub fn build(&mut self) {
        for n in self.nodes.iter_mut() {
            n.mass = Fx::ZERO;
        }
        self.node_segs = vec![Vec::new(); self.nodes.len()];
        for (i, s) in self.segments.iter().enumerate() {
            if s.broken {
                continue;
            }
            let half = s.mass() / 2;
            self.nodes[s.a].mass += half;
            self.nodes[s.b].mass += half;
            self.node_segs[s.a].push(i);
            self.node_segs[s.b].push(i);
        }
        let tol = TUNE.grid / 2;
        for n in self.nodes.iter_mut() {
            n.anchored = n.pos.y <= self.terrain.height_at(n.pos.x) + tol;
            n.inv_mass = if n.anchored || n.mass == Fx::ZERO {
                Fx::ZERO
            } else {
                Fx::ONE / n.mass
            };
        }
        self.joints.clear();
        for j in 0..self.nodes.len() {
            let solid: Vec<usize> = self.node_segs[j]
                .iter()
                .copied()
                .filter(|&s| !TUNE.material(self.segments[s].material).rope)
                .collect();
            for w in solid.windows(2) {
                let (sa, sb) = (w[0], w[1]);
                let p = self.other_end(sa, j);
                let q = self.other_end(sb, j);
                let da = (self.nodes[p].pos - self.nodes[j].pos).normalized();
                let db = (self.nodes[q].pos - self.nodes[j].pos).normalized();
                self.joints.push(Joint {
                    j,
                    p,
                    q,
                    sa,
                    sb,
                    rest_cos: da.dot(db),
                    rest_sin: da.cross(db),
                    acc: Fx::ZERO,
                    la: Fx::ONE,
                    lb: Fx::ONE,
                });
            }
        }
        self.built = true;
        self.settle_left = TUNE.settle_ticks;
    }

    fn other_end(&self, s: usize, j: usize) -> usize {
        let seg = &self.segments[s];
        if seg.a == j {
            seg.b
        } else {
            seg.a
        }
    }

    /// One tick. Returns what happened, in a fixed order.
    pub fn step(&mut self) -> Vec<Event> {
        if !self.built {
            self.build();
        }
        let mut events = Vec::new();
        self.integrate();
        for s in self.segments.iter_mut() {
            s.axial = Fx::ZERO;
            s.bend = Fx::ZERO;
        }
        for jt in self.joints.iter_mut() {
            jt.acc = Fx::ZERO;
        }
        for _ in 0..TUNE.iterations {
            self.solve_segments();
            self.solve_joints();
        }
        for jt in self.joints.iter() {
            if jt.acc != Fx::ZERO {
                let mag = jt.acc.abs();
                self.segments[jt.sa].bend += mag / jt.la;
                self.segments[jt.sb].bend += mag / jt.lb;
            }
        }
        self.ground_nodes();
        let mut to_break = Vec::new();
        self.step_balls(&mut to_break, &mut events);
        self.step_fire(&mut to_break);
        self.measure_strain(&mut to_break);
        for (s, cause) in to_break {
            self.finish_break(s, cause, &mut events);
        }
        self.step_bodies(&mut events);
        let now = self.tick;
        self.impacts.retain(|m| m.tick + IMPACT_LIFE > now);
        self.tick += 1;
        events
    }

    fn integrate(&mut self) {
        let g = TUNE.gravity;
        for n in self.nodes.iter_mut() {
            if !n.moves() {
                n.prev = n.pos;
                continue;
            }
            let v = (n.pos - n.prev) * TUNE.damping;
            n.prev = n.pos;
            n.pos += v;
            n.pos.y -= g;
        }
    }

    fn solve_segments(&mut self) {
        for s in self.segments.iter_mut() {
            if s.broken {
                continue;
            }
            let (na, nb) = (&self.nodes[s.a], &self.nodes[s.b]);
            let (wa, wb) = (na.inv_mass, nb.inv_mass);
            let wsum = wa + wb;
            if wsum == Fx::ZERO {
                continue;
            }
            let d = nb.pos - na.pos;
            let len = d.len();
            if len == Fx::ZERO {
                continue;
            }
            let c = len - s.rest;
            if c <= Fx::ZERO && TUNE.material(s.material).rope {
                continue;
            }
            let delta = c / wsum;
            let n = Vec2::new(d.x / len, d.y / len);
            self.nodes[s.a].pos += n * (delta * wa);
            self.nodes[s.b].pos -= n * (delta * wb);
            s.axial += delta;
        }
    }

    fn solve_joints(&mut self) {
        let k = TUNE.joint_stiffness;
        for jt in self.joints.iter_mut() {
            if self.segments[jt.sa].broken || self.segments[jt.sb].broken {
                continue;
            }
            let (pj, pp, pq) = (self.nodes[jt.j].pos, self.nodes[jt.p].pos, self.nodes[jt.q].pos);
            let (dp, dq) = (pp - pj, pq - pj);
            let (la, lb) = (dp.len(), dq.len());
            if la == Fx::ZERO || lb == Fx::ZERO {
                continue;
            }
            let da = Vec2::new(dp.x / la, dp.y / la);
            let db = Vec2::new(dq.x / lb, dq.y / lb);
            let target = da.rotate(jt.rest_cos, jt.rest_sin);
            let mut c = target.cross(db);
            if target.dot(db) < Fx::ZERO {
                c = if c < Fx::ZERO { -Fx::ONE } else { Fx::ONE };
            }
            if c == Fx::ZERO {
                continue;
            }
            let gq = db.perp() * (Fx::ONE / lb);
            let gp = -(da.perp() * (Fx::ONE / la));
            let gj = -(gq + gp);
            let (wj, wp, wq) = (
                self.nodes[jt.j].inv_mass,
                self.nodes[jt.p].inv_mass,
                self.nodes[jt.q].inv_mass,
            );
            let denom = wq * gq.len2() + wp * gp.len2() + wj * gj.len2();
            if denom == Fx::ZERO {
                continue;
            }
            let lambda = -(c * k) / denom;
            self.nodes[jt.q].pos += gq * (wq * lambda);
            self.nodes[jt.p].pos += gp * (wp * lambda);
            self.nodes[jt.j].pos += gj * (wj * lambda);
            jt.acc += lambda;
            jt.la = la;
            jt.lb = lb;
        }
    }

    fn ground_nodes(&mut self) {
        for n in self.nodes.iter_mut() {
            if !n.moves() {
                continue;
            }
            let h = self.terrain.height_at(n.pos.x);
            if n.pos.y < h {
                n.pos.y = h;
                n.prev.y = h;
                n.prev.x = n.pos.x - (n.pos.x - n.prev.x) * TUNE.ground_friction;
            }
        }
    }

    fn measure_strain(&mut self, to_break: &mut Vec<(usize, BreakCause)>) {
        let smooth = TUNE.strain_smoothing;
        let settling = self.settle_left > 0;
        if settling {
            self.settle_left -= 1;
        }
        for (i, s) in self.segments.iter_mut().enumerate() {
            if s.broken {
                continue;
            }
            let m = TUNE.material(s.material);
            let health = s.hp / m.hp;
            let now = if health <= Fx::ZERO {
                Fx::from_int(1000)
            } else {
                let pull = s.axial.max(Fx::ZERO) / m.tension;
                let push = (-s.axial).max(Fx::ZERO) / m.compression;
                (pull + push + s.bend / m.bending) / health
            };
            s.strain = (s.strain * (smooth - 1) + now) / smooth;
            if settling {
                continue;
            }
            if s.strain > s.max_strain {
                s.max_strain = s.strain;
            }
            if s.strain > Fx::ONE {
                s.broken = true;
                to_break.push((i, BreakCause::Strain));
            }
        }
    }

    /// Marks a segment broken now; `finish_break` does the rest after the
    /// pass that found it. Returns false when it was already broken.
    pub(super) fn mark_broken(segments: &mut [Segment], s: usize) -> bool {
        if segments[s].broken {
            return false;
        }
        segments[s].broken = true;
        true
    }

    fn finish_break(&mut self, s: usize, cause: BreakCause, events: &mut Vec<Event>) {
        let (a, b, mass) = {
            let seg = &self.segments[s];
            (seg.a, seg.b, seg.mass())
        };
        let half = mass / 2;
        for n in [a, b] {
            let node = &mut self.nodes[n];
            node.mass = (node.mass - half).max(Fx::ZERO);
            node.inv_mass = if node.anchored || node.mass == Fx::ZERO {
                Fx::ZERO
            } else {
                Fx::ONE / node.mass
            };
        }
        let seg = &self.segments[s];
        let kind = match seg.material {
            Material::Wood => Some(ImpactKind::Wood),
            Material::Stone => Some(ImpactKind::Stone),
            Material::Metal => Some(ImpactKind::Metal),
            Material::Rope => None,
        };
        if let Some(kind) = kind {
            let pos = (self.nodes[seg.a].pos + self.nodes[seg.b].pos) * Fx::HALF;
            self.impacts.push(Impact {
                pos,
                tick: self.tick,
                kind,
            });
        }
        self.spawn_debris(s);
        events.push(Event::Broke { segment: s, cause });
    }

    /// Steps a copy of the world `ticks` times with no shots in the air and
    /// reports which segments broke and each segment's highest strain.
    pub fn static_test(&self, ticks: u32) -> StaticReport {
        let mut w = self.clone();
        w.projectiles.clear();
        w.debris.clear();
        let before: Vec<bool> = w.segments.iter().map(|s| s.broken).collect();
        for _ in 0..ticks {
            w.step();
        }
        let broken = w
            .segments
            .iter()
            .enumerate()
            .filter(|(i, s)| s.broken && !before[*i])
            .map(|(i, _)| i)
            .collect();
        StaticReport {
            broken,
            max_strain: w.segments.iter().map(|s| s.max_strain).collect(),
        }
    }

    /// A fingerprint of the whole state, for determinism checks.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: i64| {
            for byte in v.to_le_bytes() {
                h ^= byte as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for n in &self.nodes {
            mix(n.pos.x.0);
            mix(n.pos.y.0);
            mix(n.prev.x.0);
            mix(n.prev.y.0);
        }
        for s in &self.segments {
            mix(s.broken as i64);
            mix(s.hp.0);
            mix(s.strain.0);
            mix(s.burn_ticks as i64);
        }
        for b in self.projectiles.iter().chain(self.debris.iter()) {
            mix(b.pos.x.0);
            mix(b.pos.y.0);
            mix(b.alive as i64);
        }
        for b in &self.bodies {
            mix(b.pos.x.0);
            mix(b.pos.y.0);
            mix(b.alive as i64);
        }
        mix(self.tick as i64);
        h
    }
}
