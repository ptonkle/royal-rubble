//! Ammo kinds as data, and what plain shots lack: blasts, spreads and
//! strength bonuses. The match layer chooses a `ShotKind` and `ShotMods`
//! from a player's loadout and upgrades; this file turns them into balls.
//!
//! - `fire_shot` puts one shot (or a spread) in the air.
//! - An exploding shot comes back with a `Fuse`; call `step_fuses` after each
//!   `World::step`. A fuse goes off when its shot touches a beam, the ground
//!   or a body, or stops: it wears down every beam in reach, pushes its
//!   nodes outward and kills every body in reach.
//! - `strengthen` raises a segment's hit points by a percent, which also
//!   lowers its strain (strain divides by health, so +25% carries 25% more).

use super::fixed::{cos, sin, Fx, Vec2, TURN};
use super::trebuchet::Launch;
use super::tune::TUNE;
use super::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShotKind {
    Normal,
    Fire,
    Exploding,
    Heavy,
    Spread,
    Chain,
    Meteor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotTune {
    pub radius: Fx,
    pub mass: Fx,
    /// Sets wood alight on contact.
    pub ignites: bool,
    /// Blast reach; zero means no blast.
    pub blast_radius: Fx,
    /// Hit points a beam at the center loses; falls off to 0 at the edge.
    pub blast_damage: Fx,
    /// Speed a node at the center gains, outward, units per tick.
    pub blast_push: Fx,
    /// Balls per shot (a spread fans them out).
    pub pieces: u32,
    /// Angle between spread pieces, in brads.
    pub spread: i64,
}

const NORMAL: ShotTune = ShotTune {
    radius: Fx::ratio(1, 2),
    mass: Fx::from_int(6),
    ignites: false,
    blast_radius: Fx::ZERO,
    blast_damage: Fx::ZERO,
    blast_push: Fx::ZERO,
    pieces: 1,
    spread: 0,
};

/// Every ammo kind's numbers. Meteor is twice the size and three times the
/// weight of a normal shot, and burns (data/upgrades.draft.json).
pub const SHOTS: [(ShotKind, ShotTune); 7] = [
    (ShotKind::Normal, NORMAL),
    (
        ShotKind::Fire,
        ShotTune {
            ignites: true,
            ..NORMAL
        },
    ),
    (
        ShotKind::Exploding,
        ShotTune {
            blast_radius: Fx::ratio(3, 2),
            blast_damage: Fx::from_int(2),
            blast_push: Fx::ratio(1, 4),
            ..NORMAL
        },
    ),
    (
        ShotKind::Heavy,
        ShotTune {
            radius: Fx::ratio(3, 4),
            mass: Fx::from_int(15),
            ..NORMAL
        },
    ),
    (
        ShotKind::Spread,
        ShotTune {
            radius: Fx::ratio(1, 4),
            mass: Fx::from_int(2),
            pieces: 5,
            spread: TURN / 72,
            ..NORMAL
        },
    ),
    (ShotKind::Chain, NORMAL),
    (
        ShotKind::Meteor,
        ShotTune {
            radius: Fx::from_int(1),
            mass: Fx::from_int(18),
            ignites: true,
            ..NORMAL
        },
    ),
];

pub fn shot_tune(kind: ShotKind) -> ShotTune {
    SHOTS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, t)| *t)
        .unwrap_or(NORMAL)
}

/// A player's upgrades as they touch one shot. Percents are bonuses (0 none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShotMods {
    pub weight_pct: u32,
    pub blast_pct: u32,
    /// Every shot sets wood alight.
    pub ignite: bool,
}

/// A live exploding shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuse {
    pub projectile: usize,
    pub radius: Fx,
    pub damage: Fx,
    pub push: Fx,
}

/// What one blast did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blast {
    pub at: Vec2,
    pub radius: Fx,
    /// Beams it wore down.
    pub segments: Vec<usize>,
    /// Bodies it killed.
    pub bodies: Vec<usize>,
}

/// A shot just fired: its balls, and a fuse if it explodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub balls: Vec<usize>,
    pub fuse: Option<Fuse>,
}

fn pct(v: Fx, bonus: u32) -> Fx {
    v * (100 + bonus as i64) / 100
}

/// How close counts as touching for a fuse.
const FUSE_SLACK: Fx = Fx::ratio(1, 64);

impl World {
    /// Fires one shot of `kind` with a player's `mods`.
    pub fn fire_shot(&mut self, launch: Launch, kind: ShotKind, mods: ShotMods) -> Shot {
        let t = shot_tune(kind);
        let mass = pct(t.mass, mods.weight_pct);
        let n = t.pieces.max(1) as i64;
        let mut balls = Vec::new();
        for k in 0..n {
            let turn = t.spread * (2 * k - (n - 1)) / 2;
            let vel = if turn == 0 {
                launch.vel
            } else {
                launch.vel.rotate(cos(turn), sin(turn))
            };
            let i = self.fire(Launch { pos: launch.pos, vel }, t.radius, mass);
            self.projectiles[i].fire = t.ignites || mods.ignite;
            balls.push(i);
        }
        let fuse = (t.blast_radius > Fx::ZERO).then(|| Fuse {
            projectile: balls[0],
            radius: pct(t.blast_radius, mods.blast_pct),
            damage: t.blast_damage,
            push: t.blast_push,
        });
        Shot { balls, fuse }
    }

    fn fuse_touches(&self, f: &Fuse) -> bool {
        let Some(b) = self.projectiles.get(f.projectile) else {
            return true;
        };
        if !b.alive || b.pos.y - b.radius <= self.terrain.height_at(b.pos.x) + FUSE_SLACK {
            return true;
        }
        let hits_beam = self.segments.iter().any(|s| {
            let m = TUNE.material(s.material);
            if s.broken || m.rope {
                return false;
            }
            let d = dist_to_segment(b.pos, self.nodes[s.a].pos, self.nodes[s.b].pos);
            d <= b.radius + m.thickness / 2 + FUSE_SLACK
        });
        hits_beam
            || self
                .bodies
                .iter()
                .any(|body| body.alive && body.touches(b.pos, b.radius + FUSE_SLACK))
    }

    /// Sets off every fuse whose shot touched something this tick; keeps the
    /// rest. Beams worn to 0 break on the next step.
    pub fn step_fuses(&mut self, fuses: &mut Vec<Fuse>) -> Vec<Blast> {
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for f in fuses.drain(..) {
            if self.fuse_touches(&f) {
                if let Some(at) = self.projectiles.get(f.projectile).map(|b| b.pos) {
                    self.projectiles[f.projectile].alive = false;
                    out.push(self.explode(at, f.radius, f.damage, f.push));
                }
            } else {
                keep.push(f);
            }
        }
        *fuses = keep;
        out
    }

    /// A blast at `at`: damage and push fall off linearly to 0 at `radius`.
    pub fn explode(&mut self, at: Vec2, radius: Fx, damage: Fx, push: Fx) -> Blast {
        let mut blast = Blast {
            at,
            radius,
            segments: Vec::new(),
            bodies: Vec::new(),
        };
        if radius <= Fx::ZERO {
            return blast;
        }
        for si in 0..self.segments.len() {
            let (a, b) = {
                let s = &self.segments[si];
                if s.broken || TUNE.material(s.material).rope {
                    continue;
                }
                (s.a, s.b)
            };
            let d = dist_to_segment(at, self.nodes[a].pos, self.nodes[b].pos);
            if d >= radius {
                continue;
            }
            let share = (radius - d) / radius;
            let seg = &mut self.segments[si];
            seg.hp = (seg.hp - damage * share).max(Fx::ZERO);
            blast.segments.push(si);
            for n in [a, b] {
                let node = &mut self.nodes[n];
                if node.anchored || node.inv_mass == Fx::ZERO {
                    continue;
                }
                let dir = (node.pos - at).normalized();
                node.prev -= dir * (push * share);
            }
        }
        for (bi, body) in self.bodies.iter_mut().enumerate() {
            if body.alive && body.touches(at, radius) {
                body.alive = false;
                blast.bodies.push(bi);
            }
        }
        blast
    }

    /// Sets segment `s` to its material's hit points plus `bonus` percent.
    /// Call it after building and before any shot lands.
    pub fn strengthen(&mut self, s: usize, bonus: u32) {
        if let Some(seg) = self.segments.get_mut(s) {
            seg.hp = pct(TUNE.material(seg.material).hp, bonus);
        }
    }
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> Fx {
    let ab = b - a;
    let l2 = ab.len2();
    let u = if l2 == Fx::ZERO {
        Fx::ZERO
    } else {
        ((p - a).dot(ab) / l2).clamp_to(Fx::ZERO, Fx::ONE)
    };
    (p - (a + ab * u)).len()
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    fn flat() -> World {
        World::new(Terrain::flat(80, Fx::ZERO))
    }

    fn still(at: Vec2) -> Launch {
        Launch {
            pos: at,
            vel: Vec2::new(Fx::ratio(1, 4), Fx::ZERO),
        }
    }

    #[test]
    fn ammo_kinds_carry_their_tune() {
        let mut w = flat();
        let at = Vec2::ints(5, 5);
        let heavy = w.fire_shot(still(at), ShotKind::Heavy, ShotMods::default());
        assert_eq!(w.projectiles[heavy.balls[0]].mass, Fx::from_int(15));
        let fire = w.fire_shot(still(at), ShotKind::Fire, ShotMods::default());
        assert!(w.projectiles[fire.balls[0]].fire && fire.fuse.is_none());
        let spread = w.fire_shot(still(at), ShotKind::Spread, ShotMods::default());
        assert_eq!(spread.balls.len(), 5);
        let up = w.projectiles[spread.balls[0]].vel.y;
        let down = w.projectiles[spread.balls[4]].vel.y;
        assert!(up < Fx::ZERO && down > Fx::ZERO);
    }

    #[test]
    fn shot_mods_add_weight_blast_and_fire() {
        let mut w = flat();
        let mods = ShotMods {
            weight_pct: 25,
            blast_pct: 50,
            ignite: true,
        };
        let s = w.fire_shot(still(Vec2::ints(5, 5)), ShotKind::Exploding, mods);
        let b = w.projectiles[s.balls[0]];
        assert_eq!(b.mass, Fx::ratio(15, 2));
        assert!(b.fire);
        assert_eq!(s.fuse.unwrap().radius, Fx::ratio(9, 4));
    }

    #[test]
    fn an_exploding_shot_blows_up_a_wall_it_touches() {
        let mut w = flat();
        let near = w
            .add_beam(Vec2::ints(8, 0), Vec2::ints(8, 2), Material::Wood)
            .unwrap();
        let far = w
            .add_beam(Vec2::ints(20, 0), Vec2::ints(20, 2), Material::Wood)
            .unwrap();
        for _ in 0..60 {
            w.step();
        }
        let s = w.fire_shot(
            still(Vec2::new(Fx::from_int(7), Fx::ONE)),
            ShotKind::Exploding,
            ShotMods::default(),
        );
        let mut fuses = vec![s.fuse.unwrap()];
        let mut blasts = Vec::new();
        for _ in 0..30 {
            w.step();
            blasts.extend(w.step_fuses(&mut fuses));
        }
        assert_eq!(blasts.len(), 1);
        assert!(fuses.is_empty());
        assert!(w.segments[near].broken);
        assert!(!w.segments[far].broken);
    }

    #[test]
    fn a_blast_wears_beams_down_by_distance() {
        let mut w = flat();
        let near = w
            .add_beam(Vec2::ints(8, 0), Vec2::ints(8, 2), Material::Stone)
            .unwrap();
        let far = w
            .add_beam(Vec2::ints(12, 0), Vec2::ints(12, 2), Material::Stone)
            .unwrap();
        let b = w.explode(Vec2::ints(7, 1), Fx::from_int(2), Fx::from_int(2), Fx::ZERO);
        // 1 unit from a 2-unit blast: half of 2 hp
        assert_eq!(w.segments[near].hp, Fx::ONE);
        assert_eq!(w.segments[far].hp, Fx::from_int(2));
        assert_eq!(b.segments, vec![near]);
    }

    #[test]
    fn a_blast_kills_bodies_in_reach() {
        let mut w = flat();
        let floor = w
            .add_beam(Vec2::ints(2, 0), Vec2::ints(4, 0), Material::Stone)
            .unwrap();
        let body = w.add_body(floor, Fx::HALF, Fx::ratio(1, 4));
        let b = w.explode(w.bodies[body].pos, Fx::ONE, Fx::ZERO, Fx::ZERO);
        assert_eq!(b.bodies, vec![body]);
        assert!(!w.bodies[body].alive);
    }

    #[test]
    fn a_stronger_segment_strains_less() {
        let run = |bonus| {
            let mut w = flat();
            w.add_beam(Vec2::ints(5, 0), Vec2::ints(5, 2), Material::Wood)
                .unwrap();
            let arm = w
                .add_beam(Vec2::ints(5, 2), Vec2::ints(7, 2), Material::Wood)
                .unwrap();
            w.build();
            w.strengthen(arm, bonus);
            *w.static_test(150).max_strain.get(arm).unwrap()
        };
        let (plain, strong) = (run(0), run(100));
        assert!(strong < plain * 2 / 3, "{strong:?} vs {plain:?}");
    }
}
