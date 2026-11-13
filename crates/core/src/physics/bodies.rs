//! Point bodies for royals and buildings. A body sits on a floor segment and
//! rides it; when that segment breaks it falls, lands on the ground or on a
//! floor below, and dies if the drop was too far. Deaths come back as events.

use super::fixed::{Fx, Vec2};
use super::tune::TUNE;
use super::world::{Event, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Body {
    pub pos: Vec2,
    pub prev: Vec2,
    pub radius: Fx,
    /// A royal stands taller than it is wide: its shape is every point
    /// within `radius` of the upright from `pos` to `pos + (0, tall)`, so
    /// the feet stay on the circle's bottom (R1.2500). 0 for a building.
    pub tall: Fx,
    /// The segment it stands on and where along it (0..1), or `None` falling.
    pub attached: Option<(usize, Fx)>,
    /// Highest point since it last stood still.
    pub peak_y: Fx,
    pub alive: bool,
    /// The world tick it died on; a dead body keeps falling until it lies
    /// on the ground or a floor, never left in the air (R1.3603).
    pub died: Option<u32>,
}

impl Body {
    /// The point of the body's upright nearest `p`: hits measure from here.
    pub fn nearest(&self, p: Vec2) -> Vec2 {
        Vec2::new(self.pos.x, p.y.clamp_to(self.pos.y, self.pos.y + self.tall))
    }

    /// Kills the body on world tick `tick`, knocked by `knock` (units a
    /// tick): it lets go of its floor and flies off with that speed.
    pub fn kill(&mut self, tick: u32, knock: Vec2) {
        self.alive = false;
        self.died = Some(tick);
        self.attached = None;
        self.prev = self.pos - knock;
    }

    /// True when a circle of radius `r` at `p` touches the body.
    pub fn touches(&self, p: Vec2, r: Fx) -> bool {
        let rr = r + self.radius;
        let d = p - self.nearest(p);
        d.x.abs() <= rr && d.y.abs() <= rr && d.len2() < rr * rr
    }
}

impl World {
    fn standing_point(&self, s: usize, u: Fx, radius: Fx) -> Vec2 {
        let seg = &self.segments[s];
        Vec2::lerp(self.nodes[seg.a].pos, self.nodes[seg.b].pos, u) + Vec2::new(Fx::ZERO, radius)
    }

    /// Puts a body on segment `s` at `u` (0..1 from its first node).
    pub fn add_body(&mut self, s: usize, u: Fx, radius: Fx) -> usize {
        let u = u.clamp_to(Fx::ZERO, Fx::ONE);
        let pos = self.standing_point(s, u, radius);
        self.bodies.push(Body {
            pos,
            prev: pos,
            radius,
            tall: Fx::ZERO,
            attached: Some((s, u)),
            peak_y: pos.y,
            alive: true,
            died: None,
        });
        self.bodies.len() - 1
    }

    pub(super) fn step_bodies(&mut self, events: &mut Vec<Event>) {
        for i in 0..self.bodies.len() {
            let mut b = self.bodies[i];
            if b.alive && b.died.is_some() {
                b.died = None;
            }
            if let Some((s, _)) = b.attached {
                if self.segments[s].broken {
                    b.attached = None;
                }
            }
            match b.attached {
                Some((s, u)) => {
                    let p = self.standing_point(s, u, b.radius);
                    b.prev = b.pos;
                    b.pos = p;
                }
                None => {
                    let v = b.pos - b.prev;
                    b.prev = b.pos;
                    b.pos += v;
                    b.pos.y -= TUNE.gravity;
                    self.land(&mut b);
                }
            }
            let vy = b.pos.y - b.prev.y;
            if !b.alive {
                // the dead only fall; nothing below kills them again
            } else if vy >= -TUNE.fall_speed {
                let drop = b.peak_y - b.pos.y;
                if drop > TUNE.fall_kill {
                    b.alive = false;
                    b.died = Some(self.tick);
                    events.push(Event::BodyFell { body: i, drop });
                }
                b.peak_y = b.pos.y;
            } else if b.pos.y > b.peak_y {
                b.peak_y = b.pos.y;
            }
            self.bodies[i] = b;
        }
    }

    /// A falling body stops on the ground or on a floor-like segment it meets
    /// from above.
    fn land(&self, b: &mut Body) {
        let h = self.terrain.height_at(b.pos.x);
        if b.pos.y - b.radius <= h {
            b.pos.y = h + b.radius;
            b.prev = b.pos;
            return;
        }
        if b.pos.y >= b.prev.y {
            return;
        }
        for (si, s) in self.segments.iter().enumerate() {
            if s.broken || TUNE.material(s.material).rope {
                continue;
            }
            let (pa, pb) = (self.nodes[s.a].pos, self.nodes[s.b].pos);
            let ab = pb - pa;
            if ab.x.abs() < ab.y.abs() || ab.x == Fx::ZERO {
                continue;
            }
            let u = (b.pos.x - pa.x) / ab.x;
            if u < Fx::ZERO || u > Fx::ONE {
                continue;
            }
            let floor_y = pa.y + ab.y * u + b.radius;
            if b.pos.y <= floor_y && b.prev.y >= floor_y {
                b.attached = Some((si, u));
                b.pos = self.standing_point(si, u, b.radius);
                b.prev = b.pos;
                return;
            }
        }
    }
}
