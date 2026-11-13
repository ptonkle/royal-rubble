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

    /// A royal figure's drawn height: world scale, or at least
    /// `FIGURE_MIN_PX` when zoomed far out, never above the head room.
    pub fn figure_tall(&self) -> i32 {
        use crate::play::tune::{FIGURE_MIN_PX, FIGURE_TALL, HEAD_ROOM};
        self.len(FIGURE_TALL, 0)
            .max(FIGURE_MIN_PX.min(self.len(HEAD_ROOM, 0)))
    }

    /// Pixels per world unit, in 256ths (R1.4201).
    pub fn zoom(&self) -> i32 {
        (self.ppu.0 >> 8) as i32
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

    /// The view kept inside the world, `0..right` across (R1.2200): moved
    /// so nothing left of the first column or right of the last shows, and
    /// narrowed to the world's width, bottom kept, when it is wider.
    pub fn kept_in(self, right: Fx) -> Camera {
        let mut cam = if self.width() > right {
            Camera::spanning(Fx::ZERO, right, self.bottom, self.w, self.h)
        } else {
            self
        };
        cam.left = cam.left.min(right - cam.width()).max(Fx::ZERO);
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

/// A scenery tree as drawn: its foot on the ground, its height, its look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tree {
    pub x: i32,
    pub y: i32,
    pub h: i32,
    pub look: u8,
}

/// How tall a scenery tree stands, in cells.
pub const TREE_TALL_CELLS: i32 = 5;

/// The plot's scenery trees inside the view, in pixels.
pub fn tree_marks(plot: &Plot, cam: &Camera) -> Vec<Tree> {
    let col = crate::terrain::COL_W;
    let h = cam.len(cell_units(TREE_TALL_CELLS * crate::terrain::CELL), 8);
    plot.trees()
        .into_iter()
        .map(|(c, look)| {
            let p = cam.px(Vec2::new(
                cell_units(col * c as i32 + col / 2),
                cell_units(plot.heights[c]),
            ));
            Tree {
                x: p[0],
                y: p[1],
                h,
                look,
            }
        })
        .filter(|t| t.x > -t.h && t.x < cam.w + t.h)
        .collect()
}

/// A royal as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Figure {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    /// the figure's drawn height in pixels, feet on the body's bottom
    /// (`play::tune::FIGURE_TALL`, R1.2400)
    pub tall: i32,
    /// "squire", "knight", ... for the renderer's look
    pub rank: &'static str,
    #[serde(skip_serializing_if = "is_false")]
    pub dead: bool,
    /// 1 faces right, -1 left: toward the trebuchet that shoots at this
    /// castle, so the front reads (R1.3602)
    pub face: i8,
    /// a dead royal's tumble: the turn about its feet as cosine and sine in
    /// thousandths, from upright to lying on its back (R1.3603)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<[i32; 2]>,
}

/// Cosine and sine, in thousandths, of each eighth of a quarter turn.
const QUARTER: [[i32; 2]; 9] = [
    [1000, 0],
    [981, 195],
    [924, 383],
    [831, 556],
    [707, 707],
    [556, 831],
    [383, 924],
    [195, 981],
    [0, 1000],
];

/// Ticks a fall takes to lie flat, squared: the tumble speeds up as it goes.
const TUMBLE_SQ: u32 = 50;

/// How far over a royal dead for `ticks` has tumbled: it falls on its back,
/// away from the trebuchet it faces, so the turn's sine has the other sign.
pub fn death_turn(ticks: u32, face: i8) -> [i32; 2] {
    let t = ticks.min(64);
    let step = ((t * t) / TUMBLE_SQ).min(8) as usize;
    let [c, s] = QUARTER[step];
    [c, -s * face as i32]
}

/// The tumble of a body for the frame: none while it lives.
pub fn body_turn(b: &crate::physics::Body, tick: u32, face: i8) -> Option<[i32; 2]> {
    if b.alive {
        return None;
    }
    Some(death_turn(tick.saturating_sub(b.died.unwrap_or(0)), face))
}

/// Which way a royal at pixel x faces: toward the trebuchet at `treb_x`.
pub fn facing(x: i32, treb_x: i32) -> i8 {
    if treb_x < x {
        -1
    } else {
        1
    }
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
    /// pixel lengths of the cross braces (each foot to the brace beam's far
    /// end), the two hinge straps (short arm's end to the box's top
    /// corners) and the sling's two ropes, so the page can draw them as
    /// beams (R1.3606)
    pub struts: [i32; 6],
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
        let box_ = p.box_.map(|v| cam.px(v));
        Engine {
            feet,
            wheels: p.wheels.map(|v| cam.px(v)),
            wheel_r: cam.len(p.wheel_r, 2),
            pivot,
            brace,
            tip,
            weight,
            box_,
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
            struts: [
                px_len(feet[0], brace[1]),
                px_len(feet[1], brace[0]),
                px_len(weight, box_[0]),
                px_len(weight, box_[1]),
                px_len(sling[0], sling[1]),
                px_len(sling[1], sling[2]),
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

/// The build tools' marks over the field (R1.3300).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct BuildMarks {
    /// the buildable area's outline: along the top, down the right side,
    /// back along the ground, up the left side; exactly where pieces fit
    pub zone_edge: Vec<[i32; 2]>,
    /// where the royal being placed may not stand: a circle round each
    /// placed royal's feet, the spacing rule's distance
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keep_out: Vec<Ring>,
    /// the royal being placed, standing where a press would put it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ghost: Option<Figure>,
}

/// Everything on the canvas, in pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Frame {
    /// "build" or "siege"
    pub mode: &'static str,
    pub size: [i32; 2],
    /// where the world's origin falls on screen, so the cloth and its
    /// border scroll with the view (R1.1405)
    pub cloth: [i32; 2],
    /// pixels per world unit, in 256ths, so the ground's texture scales
    /// with the view instead of sliding over it as it zooms (R1.4201)
    pub zoom: i32,
    /// the ground's top edge, left to right; the page fills below it
    pub ground: Vec<[i32; 2]>,
    /// scenery trees standing on the ground
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trees: Vec<Tree>,
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
    /// dust and chips from recent impacts
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub puffs: Vec<Puff>,
    /// the view's offset in pixels while a big hit shakes it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shake: Option<[i32; 2]>,
    /// the build tools' marks, while you build
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildMarks>,
}

/// A puff of dust, and for a broken segment its chips or splinters flying
/// out, drawn over the scene while it fades (R1.3608).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Puff {
    pub x: i32,
    pub y: i32,
    /// the cloud's radius, pixels: it grows as it fades
    pub r: i32,
    /// "dust", "wood", "stone" or "metal"
    pub kind: &'static str,
    /// how much still shows, in thousandths
    pub fade: i32,
    /// the chips' centers, pixels, and their size
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chips: Vec<[i32; 2]>,
    pub chip_r: i32,
}

/// Each chip's way out, in 64ths of a unit a tick: up and to both sides.
const CHIP_WAYS: [(i64, i64); 5] = [(-3, 6), (-1, 8), (2, 7), (4, 5), (-5, 3)];

/// Breaks within this many ticks count toward a shake.
const SHAKE_TICKS: u32 = 10;

/// How many fresh breaks make a big hit that shakes the view.
const SHAKE_BREAKS: u32 = 3;

/// The puffs of a world's recent impacts, and the view's shake: only a big
/// hit (`SHAKE_BREAKS` segments broken within `SHAKE_TICKS`) shakes it, by
/// up to 12 pixels, less as the hit ages, flipping side each tick.
pub fn impact_marks(world: &World, cam: &Camera) -> (Vec<Puff>, Option<[i32; 2]>) {
    use crate::physics::world::{ImpactKind, IMPACT_LIFE};
    let now = world.tick;
    let mut puffs = Vec::new();
    let mut fresh = 0u32;
    let mut youngest = SHAKE_TICKS;
    for m in &world.impacts {
        let age = now.saturating_sub(m.tick).min(IMPACT_LIFE);
        let kind = match m.kind {
            ImpactKind::Dust => "dust",
            ImpactKind::Wood => "wood",
            ImpactKind::Stone => "stone",
            ImpactKind::Metal => "metal",
        };
        if m.kind != ImpactKind::Dust && age < SHAKE_TICKS {
            fresh += 1;
            youngest = youngest.min(age);
        }
        let p = cam.px(m.pos);
        let a = age as i64;
        let chips = if m.kind == ImpactKind::Dust {
            Vec::new()
        } else {
            CHIP_WAYS
                .iter()
                .map(|&(dx, dy)| {
                    cam.px(Vec2::new(
                        m.pos.x + Fx::ratio(dx * a, 64),
                        m.pos.y + Fx::ratio(dy * a, 64) - Fx::ratio(a * a, 400),
                    ))
                })
                .collect()
        };
        puffs.push(Puff {
            x: p[0],
            y: p[1],
            r: cam.len(Fx::HALF + Fx::ratio(a, 30), 4),
            kind,
            fade: 1000 - (age as i32 * 1000) / IMPACT_LIFE as i32,
            chips,
            chip_r: cam.len(Fx::ratio(1, 8), 2),
        });
    }
    let shake = (fresh >= SHAKE_BREAKS).then(|| {
        let amp = (fresh.min(6) as i32 * 2 - youngest as i32).max(1);
        let side = if now.is_multiple_of(2) { 1 } else { -1 };
        [amp * side, -(amp / 2) * side]
    });
    (puffs, shake)
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
    /// your castle was just hit: the frame flashes (R1.3208)
    pub hit: bool,
}

fn is_zero_i(v: &i32) -> bool {
    *v == 0
}

/// Where the world's origin falls on screen: the page lays the cloth from
/// here so it moves with the ground.
pub fn cloth_at(cam: &Camera) -> [i32; 2] {
    cam.px(Vec2::new(Fx::ZERO, Fx::ZERO))
}

/// The world's right edge: the far side of the plot's last column.
pub fn world_right(plot: &Plot) -> Fx {
    cell_units(crate::terrain::COL_W) * plot.heights.len() as i64
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
    fn camera_kept_in_never_shows_past_the_world_edges() {
        // R1.2200: a world 100 units across, views 20 wide at 1000 px
        let world = Fx::from_int(100);
        let at = |left: i64, width: i64| {
            Camera::spanning(
                Fx::from_int(left),
                Fx::from_int(width),
                Fx::from_int(3),
                1000,
                500,
            )
            .kept_in(world)
        };
        // past the left: moved right to 0
        assert_eq!(at(-5, 20).left, Fx::ZERO);
        // past the right: moved left so its right edge is 100
        assert_eq!(at(90, 20).left, Fx::from_int(80));
        // inside: untouched
        assert_eq!(
            at(30, 20),
            Camera::spanning(Fx::from_int(30), Fx::from_int(20), Fx::from_int(3), 1000, 500)
        );
        // wider than the world: narrowed to it, bottom kept
        let wide = at(-50, 200);
        assert_eq!(
            (wide.left, wide.bottom, wide.ppu),
            (Fx::ZERO, Fx::from_int(3), Fx::from_int(10))
        );
    }

    #[test]
    fn cloth_offset_moves_with_the_view() {
        // 100 units across 1000 px: 10 px a unit, the origin at the bottom left
        let a = Camera::spanning(Fx::ZERO, Fx::from_int(100), Fx::ZERO, 1000, 500);
        assert_eq!(cloth_at(&a), [0, 500]);
        // the view 10 units right and 5 up: the cloth slides 100 px left, 50 down
        let b = Camera::spanning(Fx::from_int(10), Fx::from_int(100), Fx::from_int(5), 1000, 500);
        assert_eq!(cloth_at(&b), [-100, 550]);
    }

    #[test]
    fn camera_zoom_counts_pixels_per_unit_in_256ths() {
        // R1.4201: 1000 px over 100 units is 10 px a unit, 2560 256ths;
        // over 40 units, 25 px, 6400
        let a = Camera::spanning(Fx::ZERO, Fx::from_int(100), Fx::ZERO, 1000, 500);
        assert_eq!(a.zoom(), 2560);
        let b = Camera::spanning(Fx::ZERO, Fx::from_int(40), Fx::ZERO, 1000, 500);
        assert_eq!(b.zoom(), 6400);
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
    fn impacts_make_fading_puffs_and_only_a_big_hit_shakes() {
        use crate::physics::world::{Impact, ImpactKind};
        use crate::physics::Terrain;
        // 10 px a unit on a 1000 by 500 view
        let cam = Camera::spanning(Fx::ZERO, Fx::from_int(100), Fx::ZERO, 1000, 500);
        let mut w = World::new(Terrain::flat(80, Fx::ZERO));
        w.tick = 18;
        let at = |kind| Impact {
            pos: Vec2::ints(10, 5),
            tick: 0,
            kind,
        };
        w.impacts = vec![at(ImpactKind::Dust), at(ImpactKind::Stone)];
        let (puffs, shake) = impact_marks(&w, &cam);
        // age 18 of 36: half faded; dust has no chips, stone has five
        assert_eq!(puffs[0].fade, 500);
        assert_eq!(puffs[0].kind, "dust");
        assert!(puffs[0].chips.is_empty());
        assert_eq!(puffs[1].chips.len(), 5);
        // radius 0.5 + 18/30 = 1.1 units; 18/30 is a hair under 0.6 in fixed
        // point, so 10.99 px, rounded down to 10
        assert_eq!(puffs[1].r, 10);
        // the first chip: 10 - 3*18/64 = 9.156 across (91 px), 5 + 6*18/64
        // - 324/400 = 5.878 up (500 - 58.78 = 441.2, px 442)
        assert_eq!(puffs[1].chips[0], [91, 442]);
        // old breaks never shake
        assert_eq!(shake, None);
        // three fresh breaks, the youngest 1 tick old, on an even tick:
        // 3*2 - 1 = 5 px across and half that up
        w.impacts = vec![
            Impact {
                tick: 16,
                ..at(ImpactKind::Wood)
            },
            Impact {
                tick: 16,
                ..at(ImpactKind::Wood)
            },
            Impact {
                tick: 17,
                ..at(ImpactKind::Stone)
            },
        ];
        assert_eq!(impact_marks(&w, &cam).1, Some([5, -2]));
        // two fresh breaks are not a big hit
        w.impacts.pop();
        assert_eq!(impact_marks(&w, &cam).1, None);
    }

    #[test]
    fn a_dead_royal_tumbles_onto_its_back_then_lies_flat() {
        // R1.3603: upright when it dies, an eighth over after 3 ticks (9 / 50
        // rounds down to 0, so still upright), part way at 10 (100 / 50 = 2)
        assert_eq!(death_turn(0, 1), [1000, 0]);
        assert_eq!(death_turn(10, 1), [924, -383]);
        // flat on its back from 20 ticks (400 / 50 = 8), and stays there
        assert_eq!(death_turn(20, 1), [0, -1000]);
        assert_eq!(death_turn(5000, 1), [0, -1000]);
        // facing left, it falls the other way
        assert_eq!(death_turn(20, -1), [0, 1000]);
    }

    #[test]
    fn royals_face_the_trebuchet_on_either_side() {
        // R1.3602: a trebuchet at x 100 on the left; royals right of it face left
        assert_eq!(facing(400, 100), -1);
        // a trebuchet on the right: face right
        assert_eq!(facing(400, 900), 1);
        assert_eq!(facing(50, 100), 1);
    }

    #[test]
    fn a_terrain_cell_is_one_world_unit() {
        assert_eq!(cell_units(crate::terrain::CELL), Fx::ONE);
    }
}
