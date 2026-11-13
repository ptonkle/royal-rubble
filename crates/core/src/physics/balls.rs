//! Shots and debris: circles with mass and velocity. They hit beams (never
//! ropes), push their nodes, wear down their hit points and break them.
//! Air effects are operations on a live shot.

use super::fixed::{cos, sin, Fx, Vec2};
use super::trebuchet::Launch;
use super::tune::{Material, TUNE};
use super::world::{BreakCause, Event, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: Fx,
    pub mass: Fx,
    /// Ticks left passing through beams.
    pub phase: u32,
    /// Sets wood alight on contact.
    pub fire: bool,
    pub alive: bool,
    pub age: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirEffect {
    Drop,
    Reverse,
    Phase,
    Split,
    Dive,
    Boost,
}

impl Ball {
    pub fn new(pos: Vec2, vel: Vec2, radius: Fx, mass: Fx) -> Ball {
        Ball {
            pos,
            vel,
            radius,
            mass,
            phase: 0,
            fire: false,
            alive: true,
            age: 0,
        }
    }

    /// Stops all sideways motion: it falls straight down from here.
    pub fn drop_down(&mut self) {
        self.vel.x = Fx::ZERO;
    }

    pub fn reverse(&mut self) {
        self.vel.x = -self.vel.x;
    }

    pub fn phase_for(&mut self, ticks: u32) {
        self.phase = ticks;
    }

    /// Straight down at twice its current speed.
    pub fn dive(&mut self) {
        let speed = self.vel.len();
        self.vel = Vec2::new(Fx::ZERO, -(speed * 2));
    }

    /// Extra speed along its flight path.
    pub fn boost(&mut self, speed: Fx) {
        self.vel += self.vel.normalized() * speed;
    }

    /// Three pieces, each a third of the mass, fanned by `spread` brads.
    pub fn split(&self, spread: i64) -> [Ball; 3] {
        let mut out = [*self; 3];
        for (k, piece) in out.iter_mut().enumerate() {
            piece.mass = self.mass / 3;
            if k == 1 {
                continue;
            }
            let turn = spread * (k as i64 - 1);
            piece.vel = self.vel.rotate(cos(turn), sin(turn));
        }
        out
    }
}

impl World {
    /// Puts a shot in the air; returns its index.
    pub fn fire(&mut self, launch: Launch, radius: Fx, mass: Fx) -> usize {
        self.projectiles
            .push(Ball::new(launch.pos, launch.vel, radius, mass));
        self.projectiles.len() - 1
    }

    /// Applies an air effect to a live shot. Returns the indices of the
    /// shots it leaves (three for a split, else the same one).
    pub fn air_effect(&mut self, projectile: usize, effect: AirEffect) -> Vec<usize> {
        let Some(ball) = self.projectiles.get_mut(projectile) else {
            return Vec::new();
        };
        if !ball.alive {
            return Vec::new();
        }
        match effect {
            AirEffect::Drop => ball.drop_down(),
            AirEffect::Reverse => ball.reverse(),
            AirEffect::Phase => ball.phase_for(TUNE.phase_ticks),
            AirEffect::Dive => ball.dive(),
            AirEffect::Boost => ball.boost(TUNE.boost_speed),
            AirEffect::Split => {
                let [left, mid, right] = ball.split(TUNE.split_spread);
                *ball = mid;
                self.projectiles.push(left);
                self.projectiles.push(right);
                let n = self.projectiles.len();
                return vec![projectile, n - 2, n - 1];
            }
        }
        vec![projectile]
    }

    pub(super) fn spawn_debris(&mut self, s: usize) {
        let seg = &self.segments[s];
        let (na, nb) = (&self.nodes[seg.a], &self.nodes[seg.b]);
        let radius = (seg.rest / 4).min(TUNE.debris_max_radius);
        let mass = seg.mass() / 2;
        for t in [Fx::ratio(1, 4), Fx::ratio(3, 4)] {
            let pos = Vec2::lerp(na.pos, nb.pos, t);
            let vel = Vec2::lerp(na.velocity(), nb.velocity(), t);
            self.debris.push(Ball::new(pos, vel, radius, mass));
        }
    }

    pub(super) fn step_balls(&mut self, to_break: &mut Vec<(usize, BreakCause)>, events: &mut Vec<Event>) {
        for i in 0..self.projectiles.len() {
            let mut b = self.projectiles[i];
            self.step_ball(&mut b, i, false, to_break, events);
            self.projectiles[i] = b;
        }
        for i in 0..self.debris.len() {
            let mut b = self.debris[i];
            self.step_ball(&mut b, i, true, to_break, events);
            self.debris[i] = b;
        }
    }

    fn step_ball(
        &mut self,
        ball: &mut Ball,
        index: usize,
        debris: bool,
        to_break: &mut Vec<(usize, BreakCause)>,
        events: &mut Vec<Event>,
    ) {
        if !ball.alive {
            return;
        }
        ball.vel.y -= TUNE.gravity;
        if !debris {
            ball.vel.x += self.wind;
        }
        ball.pos += ball.vel;
        ball.age += 1;
        if ball.phase > 0 {
            ball.phase -= 1;
        } else if !debris || ball.age > TUNE.debris_grace {
            self.collide_beams(ball, to_break);
        }
        self.collide_bodies(ball, index, debris, events);
        let h = self.terrain.height_at(ball.pos.x);
        if ball.pos.y - ball.radius < h {
            ball.pos.y = h + ball.radius;
            if ball.vel.y < Fx::ZERO {
                ball.vel.y = -ball.vel.y * TUNE.ground_bounce;
                ball.vel.x = ball.vel.x * TUNE.ground_friction;
            }
            if ball.vel.len2() < TUNE.rest_speed * TUNE.rest_speed {
                ball.alive = false;
            }
        }
        let m = TUNE.edge_margin;
        if ball.pos.x < -m || ball.pos.x > self.terrain.width() + m || ball.pos.y < -m {
            ball.alive = false;
        }
        if debris && ball.age > TUNE.debris_life {
            ball.alive = false;
        }
    }

    fn collide_beams(&mut self, ball: &mut Ball, to_break: &mut Vec<(usize, BreakCause)>) {
        for si in 0..self.segments.len() {
            let (a, b, material) = {
                let s = &self.segments[si];
                if s.broken {
                    continue;
                }
                (s.a, s.b, s.material)
            };
            let m = TUNE.material(material);
            if m.rope {
                continue;
            }
            let (pa, pb) = (self.nodes[a].pos, self.nodes[b].pos);
            let rr = ball.radius + m.thickness / 2;
            let lo_x = pa.x.min(pb.x) - rr;
            let hi_x = pa.x.max(pb.x) + rr;
            let lo_y = pa.y.min(pb.y) - rr;
            let hi_y = pa.y.max(pb.y) + rr;
            if ball.pos.x < lo_x || ball.pos.x > hi_x || ball.pos.y < lo_y || ball.pos.y > hi_y {
                continue;
            }
            let ab = pb - pa;
            let l2 = ab.len2();
            let u = if l2 == Fx::ZERO {
                Fx::ZERO
            } else {
                ((ball.pos - pa).dot(ab) / l2).clamp_to(Fx::ZERO, Fx::ONE)
            };
            let p = pa + ab * u;
            let diff = ball.pos - p;
            let d2 = diff.len2();
            if d2 >= rr * rr {
                continue;
            }
            let dist = d2.sqrt();
            let n = if dist == Fx::ZERO {
                -ball.vel.normalized()
            } else {
                Vec2::new(diff.x / dist, diff.y / dist)
            };
            let vs = Vec2::lerp(self.nodes[a].velocity(), self.nodes[b].velocity(), u);
            let vn = (ball.vel - vs).dot(n);
            ball.pos = p + n * rr;
            if vn >= Fx::ZERO {
                continue;
            }
            let (wa, wb) = (self.nodes[a].inv_mass, self.nodes[b].inv_mass);
            let iu = Fx::ONE - u;
            let w_seg = wa * iu * iu + wb * u * u;
            let w_ball = Fx::ONE / ball.mass;
            let mut j = -((Fx::ONE + TUNE.restitution) * vn) / (w_ball + w_seg);
            let seg = &mut self.segments[si];
            let mut breaks = false;
            if j > TUNE.min_damage_impulse {
                if j >= seg.hp {
                    j = seg.hp;
                    breaks = true;
                }
                seg.hp -= j;
            }
            if ball.fire && m.burns && seg.burn_ticks == 0 {
                seg.burn_ticks = TUNE.burn_ticks;
            }
            ball.vel += n * (j * w_ball);
            self.nodes[a].prev += n * (j * wa * iu);
            self.nodes[b].prev += n * (j * wb * u);
            if breaks && World::mark_broken(&mut self.segments, si) {
                to_break.push((si, BreakCause::Impact));
            }
        }
    }

    fn collide_bodies(&mut self, ball: &Ball, index: usize, debris: bool, events: &mut Vec<Event>) {
        for (bi, body) in self.bodies.iter_mut().enumerate() {
            if !body.alive {
                continue;
            }
            let rr = ball.radius + body.radius;
            let d = ball.pos - body.pos;
            if d.x.abs() > rr || d.y.abs() > rr || d.len2() >= rr * rr {
                continue;
            }
            if !debris {
                body.alive = false;
                events.push(Event::BodyHit {
                    body: bi,
                    projectile: index,
                });
            } else {
                let impulse = ball.mass * (ball.vel - (body.pos - body.prev)).len();
                if impulse >= TUNE.crush_impulse {
                    body.alive = false;
                    events.push(Event::BodyCrushed { body: bi });
                }
            }
        }
    }

    /// Sets a wood segment alight. Stone, metal and rope don't burn.
    pub fn ignite(&mut self, s: usize) -> bool {
        let seg = &mut self.segments[s];
        if seg.broken || !TUNE.material(seg.material).burns {
            return false;
        }
        seg.burn_ticks = TUNE.burn_ticks;
        true
    }

    pub(super) fn step_fire(&mut self, to_break: &mut Vec<(usize, BreakCause)>) {
        let spread = TUNE.burn_spread_every > 0 && self.tick.is_multiple_of(TUNE.burn_spread_every);
        let mut catch = Vec::new();
        for i in 0..self.segments.len() {
            let s = &mut self.segments[i];
            if s.broken || s.burn_ticks == 0 {
                continue;
            }
            s.burn_ticks -= 1;
            s.hp -= TUNE.burn_damage;
            let (a, b) = (s.a, s.b);
            if s.hp <= Fx::ZERO {
                s.hp = Fx::ZERO;
                if World::mark_broken(&mut self.segments, i) {
                    to_break.push((i, BreakCause::Fire));
                }
            }
            if spread {
                for n in [a, b] {
                    for &o in self.node_segs.get(n).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let other = &self.segments[o];
                        if !other.broken && other.burn_ticks == 0 && other.material == Material::Wood {
                            catch.push(o);
                        }
                    }
                }
            }
        }
        for o in catch {
            self.segments[o].burn_ticks = TUNE.burn_ticks;
        }
    }
}
