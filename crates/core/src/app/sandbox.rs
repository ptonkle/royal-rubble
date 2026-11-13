//! The sandbox: one plot, one castle, one trebuchet and no rules. Build with
//! segments Poly Bridge-style, test the strain, then switch to siege and
//! throw shots at it. It is the playfield the match screen will reuse: the
//! frame (`field.rs`), the camera, the build tools and the trebuchet input
//! live here until `play/` hands in a match in place of the sandbox world.
//!
//! The page sends pointer positions in pixels and the keys held; everything
//! else (snapping, reach, what may be placed, release timing) is decided here.

use std::collections::BTreeMap;

use super::field::{
    cell_units, cloth_at, ground_line, material_name, terrain_of, tree_marks, world_parts, world_right,
    Camera, Cue, Engine, Figure, Flag, Frame, Preview, Ring, CELL_TO_RAW,
};
use crate::game::royals::RoyalKind;
use crate::physics::trebuchet::{pose, Phase as Arm};
use crate::physics::world::snap;
use crate::physics::{trebuchet_launch_at, AirEffect, BreakCause, Event, Fx, Material, Vec2, World, TUNE};
use crate::strings::Strings;
use crate::terrain::{Plot, COL_W};
use crate::view::{BarOption, Item, Panel};

/// Input bits, by key code (the page reads them from `Ui::keys`).
pub const KEY_SPACE: u32 = 1;
const KEY_DIGITS: [(&str, u32); 5] = [
    ("Digit1", 1 << 1),
    ("Digit2", 1 << 2),
    ("Digit3", 1 << 3),
    ("Digit4", 1 << 4),
    ("Digit5", 1 << 5),
];
const KEY_AIR: [(&str, u32); 4] = [
    ("KeyQ", 1 << 6),
    ("KeyW", 1 << 7),
    ("KeyE", 1 << 8),
    ("KeyR", 1 << 9),
];

/// The keys the sandbox listens to.
pub fn keys() -> BTreeMap<String, u32> {
    let mut k = BTreeMap::new();
    k.insert("Space".to_string(), KEY_SPACE);
    for (code, bit) in KEY_DIGITS.iter().chain(KEY_AIR.iter()) {
        k.insert(code.to_string(), *bit);
    }
    k
}

/// The sandbox's trebuchet power: a level-1 match's, so it throws the same.
pub fn power() -> u8 {
    crate::play::siege::launch_power(crate::game::tune::LEVELS[0].shot_power_pct)
}
/// Wind per level, raw units per tick squared (about 2 units of drift over a
/// two-second flight).
const WIND_STEP: i64 = 18;
/// Most physics ticks one display frame may catch up.
const MAX_CATCH_UP: i64 = 4;
/// A trail point every this many ticks.
const TRAIL_EVERY: u32 = 2;
/// Ticks a Test runs the castle for.
const TEST_TICKS: u32 = 240;
use crate::play::tune::{ROYAL_RADIUS, ROYAL_UPRIGHT};
/// One segment length, for the royal spacing (wood's longest, as play/ has it).
const SEGMENT_LENGTH: Fx = Fx::from_int(2);
/// Breaks in one frame that sound as a collapse.
const COLLAPSE_MIN: u32 = 3;
/// Cues kept for the page.
const CUES_KEPT: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Build,
    Siege,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Beam(Material),
    Royal,
}

const TOOLS: [Tool; 5] = [
    Tool::Beam(Material::Wood),
    Tool::Beam(Material::Stone),
    Tool::Beam(Material::Metal),
    Tool::Beam(Material::Rope),
    Tool::Royal,
];

/// One kind of shot.
#[derive(Clone, Copy, Debug)]
pub struct Ammo {
    pub name: &'static str,
    pub radius: Fx,
    pub mass: Fx,
    pub fire: bool,
    /// splits in three as it leaves the arm
    pub spread: bool,
}

pub const AMMO: [Ammo; 4] = [
    Ammo {
        name: "normal",
        radius: Fx::ratio(1, 3),
        mass: Fx::from_int(6),
        fire: false,
        spread: false,
    },
    Ammo {
        name: "heavy",
        radius: Fx::ratio(1, 2),
        mass: Fx::from_int(16),
        fire: false,
        spread: false,
    },
    Ammo {
        name: "fire",
        radius: Fx::ratio(1, 3),
        mass: Fx::from_int(6),
        fire: true,
        spread: false,
    },
    Ammo {
        name: "spread",
        radius: Fx::ratio(1, 4),
        mass: Fx::from_int(9),
        fire: false,
        spread: true,
    },
];

const AIR: [(AirEffect, &str, &str); 4] = [
    (AirEffect::Drop, "drop", "air_drop"),
    (AirEffect::Reverse, "reverse", "air_reverse"),
    (AirEffect::Phase, "phase", "phase_shimmer"),
    (AirEffect::Split, "split", "split_pop"),
];

const RANKS: [RoyalKind; 6] = [
    RoyalKind::Squire,
    RoyalKind::Knight,
    RoyalKind::Princess,
    RoyalKind::Prince,
    RoyalKind::Queen,
    RoyalKind::King,
];

fn rank_name(k: RoyalKind) -> &'static str {
    match k {
        RoyalKind::Squire => "squire",
        RoyalKind::Knight => "knight",
        RoyalKind::Princess => "princess",
        RoyalKind::Prince => "prince",
        RoyalKind::Queen => "queen",
        RoyalKind::King => "king",
    }
}

fn yelp(k: RoyalKind) -> &'static str {
    match k {
        RoyalKind::Squire => "royal_squire",
        RoyalKind::Knight => "royal_knight",
        RoyalKind::Princess => "royal_princess",
        RoyalKind::Prince => "royal_prince",
        RoyalKind::Queen => "royal_queen",
        RoyalKind::King => "royal_king",
    }
}

fn impact(m: Material) -> &'static str {
    match m {
        Material::Wood | Material::Rope => "impact_wood",
        Material::Stone => "impact_stone",
        Material::Metal => "impact_metal",
    }
}

/// A placed segment, kept as built so the world can be rebuilt from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub from: Vec2,
    pub to: Vec2,
    pub material: Material,
}

/// A royal standing on placed segment `beam` at `u` along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub beam: usize,
    pub u: Fx,
    pub rank: RoyalKind,
}

#[derive(Clone, Debug)]
pub struct Sandbox {
    pub plot: Plot,
    pub level: u8,
    pub mode: Mode,
    pub tool: Tool,
    pub ammo: usize,
    pub beams: Vec<Placed>,
    pub royals: Vec<Seat>,
    pub world: World,
    /// the rank of each body in `world`
    ranks: Vec<RoyalKind>,
    /// each segment's highest strain from the last Test
    test: Option<Vec<Fx>>,
    drag: Option<Vec2>,
    cursor: Option<Vec2>,
    size: (i32, i32),
    /// the world tick the swing started on
    swing: Option<u32>,
    released: bool,
    /// The swing tick the last shot left at.
    release_at: u32,
    shot: Option<usize>,
    effect_used: bool,
    path: Vec<Vec2>,
    trail: Vec<Vec2>,
    wind_level: i32,
    clock: Option<i64>,
    stepped: i64,
    prev_keys: u32,
    cues: Vec<Cue>,
    serial: u32,
    /// The siege view as shown, easing toward `camera_target` each tick
    /// (R1.2201).
    eye: Option<Camera>,
}

impl Sandbox {
    /// A sandbox on the plot for `seed`, with a small castle standing.
    pub fn new(seed: u64) -> Sandbox {
        let plot = Plot::generate(seed);
        let world = World::new(terrain_of(&plot));
        let mut s = Sandbox {
            plot,
            level: 1,
            mode: Mode::Build,
            tool: Tool::Beam(Material::Wood),
            ammo: 0,
            beams: Vec::new(),
            royals: Vec::new(),
            world,
            ranks: Vec::new(),
            test: None,
            drag: None,
            cursor: None,
            size: (1920, 1080),
            swing: None,
            released: false,
            release_at: 0,
            shot: None,
            effect_used: false,
            path: Vec::new(),
            trail: Vec::new(),
            wind_level: (seed % 5) as i32 - 2,
            clock: None,
            stepped: 0,
            prev_keys: 0,
            cues: Vec::new(),
            eye: None,
            serial: 0,
        };
        s.starter_castle();
        s.rebuild();
        s
    }

    fn cue(&mut self, name: &'static str, count: u32) {
        self.serial += 1;
        self.cues.push(Cue {
            n: self.serial,
            name,
            count,
        });
        if self.cues.len() > CUES_KEPT {
            self.cues.remove(0);
        }
    }

    fn ground(&self, x: Fx) -> Fx {
        self.world.terrain.height_at(x)
    }

    /// The buildable area in world units: left, right, top.
    pub fn zone(&self) -> (Fx, Fx, Fx) {
        let cols = self.plot.buildable(self.level);
        let col = cell_units(COL_W);
        let front = cell_units(self.plot.heights[cols.start]);
        (
            col * cols.start as i64,
            col * cols.end as i64,
            front + cell_units(Plot::build_height(self.level)),
        )
    }

    /// Two stone towers joined by a wall, wood floors and braces, a royal on
    /// each tower and one on the wall.
    fn starter_castle(&mut self) {
        let (left, _, _) = self.zone();
        let x0 = snap(left) + Fx::from_int(3);
        let xs: Vec<Fx> = (0..7).map(|i| x0 + Fx::from_int(i)).collect();
        let base = xs.iter().map(|&x| snap(self.ground(x))).max().unwrap_or(Fx::ZERO);
        let tall = [4, 4, 2, 2, 2, 4, 4];
        let add = |s: &mut Sandbox, a: Vec2, b: Vec2, m: Material| {
            s.beams.push(Placed {
                from: a,
                to: b,
                material: m,
            })
        };
        for (i, &x) in xs.iter().enumerate() {
            let foot = snap(self.ground(x));
            let mut y = foot;
            for f in 1..=tall[i] {
                let to = base + Fx::from_int(f);
                if to - y > Fx::from_int(2) {
                    let mid = snap(y + (to - y) / 2);
                    add(self, Vec2::new(x, y), Vec2::new(x, mid), Material::Stone);
                    y = mid;
                }
                add(self, Vec2::new(x, y), Vec2::new(x, to), Material::Stone);
                y = to;
            }
        }
        for i in 0..6 {
            let floors = tall[i].min(tall[i + 1]);
            for f in 1..=floors {
                let y = base + Fx::from_int(f);
                add(self, Vec2::new(xs[i], y), Vec2::new(xs[i + 1], y), Material::Wood);
                if i != 2 && i != 3 {
                    let y0 = base + Fx::from_int(f - 1);
                    add(
                        self,
                        Vec2::new(xs[i], y0),
                        Vec2::new(xs[i + 1], y),
                        Material::Wood,
                    );
                }
            }
        }
        let top = |s: &Sandbox, a: Fx, y: Fx| {
            s.beams
                .iter()
                .position(|p| p.from == Vec2::new(a, y) && p.to.y == y && p.to.x > a)
        };
        let seats = [(xs[0], 4), (xs[3], 2), (xs[5], 4)];
        for (k, (x, f)) in seats.into_iter().enumerate() {
            if let Some(beam) = top(self, x, base + Fx::from_int(f)) {
                self.royals.push(Seat {
                    beam,
                    u: Fx::HALF,
                    rank: RANKS[k],
                });
            }
        }
    }

    /// The physics world, rebuilt from the placed segments and royals.
    pub fn rebuild(&mut self) {
        let mut w = World::new(self.world.terrain.clone());
        w.wind = Fx(self.wind_level as i64 * WIND_STEP);
        let index: Vec<Option<usize>> = self
            .beams
            .iter()
            .map(|p| w.add_beam(p.from, p.to, p.material).ok())
            .collect();
        self.ranks.clear();
        for seat in &self.royals {
            if let Some(Some(s)) = index.get(seat.beam) {
                let b = w.add_body(*s, seat.u, ROYAL_RADIUS);
                w.bodies[b].tall = ROYAL_UPRIGHT;
                self.ranks.push(seat.rank);
            }
        }
        self.world = w;
        self.shot = None;
        self.swing = None;
        self.released = false;
    }

    pub fn set_size(&mut self, value: &str) -> Result<(), String> {
        let (w, h) = pair(value)?;
        self.size = (w.clamp(320, 7680), h.clamp(240, 4320));
        Ok(())
    }

    // ---- the camera ----

    /// The view shown: eased in the siege, exact in the build.
    pub fn camera(&self) -> Camera {
        match (self.mode, self.eye) {
            (Mode::Siege, Some(eye)) => eye,
            _ => self.camera_target(),
        }
    }

    /// One tick of the shown view easing toward its target, as the match's
    /// does (R1.2201), kept inside the world.
    fn move_eye(&mut self) {
        let target = self.camera_target();
        let eye = self
            .eye
            .map_or(target, |e| e.ease_toward(target, super::arena::CAM_EASE));
        self.eye = Some(eye.kept_in(world_right(&self.plot)));
    }

    /// Where the view heads, kept inside the world (R1.2200).
    pub fn camera_target(&self) -> Camera {
        self.view().kept_in(world_right(&self.plot))
    }

    fn view(&self) -> Camera {
        let (w, h) = self.size;
        let (zl, zr, _) = self.zone();
        let margin = Fx::from_int(2);
        match self.mode {
            Mode::Build => {
                let mut cam = Camera::spanning(zl - margin, zr - zl + margin * 2, Fx::ZERO, w, h);
                let front = self.ground(zl);
                cam.bottom = front - cam.height() / 5;
                cam
            }
            Mode::Siege => {
                let base = self.engine_base();
                // as far as the castle reaches, not the whole plot
                let reach = self
                    .world
                    .nodes
                    .iter()
                    .map(|n| n.pos.x)
                    .max()
                    .unwrap_or(zl)
                    .max(zl + Fx::from_int(10));
                let castle_right = (reach + Fx::from_int(6)).min(zr + margin);
                let flying = self
                    .shot
                    .and_then(|i| self.world.projectiles.get(i))
                    .filter(|b| b.alive);
                // the trebuchet stays in view for the whip (R1.1800)
                let held = flying.is_some_and(|b| b.age < super::arena::CAM_HOLD);
                let flying = flying.filter(|_| !held);
                if let Some(b) = flying {
                    // in flight: both ends, zoomed out to keep the shot in view
                    let left = base.x - Fx::from_int(6);
                    let mut cam = Camera::spanning(left, castle_right - left, Fx::ZERO, w, h);
                    cam.bottom = base.y.min(self.ground(zl)) - cam.height() / 5;
                    let edge = Fx::from_int(3);
                    let tall = b.pos.y + edge - cam.bottom;
                    let wide = b.pos.x + edge - cam.left;
                    let ppu_y = Fx::from_int(h as i64) / tall.max(Fx::ONE);
                    let ppu_x = Fx::from_int(w as i64) / wide.max(Fx::ONE);
                    cam.ppu = cam.ppu.min(ppu_y).min(ppu_x);
                    return cam;
                }
                // the build view's width, so a castle and a trebuchet read at one scale
                let width = zr - zl + margin * 2;
                if self.swing.is_none() && !self.trail.is_empty() && !held {
                    // after a shot lands: settle on the castle
                    let left = zl - margin;
                    let mut cam = Camera::spanning(
                        left,
                        width.min(castle_right - left).max(width / 2),
                        Fx::ZERO,
                        w,
                        h,
                    );
                    cam.bottom = self.ground(zl) - cam.height() / 5;
                    return cam;
                }
                // aiming: the trebuchet, a quarter in from the left
                let mut cam = Camera::spanning(base.x - width / 4, width, Fx::ZERO, w, h);
                cam.bottom = base.y - cam.height() / 4;
                cam
            }
        }
    }

    // ---- the trebuchet ----

    pub fn engine_base(&self) -> Vec2 {
        let col = cell_units(COL_W);
        let x = col * self.plot.launch_col as i64;
        Vec2::new(x, self.ground(x))
    }

    /// The tick the arm stops, when a held swing throws by itself.
    pub fn swing_end() -> u32 {
        crate::physics::SWING_END
    }

    fn swing_tick(&self) -> Option<u32> {
        self.swing.map(|s| self.world.tick - s)
    }

    fn shot_alive(&self) -> bool {
        self.shot
            .and_then(|i| self.world.projectiles.get(i))
            .is_some_and(|b| b.alive)
    }

    /// Space: start the swing, or let go.
    fn space(&mut self, part: Option<u8>) {
        if self.mode != Mode::Siege {
            return;
        }
        match (self.swing, self.released) {
            (None, _) if !self.shot_alive() => {
                self.swing = Some(self.world.tick);
                self.cue("trebuchet_creak", 0);
            }
            (Some(_), false) => self.release(part),
            _ => {}
        }
    }

    /// Lets go `part` sixteenths of a tick after the last tick run (0-31),
    /// or on it when `None` (R1.2202).
    fn release(&mut self, part: Option<u8>) {
        let t = self.swing_tick().unwrap_or(0);
        let parts = crate::physics::RELEASE_PARTS;
        let at = t * parts + part.map_or(0, |p| (p as u32).min(2 * parts - 1));
        let launch = trebuchet_launch_at(self.engine_base(), at, power(), false);
        let ammo = AMMO[self.ammo];
        let i = self.world.fire(launch, ammo.radius, ammo.mass);
        self.world.projectiles[i].fire = ammo.fire;
        if ammo.spread {
            for k in self.world.air_effect(i, AirEffect::Split) {
                self.world.projectiles[k].radius = ammo.radius;
            }
        }
        self.shot = Some(i);
        self.released = true;
        self.release_at = t;
        self.effect_used = false;
        self.path.clear();
        self.cue("trebuchet_release", 0);
    }

    fn air(&mut self, k: usize) {
        if self.mode != Mode::Siege || self.effect_used || !self.shot_alive() {
            return;
        }
        let (effect, _, sound) = AIR[k];
        if let Some(i) = self.shot {
            self.world.air_effect(i, effect);
            self.effect_used = true;
            self.cue(sound, 0);
        }
    }

    // ---- one display frame ----

    /// Reads the keys and steps the world up to `now_ms` (siege only).
    /// `press_us` is when the latest key went down (microseconds), so a
    /// release is timed within a tick as in the match (R1.2202).
    pub fn pump(&mut self, now_ms: i64, keys: u32, press_us: Option<i64>) {
        let pressed = keys & !self.prev_keys;
        self.prev_keys = keys;
        if pressed & KEY_SPACE != 0 {
            let part = self.press_part(press_us);
            self.space(part);
        }
        for (k, (_, bit)) in KEY_DIGITS.iter().enumerate() {
            if pressed & bit != 0 {
                self.pick(k);
            }
        }
        for (k, (_, bit)) in KEY_AIR.iter().enumerate() {
            if pressed & bit != 0 {
                self.air(k);
            }
        }
        let start = *self.clock.get_or_insert(now_ms);
        let due = (now_ms - start) * TUNE_TICKS / 1000;
        if due - self.stepped > MAX_CATCH_UP {
            self.stepped = due - MAX_CATCH_UP;
        }
        if self.mode != Mode::Siege {
            self.stepped = due;
            return;
        }
        while self.stepped < due {
            self.stepped += 1;
            self.tick();
        }
    }

    /// Sixteenths of a tick from the last tick run to the key press; on
    /// the tick (`None`) when either is unknown.
    fn press_part(&self, press_us: Option<i64>) -> Option<u8> {
        match (press_us, self.clock) {
            (Some(press), Some(start)) => {
                let basis = start * 1000 + self.stepped * 1_000_000 / TUNE_TICKS;
                Some(super::arena::release_part(press, basis, TUNE_TICKS))
            }
            _ => None,
        }
    }

    /// One physics tick of the siege, and the sounds it makes.
    pub fn tick(&mut self) {
        if let Some(t) = self.swing_tick() {
            if !self.released && t >= Sandbox::swing_end() {
                self.release(None);
            }
        }
        let events = self.world.step();
        let mut breaks = 0;
        for e in events {
            match e {
                Event::Broke { segment, cause } => {
                    breaks += 1;
                    match cause {
                        BreakCause::Impact => {
                            let m = self.world.segments[segment].material;
                            self.cue(impact(m), 0);
                        }
                        BreakCause::Strain => self.cue("segment_snap", 0),
                        BreakCause::Fire => {}
                    }
                }
                Event::BodyHit { body, .. } | Event::BodyCrushed { body } | Event::BodyFell { body, .. } => {
                    if let Some(&k) = self.ranks.get(body) {
                        self.cue(yelp(k), 0);
                    }
                }
            }
        }
        if breaks >= COLLAPSE_MIN {
            self.cue("collapse", breaks);
        }
        for i in 0..self.world.projectiles.len() {
            let b = self.world.projectiles[i];
            if b.alive
                && self
                    .plot
                    .in_water((b.pos.x.0 / CELL_TO_RAW) as i32, (b.pos.y.0 / CELL_TO_RAW) as i32)
            {
                self.world.projectiles[i].alive = false;
                self.cue("splash", 0);
            }
        }
        if let Some(i) = self.shot {
            let b = self.world.projectiles[i];
            if b.alive {
                if b.age.is_multiple_of(TRAIL_EVERY) {
                    self.path.push(b.pos);
                }
            } else if !self.world.projectiles.iter().any(|p| p.alive) {
                self.trail = std::mem::take(&mut self.path);
                self.trail.push(b.pos);
                self.shot = None;
                self.swing = None;
                self.released = false;
            }
        }
        self.move_eye();
    }

    // ---- build mode ----

    fn pick(&mut self, k: usize) {
        match self.mode {
            Mode::Build => {
                if let Some(&t) = TOOLS.get(k) {
                    self.tool = t;
                }
            }
            Mode::Siege => {
                if k < AMMO.len() && !self.released && self.swing.is_none() {
                    self.ammo = k;
                }
            }
        }
    }

    pub fn set_tool(&mut self, value: &str) -> Result<(), String> {
        let k = ["wood", "stone", "metal", "rope", "royal"]
            .iter()
            .position(|n| *n == value)
            .ok_or_else(|| format!("\"{value}\" is not a tool."))?;
        self.tool = TOOLS[k];
        Ok(())
    }

    pub fn set_ammo(&mut self, value: &str) -> Result<(), String> {
        let k = AMMO
            .iter()
            .position(|a| a.name == value)
            .ok_or_else(|| format!("\"{value}\" is not a shot."))?;
        self.pick_ammo(k);
        Ok(())
    }

    fn pick_ammo(&mut self, k: usize) {
        if !self.released && self.swing.is_none() {
            self.ammo = k;
        }
    }

    pub fn set_air(&mut self, value: &str) -> Result<(), String> {
        let k = AIR
            .iter()
            .position(|a| a.1 == value)
            .ok_or_else(|| format!("\"{value}\" is not a trick."))?;
        self.air(k);
        Ok(())
    }

    pub fn toggle_mode(&mut self) {
        self.rebuild();
        self.drag = None;
        // a new mode's view starts where it wants to be
        self.eye = None;
        match self.mode {
            Mode::Build => {
                self.mode = Mode::Siege;
                self.test = None;
                self.cue("horn_siege", 0);
            }
            Mode::Siege => {
                self.mode = Mode::Build;
                self.cue("horn_build", 0);
            }
        }
    }

    pub fn run_test(&mut self) {
        if self.mode != Mode::Build {
            return;
        }
        self.rebuild();
        self.test = Some(self.world.static_test(TEST_TICKS).max_strain);
    }

    fn max_len(&self) -> Fx {
        match self.tool {
            Tool::Beam(m) => TUNE.material(m).max_len,
            Tool::Royal => Fx::ZERO,
        }
    }

    fn in_zone(&self, p: Vec2) -> bool {
        let (l, r, top) = self.zone();
        let g = self.ground(p.x);
        p.x >= l && p.x <= r && p.y <= top && p.y >= g - TUNE.grid / 2
    }

    fn is_node(&self, p: Vec2) -> bool {
        self.beams.iter().any(|b| b.from == p || b.to == p)
    }

    /// A segment may start at a placed node or on the ground.
    fn can_start(&self, p: Vec2) -> bool {
        self.in_zone(p) && (self.is_node(p) || p.y <= self.ground(p.x) + TUNE.grid / 2)
    }

    /// Where a drag from `a` toward `to` ends: snapped, and pulled back
    /// inside the material's reach.
    pub fn drag_end(&self, a: Vec2, to: Vec2) -> Vec2 {
        reach_end(a, to, self.max_len())
    }
}

/// Where a drag from `a` toward `to` ends for a piece at most `max` long:
/// snapped, and pulled back inside the reach.
pub(super) fn reach_end(a: Vec2, to: Vec2, max: Fx) -> Vec2 {
    {
        let b = Vec2::new(snap(to.x), snap(to.y));
        if (b - a).len() <= max {
            return b;
        }
        let d = (to - a).normalized() * max;
        let mut end = Vec2::new(toward(a.x + d.x, a.x), toward(a.y + d.y, a.y));
        let mut guard = 0;
        while (end - a).len() > max && guard < 64 {
            let dx = end.x - a.x;
            let dy = end.y - a.y;
            if dx.abs() >= dy.abs() {
                end.x -= if dx > Fx::ZERO { TUNE.grid } else { -TUNE.grid };
            } else {
                end.y -= if dy > Fx::ZERO { TUNE.grid } else { -TUNE.grid };
            }
            guard += 1;
        }
        end
    }
}

impl Sandbox {
    fn can_place(&self, a: Vec2, b: Vec2) -> bool {
        a != b
            && self.in_zone(b)
            && !self
                .beams
                .iter()
                .any(|p| (p.from == a && p.to == b) || (p.from == b && p.to == a))
    }

    /// The placed segment nearest `p`, within a grid step.
    fn beam_near(&self, p: Vec2, floors_only: bool) -> Option<(usize, Fx)> {
        let mut best: Option<(usize, Fx, Fx)> = None;
        for (i, b) in self.beams.iter().enumerate() {
            let d = b.to - b.from;
            if floors_only && (b.material == Material::Rope || d.x.abs() < d.y.abs()) {
                continue;
            }
            let l2 = d.len2();
            if l2 == Fx::ZERO {
                continue;
            }
            let u = ((p - b.from).dot(d) / l2).clamp_to(Fx::ZERO, Fx::ONE);
            let dist = (p - (b.from + d * u)).len();
            if dist <= TUNE.grid && best.is_none_or(|(_, _, bd)| dist < bd) {
                best = Some((i, u, dist));
            }
        }
        best.map(|(i, u, _)| (i, u))
    }

    fn seat_pos(&self, seat: &Seat) -> Vec2 {
        let b = self.beams[seat.beam];
        Vec2::lerp(b.from, b.to, seat.u)
    }

    fn spacing(&self) -> Fx {
        SEGMENT_LENGTH * crate::game::tune::ROYAL_SPACING_SEGMENTS as i64
    }

    fn spaced(&self, p: Vec2) -> bool {
        let min = self.spacing();
        self.royals.iter().all(|s| (self.seat_pos(s) - p).len() >= min)
    }

    fn sell(&mut self, p: Vec2) {
        if let Some(k) = self.royals.iter().position(|s| {
            // the royal's capsule (R1.2504): its feet on the seat
            let foot = self.seat_pos(s) + Vec2::new(Fx::ZERO, ROYAL_RADIUS);
            let near = Vec2::new(foot.x, p.y.clamp_to(foot.y, foot.y + ROYAL_UPRIGHT));
            (near - p).len() <= ROYAL_RADIUS
        }) {
            self.royals.remove(k);
        } else if let Some((i, _)) = self.beam_near(p, false) {
            self.beams.remove(i);
            self.royals.retain(|s| s.beam != i);
            for s in self.royals.iter_mut() {
                if s.beam > i {
                    s.beam -= 1;
                }
            }
        } else {
            return;
        }
        self.test = None;
        self.rebuild();
        self.cue("sell_segment", 0);
    }

    /// A press on the field: `x,y,button` in pixels (button 2 sells).
    pub fn down(&mut self, value: &str) -> Result<(), String> {
        let mut parts = value.split(',');
        let x = num(parts.next())?;
        let y = num(parts.next())?;
        let button = num(parts.next()).unwrap_or(0);
        if self.mode != Mode::Build {
            return Ok(());
        }
        let p = self.camera().world(x, y);
        self.cursor = Some(p);
        if button == 2 {
            self.sell(p);
            return Ok(());
        }
        match self.tool {
            Tool::Beam(_) => {
                let s = Vec2::new(snap(p.x), snap(p.y));
                if self.can_start(s) {
                    self.drag = Some(s);
                }
            }
            Tool::Royal => {
                if let Some((beam, u)) = self.beam_near(p, true) {
                    let b = self.beams[beam];
                    if self.spaced(Vec2::lerp(b.from, b.to, u)) {
                        let rank = RANKS[self.royals.len() % RANKS.len()];
                        self.royals.push(Seat { beam, u, rank });
                        self.test = None;
                        self.rebuild();
                        self.cue("place_segment", 0);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn moved(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        self.cursor = Some(self.camera().world(x, y));
        Ok(())
    }

    pub fn up(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        let p = self.camera().world(x, y);
        self.cursor = Some(p);
        let Some(a) = self.drag.take() else {
            return Ok(());
        };
        let Tool::Beam(material) = self.tool else {
            return Ok(());
        };
        let b = self.drag_end(a, p);
        if self.can_place(a, b) {
            self.beams.push(Placed {
                from: a,
                to: b,
                material,
            });
            self.test = None;
            self.rebuild();
            self.cue("place_segment", 0);
        }
        Ok(())
    }

    // ---- what the page draws ----

    pub fn frame(&self) -> Frame {
        let cam = self.camera();
        let (ground, water) = ground_line(&self.plot, &cam);
        let build = self.mode == Mode::Build;
        let (beams, nodes, shots, debris) = world_parts(&self.world, &cam, self.test.as_deref(), build);
        let royals = self
            .world
            .bodies
            .iter()
            .zip(self.ranks.iter())
            .map(|(b, &k)| {
                let p = cam.px(b.pos);
                Figure {
                    x: p[0],
                    y: p[1],
                    r: cam.len(b.radius, 3),
                    tall: cam.figure_tall(),
                    rank: rank_name(k),
                    dead: !b.alive,
                }
            })
            .collect();
        let zone = build.then(|| {
            let (l, r, top) = self.zone();
            let lt = cam.px(Vec2::new(l, top));
            let rb = cam.px(Vec2::new(r, self.ground(l)));
            [lt[0], lt[1], rb[0], rb[1]]
        });
        let (preview, ring) = if build {
            self.build_marks(&cam)
        } else {
            (None, None)
        };
        let mut loops = Vec::new();
        if self.shot_alive() {
            loops.push("shot_whoosh");
        }
        if self.world.segments.iter().any(|s| !s.broken && s.burning()) {
            loops.push("fire_crackle");
        }
        Frame {
            mode: if build { "build" } else { "siege" },
            size: [self.size.0, self.size.1],
            cloth: cloth_at(&cam),
            ground,
            water,
            trees: tree_marks(&self.plot, &cam),
            zone,
            grid: if build { cam.len(TUNE.grid, 1) } else { 0 },
            beams,
            nodes,
            royals,
            shots,
            debris,
            engine: self.engine(&cam),
            trail: self.trail.iter().map(|&p| cam.px(p)).collect(),
            preview,
            ring,
            flag: Some(self.flag()),
            spots: Vec::new(),
            inset: None,
            cues: self.cues.clone(),
            music: if build { "music_build" } else { "music_siege" },
            loops,
        }
    }

    fn build_marks(&self, cam: &Camera) -> (Option<Preview>, Option<Ring>) {
        let Some(c) = self.cursor else {
            return (None, None);
        };
        match self.tool {
            Tool::Beam(m) => {
                let Some(a) = self.drag else {
                    let s = Vec2::new(snap(c.x), snap(c.y));
                    let p = cam.px(s);
                    let r = cam.len(TUNE.grid / 2, 3);
                    let ring = Ring {
                        x: p[0],
                        y: p[1],
                        r,
                        ok: self.can_start(s),
                    };
                    return (None, Some(ring));
                };
                let b = self.drag_end(a, c);
                let pa = cam.px(a);
                let preview = Preview {
                    a: pa,
                    b: cam.px(b),
                    w: cam.len(TUNE.material(m).thickness, 2),
                    material: material_name(m),
                    ok: self.can_place(a, b),
                    len: 0,
                }
                .measured();
                let ring = Ring {
                    x: pa[0],
                    y: pa[1],
                    r: cam.len(TUNE.material(m).max_len, 4),
                    ok: true,
                };
                (Some(preview), Some(ring))
            }
            Tool::Royal => {
                let at = self
                    .beam_near(c, true)
                    .map(|(i, u)| Vec2::lerp(self.beams[i].from, self.beams[i].to, u));
                let p = cam.px(at.unwrap_or(c));
                let ring = Ring {
                    x: p[0],
                    y: p[1],
                    r: cam.len(self.spacing(), 4),
                    ok: at.is_some_and(|q| self.spaced(q)),
                };
                (None, Some(ring))
            }
        }
    }

    fn engine(&self, cam: &Camera) -> Engine {
        let phase = match (self.swing_tick(), self.released) {
            (None, _) => Arm::Rest,
            (Some(t), false) => Arm::Swing(t),
            (Some(t), true) => Arm::After {
                at: self.release_at,
                since: t.saturating_sub(self.release_at),
            },
        };
        let ammo = AMMO[self.ammo];
        let shot = (self.mode == Mode::Siege).then_some((ammo.radius, ammo.fire));
        Engine::of(&pose(self.engine_base(), phase, false), shot, cam)
    }

    /// The wind flag at the top right, in pixels: longer for stronger wind,
    /// pointing the way it blows, hanging down when calm.
    fn flag(&self) -> Flag {
        let (w, _) = self.size;
        let x = w - 90;
        let top = [x, 70];
        let pole = [top, [x, 160]];
        let len = 24 + 22 * self.wind_level.abs();
        let cloth = if self.wind_level == 0 {
            vec![top, [x + 10, 110], [x, 110]]
        } else {
            let dir = self.wind_level.signum();
            vec![top, [x + dir * len, 82], [x, 94]]
        };
        Flag { pole, cloth }
    }

    pub fn status_key(&self) -> &'static str {
        match (self.mode, self.swing, self.released) {
            (Mode::Siege, None, _) => "sandbox.swing",
            (Mode::Siege, Some(_), false) => "sandbox.throw",
            _ => "",
        }
    }

    /// The panels over the field: the top row and the bottom bar.
    pub fn panels(&self, s: &Strings) -> Vec<Panel> {
        let button = |id: &str, key: &str| Item::Button {
            tip: String::new(),
            icon: String::new(),
            id: id.into(),
            text: s.get(key),
            quiet: false,
            disabled: false,
        };
        let mut top = vec![match self.mode {
            Mode::Build => button("sandbox.mode", "sandbox.siege"),
            Mode::Siege => button("sandbox.mode", "sandbox.build"),
        }];
        if self.mode == Mode::Build {
            top.push(button("sandbox.test", "sandbox.test"));
        }
        top.push(Item::Button {
            tip: String::new(),
            icon: String::new(),
            id: "sandbox.leave".into(),
            text: s.get("play.leave"),
            quiet: true,
            disabled: false,
        });
        let key = |k: usize| (k + 1).to_string();
        let mut bar = Vec::new();
        match self.mode {
            Mode::Build => {
                let names = ["wood", "stone", "metal", "rope", "royal"];
                bar.push(Item::Bar {
                    id: "sandbox.tool".into(),
                    options: names
                        .iter()
                        .enumerate()
                        .map(|(k, n)| BarOption {
                            tip: String::new(),
                            icon: String::new(),
                            icon_name: String::new(),
                            count: String::new(),
                            cost: String::new(),
                            group: String::new(),
                            value: n.to_string(),
                            text: s.get(&format!("tool.{n}")),
                            key: key(k),
                            selected: TOOLS[k] == self.tool,
                            disabled: false,
                        })
                        .collect(),
                });
            }
            Mode::Siege => {
                let busy = self.swing.is_some() || self.released;
                bar.push(Item::Bar {
                    id: "sandbox.ammo".into(),
                    options: AMMO
                        .iter()
                        .enumerate()
                        .map(|(k, a)| BarOption {
                            tip: String::new(),
                            icon: String::new(),
                            icon_name: String::new(),
                            count: String::new(),
                            cost: String::new(),
                            group: String::new(),
                            value: a.name.to_string(),
                            text: s.get(&format!("ammo.{}", a.name)),
                            key: key(k),
                            selected: k == self.ammo,
                            disabled: busy && k != self.ammo,
                        })
                        .collect(),
                });
                let live = self.shot_alive() && !self.effect_used;
                bar.push(Item::Bar {
                    id: "sandbox.air".into(),
                    options: AIR
                        .iter()
                        .zip(KEY_AIR.iter())
                        .map(|((_, n, _), (code, _))| BarOption {
                            tip: String::new(),
                            icon: String::new(),
                            icon_name: String::new(),
                            count: String::new(),
                            cost: String::new(),
                            group: String::new(),
                            value: n.to_string(),
                            text: s.get(&format!("air.{n}")),
                            key: code.trim_start_matches("Key").to_string(),
                            selected: false,
                            disabled: !live,
                        })
                        .collect(),
                });
            }
        }
        let status = self.status_key();
        if !status.is_empty() {
            top.push(Item::Lines {
                id: "sandbox.hint".into(),
                lines: vec![s.get(status)],
            });
        }
        vec![
            Panel::new("field-top", String::new(), top),
            Panel::new("field-bar", String::new(), bar),
        ]
    }
}

/// Ticks per second.
const TUNE_TICKS: i64 = crate::physics::tune::TICKS_PER_SECOND as i64;

/// `v` on the grid, rounded toward `origin`.
pub(super) fn toward(v: Fx, origin: Fx) -> Fx {
    let g = TUNE.grid.0;
    if v >= origin {
        Fx(v.0.div_euclid(g) * g)
    } else {
        Fx(-((-v.0).div_euclid(g) * g))
    }
}

pub(super) fn num(part: Option<&str>) -> Result<i32, String> {
    let p = part.ok_or("A position needs x and y.")?;
    // a browser may hand in a fraction of a pixel: the whole part is enough
    p.trim()
        .split('.')
        .next()
        .unwrap_or("")
        .parse::<i32>()
        .map_err(|_| format!("\"{p}\" is not a whole number."))
}

pub(super) fn pair(value: &str) -> Result<(i32, i32), String> {
    let mut parts = value.split(',');
    Ok((num(parts.next())?, num(parts.next())?))
}

#[cfg(test)]
mod tests;
