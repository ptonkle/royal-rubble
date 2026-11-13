//! The playfield's frame: everything the page draws on the canvas, already in
//! screen pixels. The core decides where every line goes, how thick it is and
//! what it is made of; the page's renderer (`web/field-draw.js`) only picks a
//! color or a texture for each kind. Built from a physics `World` and a
//! terrain `Plot`, so the sandbox and a match share one describer.

use serde::Serialize;

use crate::physics::{Fx, Material, Vec2, World, TUNE};
use crate::terrain::Plot;

/// Fixed-point raw units per terrain cell unit: a cell is one world unit
/// (1024 cell units become 65536 raw, `Fx::ONE`), as play/ has it (R1.700).
pub const CELL_TO_RAW: i64 = 64;

/// A terrain height or column edge (cell units) as world units.
pub fn cell_units(v: i32) -> Fx {
    Fx(v as i64 * CELL_TO_RAW)
}

/// The physics ground for a plot: one column per terrain column.
pub fn terrain_of(plot: &Plot) -> crate::physics::Terrain {
    crate::physics::Terrain {
        col_width: cell_units(crate::terrain::COL_W),
        heights: plot.heights.iter().map(|&h| cell_units(h)).collect(),
    }
}

/// Where the view looks: world point at the screen's bottom left, and pixels
/// per world unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    pub left: Fx,
    pub bottom: Fx,
    pub ppu: Fx,
    pub w: i32,
    pub h: i32,
}

impl Camera {
    /// A view `width` world units wide whose left edge is `left` and whose
    /// bottom is `bottom`.
    pub fn spanning(left: Fx, width: Fx, bottom: Fx, w: i32, h: i32) -> Camera {
        let ppu = Fx::from_int(w as i64) / width.max(Fx::ONE);
        Camera {
            left,
            bottom,
            ppu,
            w,
            h,
        }
    }

    /// The view's height in world units.
    pub fn height(&self) -> Fx {
        Fx::from_int(self.h as i64) / self.ppu
    }

    pub fn width(&self) -> Fx {
        Fx::from_int(self.w as i64) / self.ppu
    }

    pub fn px(&self, p: Vec2) -> [i32; 2] {
        let x = ((p.x - self.left) * self.ppu).floor_int();
        let y = ((p.y - self.bottom) * self.ppu).floor_int();
        [x as i32, self.h - y as i32]
    }

    /// A length in pixels, at least `min`.
    pub fn len(&self, l: Fx, min: i32) -> i32 {
        ((l * self.ppu).floor_int() as i32).max(min)
    }

    /// A view of a castle box (`left..right` wide, `ground..top` high) so
    /// the castle fills at least `share` (num, den) of the screen's height
    /// where its width allows: centered, `margin` each side, the ground a
    /// sixth up from the bottom (R1.1454).
    #[allow(clippy::too_many_arguments)]
    pub fn framing(
        left: Fx,
        right: Fx,
        ground: Fx,
        top: Fx,
        margin: Fx,
        share: (i64, i64),
        w: i32,
        h: i32,
    ) -> Camera {
        let tall = (top - ground).max(Fx::ONE);
        // widest view that keeps the castle at the share of the height
        let max_w = tall * Fx::from_int(w as i64 * share.1) / Fx::from_int(h as i64 * share.0);
        let width = (right - left + margin * 2)
            .min(max_w)
            .max(right - left + margin / 2);
        let mid = (left + right) / 2;
        let mut cam = Camera::spanning(mid - width / 2, width, Fx::ZERO, w, h);
        cam.bottom = ground - cam.height() / 6;
        cam
    }

    /// One step of easing toward `to`: a `1/div` share of the way, so views
    /// glide instead of jumping. A different screen size snaps.
    pub fn ease_toward(self, to: Camera, div: i64) -> Camera {
        if self.w != to.w || self.h != to.h {
            return to;
        }
        let step = |a: Fx, b: Fx| {
            let d = (b - a) / div;
            if d == Fx::ZERO {
                b
            } else {
                a + d
            }
        };
        Camera {
            left: step(self.left, to.left),
            bottom: step(self.bottom, to.bottom),
            ppu: step(self.ppu, to.ppu),
            w: to.w,
            h: to.h,
        }
    }

    pub fn world(&self, x: i32, y: i32) -> Vec2 {
        Vec2::new(
            self.left + Fx::from_int(x as i64) / self.ppu,
            self.bottom + Fx::from_int((self.h - y) as i64) / self.ppu,
        )
    }
}

pub fn material_name(m: Material) -> &'static str {
    match m {
        Material::Wood => "wood",
        Material::Stone => "stone",
        Material::Metal => "metal",
        Material::Rope => "rope",
    }
}

/// One segment as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Beam {
    pub a: [i32; 2],
    pub b: [i32; 2],
    /// line width, pixels
    pub w: i32,
    pub material: &'static str,
    /// the distance from `a` to `b`, pixels (the page places textures by it)
    pub len: i32,
    /// strain after a Test, 0 (easy) to 4 (broke); absent when not shown
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strain: Option<u8>,
    #[serde(skip_serializing_if = "is_false")]
    pub burning: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A circle: a node, a shot, a piece of debris.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dot {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    /// a shot on fire, a node on the ground
    #[serde(skip_serializing_if = "is_false")]
    pub hot: bool,
}

/// A royal as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Figure {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    /// "squire", "knight", ... for the renderer's look
    pub rank: &'static str,
    #[serde(skip_serializing_if = "is_false")]
    pub dead: bool,
}

/// The trebuchet, as pixel points (R1.1603): a wheeled base, the A-frame,
/// the arm on its pivot, the counterweight box swinging on its hinge, and
/// the sling with the shot in its pouch until release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Engine {
    /// the base beam's two ends (the uprights' feet), and the wheels under them
    pub feet: [[i32; 2]; 2],
    pub wheels: [[i32; 2]; 2],
    pub wheel_r: i32,
    pub pivot: [i32; 2],
    /// the brace across the two uprights
    pub brace: [[i32; 2]; 2],
    /// the long arm's tip and the short arm's end, where the box hangs
    pub tip: [i32; 2],
    pub weight: [i32; 2],
    /// the counterweight box's corners, in order
    #[serde(rename = "box")]
    pub box_: [[i32; 2]; 4],
    /// the sling: tip, middle, pouch
    pub sling: [[i32; 2]; 3],
    pub arm_w: i32,
    pub rope_w: i32,
    /// the sling rope, thicker so it reads in the whip (R1.1800)
    pub sling_w: i32,
    /// pixel lengths of the two uprights (feet to pivot), the base (foot to
    /// foot), the arm (weight to tip) and the brace, for placing textures
    pub lens: [i32; 5],
    /// the shot in the pouch, waiting to be thrown
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded: Option<Dot>,
}

/// The segment being dragged out in build mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub a: [i32; 2],
    pub b: [i32; 2],
    pub w: i32,
    pub material: &'static str,
    /// false when it cannot be placed there
    pub ok: bool,
    /// the distance from `a` to `b`, pixels
    pub len: i32,
}

/// The distance between two pixel points, rounded down.
pub fn px_len(a: [i32; 2], b: [i32; 2]) -> i32 {
    let dx = (b[0] - a[0]) as i64;
    let dy = (b[1] - a[1]) as i64;
    crate::physics::fixed::isqrt((dx * dx + dy * dy) as u64) as i32
}

impl Engine {
    /// The drawing of a trebuchet pose; `shot` is the loaded shot's radius
    /// and whether it burns.
    pub fn of(p: &crate::physics::Pose, shot: Option<(Fx, bool)>, cam: &Camera) -> Engine {
        let feet = p.base.map(|v| cam.px(v));
        let pivot = cam.px(p.pivot);
        let tip = cam.px(p.tip);
        let weight = cam.px(p.short);
        let brace = p.brace.map(|v| cam.px(v));
        let sling = p.sling.map(|v| cam.px(v));
        Engine {
            feet,
            wheels: p.wheels.map(|v| cam.px(v)),
            wheel_r: cam.len(p.wheel_r, 2),
            pivot,
            brace,
            tip,
            weight,
            box_: p.box_.map(|v| cam.px(v)),
            sling,
            arm_w: cam.len(Fx::ratio(1, 6), 3),
            rope_w: cam.len(Fx::ratio(1, 24), 1),
            sling_w: cam.len(Fx::ratio(1, 10), 2),
            lens: [
                px_len(feet[0], pivot),
                px_len(feet[1], pivot),
                px_len(feet[0], feet[1]),
                px_len(weight, tip),
                px_len(brace[0], brace[1]),
            ],
            loaded: shot.filter(|_| p.loaded).map(|(r, hot)| Dot {
                x: sling[2][0],
                y: sling[2][1],
                r: cam.len(r, 2),
                hot,
            }),
        }
    }
}

impl Preview {
    /// Fills in `len` from the ends.
    pub fn measured(mut self) -> Preview {
        self.len = px_len(self.a, self.b);
        self
    }
}

/// A circle around a point: the reach of a segment, a royal's spacing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ring {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    pub ok: bool,
}

/// The wind flag, in screen space: a pole and the cloth's outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flag {
    pub pole: [[i32; 2]; 2],
    pub cloth: Vec<[i32; 2]>,
}

/// A sound to play once. `n` counts up, so the page plays each cue once
/// however many frames show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cue {
    pub n: u32,
    pub name: &'static str,
    #[serde(skip_serializing_if = "is_zero")]
    pub count: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Everything on the canvas, in pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Frame {
    /// "build" or "siege"
    pub mode: &'static str,
    pub size: [i32; 2],
    /// the ground's top edge, left to right; the page fills below it
    pub ground: Vec<[i32; 2]>,
    /// the water's surface: left and right ends
    pub water: Vec<[i32; 2]>,
    /// the buildable area: left, top, right, bottom
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<[i32; 4]>,
    /// the snap grid's spacing in pixels, drawn faintly in the zone
    #[serde(skip_serializing_if = "is_zero_i")]
    pub grid: i32,
    pub beams: Vec<Beam>,
    pub nodes: Vec<Dot>,
    pub royals: Vec<Figure>,
    pub shots: Vec<Dot>,
    pub debris: Vec<Dot>,
    pub engine: Engine,
    /// the last shot's path
    pub trail: Vec<[i32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ring: Option<Ring>,
    /// the wind flag; absent in a corner window
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag: Option<Flag>,
    /// the map's spots along the ground, for placing buildings
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spots: Vec<SpotMark>,
    /// a small second view drawn over this one (your own castle in a siege)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inset: Option<Box<Inset>>,
    pub cues: Vec<Cue>,
    /// the music to play: a track name
    pub music: &'static str,
    /// the looping effects running now
    pub loops: Vec<&'static str>,
}

/// A spot on the map, as a band along the ground from `a` to `b`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpotMark {
    pub a: [i32; 2],
    pub b: [i32; 2],
    pub w: i32,
    /// "gold_vein", "forest", ... for the renderer's color
    pub kind: &'static str,
}

/// A second frame drawn in a box at `at` (its top left, pixels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inset {
    pub at: [i32; 2],
    pub frame: Frame,
}

fn is_zero_i(v: &i32) -> bool {
    *v == 0
}

/// The ground line and the water across the view.
pub fn ground_line(plot: &Plot, cam: &Camera) -> (Vec<[i32; 2]>, Vec<[i32; 2]>) {
    let col = cell_units(crate::terrain::COL_W);
    let mut ground = Vec::new();
    let first = (cam.left / col).floor_int().max(0) as usize;
    let last = (((cam.left + cam.width()) / col).floor_int() + 2).max(0) as usize;
    let n = plot.heights.len();
    for c in first..=last.min(n) {
        let x = col * c as i64;
        let h = cell_units(plot.heights[c.min(n - 1)]);
        ground.push(cam.px(Vec2::new(x, h)));
    }
    let wl = cell_units(plot.water.level);
    let water = vec![
        cam.px(Vec2::new(col * plot.water.cols.start as i64, wl)),
        cam.px(Vec2::new(col * plot.water.cols.end as i64, wl)),
    ];
    (ground, water)
}

/// Segments, nodes, shots and debris of a world, as drawn. `strain`, when
/// given, is each segment's highest strain from a Test.
pub fn world_parts(
    world: &World,
    cam: &Camera,
    strain: Option<&[Fx]>,
    show_nodes: bool,
) -> (Vec<Beam>, Vec<Dot>, Vec<Dot>, Vec<Dot>) {
    let mut beams = Vec::new();
    for (i, s) in world.segments.iter().enumerate() {
        let tested = strain
            .and_then(|st| st.get(i))
            .map(|&v| strain_bucket(v, s.broken));
        if s.broken && tested.is_none() {
            continue;
        }
        let m = TUNE.material(s.material);
        let (a, b) = (cam.px(world.nodes[s.a].pos), cam.px(world.nodes[s.b].pos));
        beams.push(Beam {
            a,
            b,
            len: px_len(a, b),
            w: cam.len(m.thickness, 2),
            material: material_name(s.material),
            strain: tested,
            burning: s.burning(),
        });
    }
    let mut nodes = Vec::new();
    if show_nodes {
        let r = cam.len(TUNE.grid / 3, 3);
        for n in &world.nodes {
            let p = cam.px(n.pos);
            nodes.push(Dot {
                x: p[0],
                y: p[1],
                r,
                hot: n.anchored,
            });
        }
    }
    let dot = |b: &crate::physics::Ball| {
        let p = cam.px(b.pos);
        Dot {
            x: p[0],
            y: p[1],
            r: cam.len(b.radius, 2),
            hot: b.fire,
        }
    };
    let shots = world.projectiles.iter().filter(|b| b.alive).map(dot).collect();
    let debris = world.debris.iter().filter(|b| b.alive).map(dot).collect();
    (beams, nodes, shots, debris)
}

/// Strain as a step from 0 (easy) to 4 (broke).
pub fn strain_bucket(v: Fx, broken: bool) -> u8 {
    if broken || v > Fx::ONE {
        return 4;
    }
    (v * 4).floor_int().clamp(0, 3) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_framing_keeps_a_castle_over_a_third_of_the_screen_height() {
        // a castle 26 wide and 6 tall on a 1920x1080 screen: 6 / 0.35 = 17.1
        // units of height at most, so at most 30.5 units wide (not 32)
        let cam = Camera::framing(
            Fx::from_int(100),
            Fx::from_int(126),
            Fx::from_int(10),
            Fx::from_int(16),
            Fx::from_int(3),
            (35, 100),
            1920,
            1080,
        );
        let tall = cam.len(Fx::from_int(6), 0);
        assert!(tall * 100 >= 1080 * 35, "castle {tall} px of 1080");
        // the whole castle across
        assert!(cam.px(Vec2::new(Fx::from_int(100), Fx::from_int(10)))[0] >= 0);
        assert!(cam.px(Vec2::new(Fx::from_int(126), Fx::from_int(10)))[0] <= 1920);
    }

    #[test]
    fn camera_eases_part_way_then_arrives() {
        let a = Camera::spanning(Fx::ZERO, Fx::from_int(40), Fx::ZERO, 1920, 1080);
        let b = Camera::spanning(Fx::from_int(60), Fx::from_int(20), Fx::from_int(4), 1920, 1080);
        let one = a.ease_toward(b, 8);
        // a step moves an eighth of the way: 60 / 8 = 7.5 units
        assert_eq!(one.left, Fx::ratio(15, 2));
        let mut c = one;
        for _ in 0..400 {
            c = c.ease_toward(b, 8);
        }
        assert_eq!(c, b);
    }

    #[test]
    fn camera_maps_a_world_point_to_pixels_and_back() {
        // 20 units across 1000 px: 50 px a unit; y grows down the screen
        let cam = Camera::spanning(Fx::from_int(10), Fx::from_int(20), Fx::ZERO, 1000, 500);
        assert_eq!(cam.px(Vec2::ints(12, 3)), [100, 350]);
        assert_eq!(cam.world(100, 350), Vec2::ints(12, 3));
    }

    #[test]
    fn strain_steps_by_quarters_and_a_break_is_the_top() {
        assert_eq!(strain_bucket(Fx::ratio(1, 5), false), 0);
        assert_eq!(strain_bucket(Fx::ratio(3, 5), false), 2);
        assert_eq!(strain_bucket(Fx::ratio(99, 100), false), 3);
        assert_eq!(strain_bucket(Fx::ratio(1, 5), true), 4);
    }

    #[test]
    fn pixel_length_is_the_rounded_down_distance() {
        // a 3-4-5 triangle, and 1 by 1 (1.414...) rounds down to 1
        assert_eq!(px_len([10, 20], [13, 24]), 5);
        assert_eq!(px_len([0, 0], [-1, 1]), 1);
        assert_eq!(px_len([5, 5], [5, 5]), 0);
    }

    #[test]
    fn a_terrain_cell_is_one_world_unit() {
        assert_eq!(cell_units(crate::terrain::CELL), Fx::ONE);
    }
}
