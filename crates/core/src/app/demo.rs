//! The menu's backdrop: a real siege behind the title, Settings and What's
//! New. One of [`CASTLES`] is picked from the page's seed and built on the
//! sandbox's plot, royals and all; the bot aim (`ai::aim::Search`, worked a
//! few slices a frame) plans each throw at the royals still standing, and
//! the sandbox's own trebuchet lets go at the planned time. When the royals
//! are all down, or after [`SHOTS_PER_CASTLE`] throws, the castle stands
//! again. Nothing on it takes input. After Chromatic's menu backdrop
//! (`game/backdrop.rs`): the machinery, not its look.

use super::field::{Camera, Frame};
use super::sandbox::{Mode, Placed, Sandbox, Seat, AMMO};
use crate::ai::aim::{Search, Shooter, Target};
use crate::game::flow::Difficulty;
use crate::game::royals::RoyalKind;
use crate::physics::world::snap;
use crate::physics::{Fx, Material, Vec2};

/// The menu theme, as `app::MENU_MUSIC` plays when no backdrop shows.
const MUSIC: &str = "music_menu";
/// The backdrop view's width, units.
const VIEW_UNITS: i64 = 30;
/// How far across the view the castle's middle stands, percent.
const CASTLE_AT_PCT: i64 = 80;
/// Throws at one castle before it stands again.
pub const SHOTS_PER_CASTLE: u32 = 6;
/// Ticks between a shot landing and the next swing.
const PAUSE_TICKS: u32 = 70;
/// Ticks the fallen castle lies before it stands again.
const RESET_TICKS: u32 = 150;
/// Aim search slices worked per display frame.
const SLICES_PER_FRAME: u32 = 24;
/// Keeps the pick's stream apart from the plot's.
const PICK_SALT: u64 = 0xDE30_CA57;

/// One post: its column from the castle's left edge, its height in
/// floors, and what it is made of.
#[derive(Clone, Copy, Debug)]
pub struct Post {
    pub x: i64,
    pub floors: i64,
    pub material: Material,
}

/// A castle as data: posts left to right; between neighbors, floors up to
/// the lower post in `floor` material, braced when `braced` holds the bay;
/// royals on (bay, floor).
#[derive(Clone, Copy, Debug)]
pub struct CastlePlan {
    pub posts: &'static [Post],
    pub floor: Material,
    pub braced: &'static [usize],
    pub royals: &'static [(usize, i64, RoyalKind)],
}

const fn post(x: i64, floors: i64, material: Material) -> Post {
    Post { x, floors, material }
}

use Material::{Metal, Stone, Wood};
use RoyalKind::{King, Knight, Prince, Princess, Queen, Squire};

/// The castles the menu picks from.
pub const CASTLES: [CastlePlan; 4] = [
    // twin keeps: two stone towers and a low wooden hall between
    CastlePlan {
        posts: &[
            post(0, 5, Stone),
            post(1, 5, Stone),
            post(2, 3, Wood),
            post(3, 3, Wood),
            post(4, 3, Wood),
            post(5, 3, Wood),
            post(6, 5, Stone),
            post(7, 5, Stone),
        ],
        floor: Wood,
        braced: &[0, 1, 5, 6],
        royals: &[(0, 5, King), (6, 5, Queen), (3, 3, Knight), (2, 1, Squire)],
    },
    // the tall keep: a stone tower over a wooden curtain wall
    CastlePlan {
        posts: &[
            post(0, 2, Wood),
            post(1, 2, Wood),
            post(2, 2, Wood),
            post(3, 6, Stone),
            post(4, 6, Stone),
            post(5, 6, Stone),
            post(6, 2, Wood),
            post(7, 2, Wood),
        ],
        floor: Wood,
        braced: &[0, 1, 2, 3, 4, 5, 6],
        royals: &[(3, 6, King), (4, 3, Princess), (0, 2, Squire), (6, 2, Knight)],
    },
    // the gatehouse: a long wall with towers at both ends and the gate
    CastlePlan {
        posts: &[
            post(0, 4, Stone),
            post(1, 4, Stone),
            post(2, 2, Wood),
            post(3, 2, Wood),
            post(4, 4, Metal),
            post(5, 4, Metal),
            post(6, 2, Wood),
            post(7, 2, Wood),
            post(8, 4, Stone),
            post(9, 4, Stone),
        ],
        floor: Wood,
        braced: &[0, 2, 4, 6, 8],
        royals: &[(0, 4, Prince), (4, 4, King), (8, 4, Queen), (2, 2, Squire)],
    },
    // the stepped keep: rising stone tiers around a metal core
    CastlePlan {
        posts: &[
            post(0, 2, Stone),
            post(1, 3, Stone),
            post(2, 4, Stone),
            post(3, 6, Metal),
            post(4, 6, Metal),
            post(5, 4, Stone),
            post(6, 3, Stone),
            post(7, 2, Stone),
        ],
        floor: Wood,
        braced: &[0, 2, 3, 4, 6],
        royals: &[(3, 6, King), (1, 3, Knight), (4, 4, Princess), (6, 2, Squire)],
    },
];

/// Which castle a seed shows.
pub fn pick(seed: u64) -> usize {
    ((seed ^ PICK_SALT) % CASTLES.len() as u64) as usize
}

/// The placed segments and royals of `plan`, standing on the sandbox's
/// ground from its build zone's left edge.
pub fn lay_out(s: &Sandbox, plan: &CastlePlan) -> (Vec<Placed>, Vec<Seat>) {
    let (left, _, _) = s.zone();
    let x0 = snap(left) + Fx::from_int(2);
    let xs: Vec<Fx> = plan.posts.iter().map(|p| x0 + Fx::from_int(p.x)).collect();
    let ground = |x: Fx| snap(s.world.terrain.height_at(x));
    let base = xs.iter().map(|&x| ground(x)).max().unwrap_or(Fx::ZERO);
    let mut beams = Vec::new();
    let add = |beams: &mut Vec<Placed>, a: Vec2, b: Vec2, m: Material| {
        beams.push(Placed {
            from: a,
            to: b,
            material: m,
        })
    };
    for (p, &x) in plan.posts.iter().zip(&xs) {
        // up from the ground to the base in steps of at most one unit
        let mut y = ground(x);
        while base - y > Fx::ONE {
            add(&mut beams, Vec2::new(x, y), Vec2::new(x, y + Fx::ONE), p.material);
            y += Fx::ONE;
        }
        for f in 1..=p.floors {
            let to = base + Fx::from_int(f);
            add(&mut beams, Vec2::new(x, y), Vec2::new(x, to), p.material);
            y = to;
        }
    }
    let mut floor_at = Vec::new();
    for i in 0..plan.posts.len().saturating_sub(1) {
        let floors = plan.posts[i].floors.min(plan.posts[i + 1].floors);
        for f in 1..=floors {
            let y = base + Fx::from_int(f);
            floor_at.push(((i, f), beams.len()));
            add(
                &mut beams,
                Vec2::new(xs[i], y),
                Vec2::new(xs[i + 1], y),
                plan.floor,
            );
            if plan.braced.contains(&i) {
                let y0 = base + Fx::from_int(f - 1);
                add(
                    &mut beams,
                    Vec2::new(xs[i], y0),
                    Vec2::new(xs[i + 1], y),
                    plan.floor,
                );
            }
        }
    }
    let royals = plan
        .royals
        .iter()
        .filter_map(|&(bay, f, rank)| {
            floor_at
                .iter()
                .find(|(at, _)| *at == (bay, f))
                .map(|&(_, beam)| Seat {
                    beam,
                    u: Fx::HALF,
                    rank,
                })
        })
        .collect();
    (beams, royals)
}

/// What the backdrop is doing now.
#[derive(Clone, Debug)]
enum Step {
    /// Waiting this many ticks before planning the next throw.
    Wait(u32),
    /// The aim at work.
    Aim(Box<Search>),
    /// The throw in flight.
    Throw,
    /// The fallen castle, this many ticks before it stands again.
    Fallen(u32),
}

/// The menu's backdrop siege.
#[derive(Clone, Debug)]
pub struct Demo {
    pub sandbox: Sandbox,
    pub castle: usize,
    beams: Vec<Placed>,
    royals: Vec<Seat>,
    step: Step,
    shots: u32,
    seed: u64,
    /// The world tick last seen, to count waits in ticks.
    seen_tick: u32,
}

impl Demo {
    pub fn new(seed: u64) -> Demo {
        let mut sandbox = Sandbox::new(seed);
        let castle = pick(seed);
        let (beams, royals) = lay_out(&sandbox, &CASTLES[castle]);
        sandbox.beams = beams.clone();
        sandbox.royals = royals.clone();
        sandbox.rebuild();
        sandbox.toggle_mode();
        if sandbox.mode != Mode::Siege {
            sandbox.mode = Mode::Siege;
        }
        Demo {
            sandbox,
            castle,
            beams,
            royals,
            step: Step::Wait(PAUSE_TICKS),
            shots: 0,
            seed,
            seen_tick: 0,
        }
    }

    /// The royals still standing, as aim targets.
    fn targets(&self) -> Vec<Target> {
        self.sandbox
            .world
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive)
            .map(|(body, _)| Target { body, value: 1 })
            .collect()
    }

    fn shooter(&self) -> Shooter {
        let ammo = AMMO[0];
        Shooter {
            base: self.sandbox.engine_base(),
            facing_left: false,
            power: super::sandbox::power(),
            radius: ammo.radius,
            mass: ammo.mass,
            fire: ammo.fire,
            spread: ammo.spread,
        }
    }

    /// Puts the castle up again.
    fn stand_again(&mut self) {
        self.sandbox.beams = self.beams.clone();
        self.sandbox.royals = self.royals.clone();
        self.sandbox.rebuild();
        self.shots = 0;
    }

    /// One display frame: the world steps by the clock, and the backdrop
    /// moves on by the ticks that passed.
    pub fn pump(&mut self, now_ms: i64) {
        self.sandbox.pump(now_ms, 0, None);
        let tick = self.sandbox.world.tick;
        let passed = tick.saturating_sub(self.seen_tick);
        self.seen_tick = tick;
        self.step = match std::mem::replace(&mut self.step, Step::Wait(0)) {
            Step::Wait(n) if n > passed => Step::Wait(n - passed),
            Step::Wait(_) => {
                let targets = self.targets();
                if targets.is_empty() || self.shots >= SHOTS_PER_CASTLE {
                    Step::Fallen(RESET_TICKS)
                } else {
                    self.seed = self.seed.wrapping_add(1);
                    let s = Search::new(
                        &self.sandbox.world,
                        self.shooter(),
                        &targets,
                        &[],
                        Difficulty::Normal,
                        self.seed,
                        &[],
                    );
                    Step::Aim(Box::new(s))
                }
            }
            Step::Aim(mut s) => {
                for _ in 0..SLICES_PER_FRAME {
                    if s.plan().is_some() {
                        break;
                    }
                    s.advance(&self.sandbox.world);
                }
                match s.plan() {
                    Some(plan) => {
                        self.sandbox.throw_at(plan.release);
                        self.shots += 1;
                        Step::Throw
                    }
                    None => Step::Aim(s),
                }
            }
            Step::Throw if self.sandbox.throwing() => Step::Throw,
            Step::Throw => Step::Wait(PAUSE_TICKS),
            Step::Fallen(n) if n > passed => Step::Fallen(n - passed),
            Step::Fallen(_) => {
                self.stand_again();
                Step::Wait(PAUSE_TICKS)
            }
        };
    }

    /// The backdrop's still view: the castle on the right, its middle
    /// [`CASTLE_AT_PCT`] percent across, the shots flying in from the left.
    pub fn camera(&self) -> Camera {
        let (w, h) = self.sandbox.size();
        let xs = self.beams.iter().flat_map(|b| [b.from.x, b.to.x]);
        let (lo, hi) = xs.fold((Fx(i64::MAX), Fx(i64::MIN)), |(lo, hi), x| (lo.min(x), hi.max(x)));
        let width = Fx::from_int(VIEW_UNITS);
        let mid = (lo + hi) / 2;
        let left = mid - width * CASTLE_AT_PCT / 100;
        let mut cam = Camera::spanning(left, width, Fx::ZERO, w, h);
        let ground = self.sandbox.world.terrain.height_at(mid);
        cam.bottom = ground - cam.height() / 4;
        cam
    }

    /// The backdrop as drawn, under the calm music.
    pub fn frame(&self) -> Frame {
        let mut f = self.sandbox.frame_from(self.camera());
        f.music = MUSIC;
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_every_castle_stands_with_all_its_royals() {
        let mut bad = Vec::new();
        for (k, plan) in CASTLES.iter().enumerate() {
            let s = Sandbox::new(3);
            let (beams, royals) = lay_out(&s, plan);
            assert_eq!(
                royals.len(),
                plan.royals.len(),
                "castle {k}: a royal has no floor"
            );
            let mut s = s;
            s.beams = beams;
            s.royals = royals;
            s.rebuild();
            for _ in 0..240 {
                s.world.step();
            }
            let broken = s.world.segments.iter().filter(|g| g.broken).count();
            bad.push((k, broken));
            assert!(
                s.world.bodies.iter().all(|b| b.alive),
                "castle {k}: a royal died standing still"
            );
        }
        bad.retain(|&(_, b)| b > 0);
        assert!(
            bad.is_empty(),
            "castles that fall by themselves (castle, segments broken): {bad:?}"
        );
    }

    #[test]
    fn demo_pick_follows_the_seed_and_shows_every_castle() {
        let mut seen = [false; CASTLES.len()];
        for seed in 0..16u64 {
            seen[pick(seed)] = true;
            assert_eq!(pick(seed), pick(seed));
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn demo_throws_at_the_castle_and_breaks_it() {
        let mut d = Demo::new(5);
        let mut t = 0;
        let mut thrown = false;
        // a frame every 16 ms for 40 seconds
        while t < 40_000 {
            d.pump(t);
            thrown |= d.sandbox.throwing();
            t += 16;
            if d.sandbox.world.segments.iter().any(|g| g.broken) {
                break;
            }
        }
        assert!(thrown, "the trebuchet threw");
        assert!(
            d.sandbox.world.segments.iter().any(|g| g.broken),
            "a throw broke a segment"
        );
    }
}
