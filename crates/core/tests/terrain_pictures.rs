//! Debug picture of the seeded terrain: twelve seeds (0 to 11, left to right,
//! top to bottom) drawn from the core's output into notes/terrain/seeds.png,
//! so the variety can be judged by eye. Run it on purpose:
//!
//!     cargo test -p tool_core --test terrain_pictures -- --ignored
//!
//! Key: brown ground, blue water, a red post at the launch column, colored
//! blocks for spots, gray lines for ruin segments, and under each map a bar
//! for the plot (dark for level 1, light for the rest up to level 9). Orange
//! dots: the highest the open country may stand (`Plot::arc_cap`).

use std::fs;
use std::path::Path;
use tool_core::terrain::{GapKind, Plot, SpotKind, CELL};

/// Pixels per column and per cell, in the grid of twelve and in the large pictures.
const PX: i32 = 3;
const PX_LARGE: i32 = 7;
const PANEL_W: i32 = 820;
const PANEL_H: i32 = 270;
const COLS: i32 = 2;
const ROWS: i32 = 6;

struct Image {
    w: i32,
    h: i32,
    px: Vec<[u8; 3]>,
}

impl Image {
    fn set(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 3]) {
        for y in y0.min(y1)..y0.max(y1) {
            for x in x0.min(x1)..x0.max(x1) {
                self.set(x, y, c);
            }
        }
    }
    fn line(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: [u8; 3]) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let x = x0 + (x1 - x0) * i / n;
            let y = y0 + (y1 - y0) * i / n;
            self.rect(x - 1, y - 1, x + 1, y + 1, c);
        }
    }
}

fn spot_color(k: SpotKind) -> [u8; 3] {
    match k {
        SpotKind::StoneDeposit => [150, 150, 160],
        SpotKind::OreDeposit => [120, 60, 40],
        SpotKind::GoldVein => [240, 200, 40],
        SpotKind::HolyWell => [120, 220, 255],
        SpotKind::Forest => [30, 120, 40],
        SpotKind::Ruins => [200, 190, 170],
        SpotKind::Watchtower => [220, 60, 200],
        SpotKind::ArmoryRuin => [200, 40, 40],
    }
}

fn draw(img: &mut Image, p: &Plot, (ox, oy): (i32, i32), (pw, ph): (i32, i32), px: i32) {
    let base = oy + ph - 24;
    // the lowest ground sits a few cells above the panel's foot
    let floor = p.heights.iter().copied().min().unwrap_or(0) - 4 * CELL;
    let y_of = |h: i32| base - (h - floor) * px / CELL;
    img.rect(ox, oy, ox + pw, oy + ph, [200, 225, 245]);
    for (c, &h) in p.heights.iter().enumerate() {
        let x = ox + 4 + c as i32 * px;
        let c_us = c;
        if p.water.cols.contains(c_us) {
            img.rect(x, y_of(p.water.level), x + px, y_of(h), [40, 90, 200]);
        }
        img.rect(x, y_of(h), x + px, base, [120, 85, 50]);
    }
    // the arc cap over the open country, dotted
    for c in (p.between.start..p.between.end).step_by(2) {
        let x = ox + 4 + c as i32 * px;
        let y = y_of(p.arc_cap(c));
        img.rect(x, y, x + 2, y + 2, [240, 140, 40]);
    }
    let lx = ox + 4 + p.launch_col as i32 * px;
    let lh = y_of(p.heights[p.launch_col]);
    img.rect(lx, lh - 16, lx + 3, lh, [220, 30, 30]);
    for s in &p.spots {
        let top = s.cols.start..s.cols.end;
        let hi = top.map(|c| p.heights[c]).max().unwrap_or(0);
        let x0 = ox + 4 + s.cols.start as i32 * px;
        let x1 = ox + 4 + s.cols.end as i32 * px;
        img.rect(x0, y_of(hi) - 6, x1, y_of(hi) - 1, spot_color(s.kind));
        for seg in &s.segments {
            let pt = |n: tool_core::terrain::Node| (ox + 4 + n.x * px / CELL, y_of(n.y));
            img.line(pt(seg.a), pt(seg.b), [90, 90, 90]);
        }
    }
    let one = p.buildable(1);
    let all = p.buildable(9);
    let bx = |c: usize| ox + 4 + c as i32 * px;
    img.rect(bx(all.start), base + 6, bx(all.end), base + 14, [190, 190, 120]);
    img.rect(bx(one.start), base + 6, bx(one.end), base + 14, [90, 90, 40]);
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    /// Writes `n` bits of `v`, least significant first.
    fn put(&mut self, v: u32, n: u32) {
        for i in 0..n {
            self.acc |= ((v >> i) & 1) << self.n;
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    /// Writes a Huffman code of `n` bits, most significant first.
    fn code(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.put((v >> i) & 1, 1);
        }
    }
    fn sym(&mut self, s: u32) {
        match s {
            0..=143 => self.code(0x30 + s, 8),
            144..=255 => self.code(0x190 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0xC0 + s - 280, 8),
        }
    }
}

const LEN_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195,
    227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut b = Bits {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    b.put(1, 1); // final block
    b.put(1, 2); // fixed Huffman
    let mut i = 0;
    while i < data.len() {
        let mut run = 0;
        while i >= 3 && i + run < data.len() && run < 258 && data[i + run] == data[i + run - 3] {
            run += 1;
        }
        if run >= 3 {
            let k = (0..29).rev().find(|&k| LEN_BASE[k] <= run as u32).unwrap_or(0);
            b.sym(257 + k as u32);
            b.put(run as u32 - LEN_BASE[k], LEN_EXTRA[k]);
            b.code(2, 5); // distance 3
            i += run;
        } else {
            b.sym(data[i] as u32);
            i += 1;
        }
    }
    b.sym(256);
    if b.n > 0 {
        b.out.push(b.acc as u8);
    }
    b.out
}

fn png(img: &Image) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..img.h {
        raw.push(0u8);
        for x in 0..img.w {
            raw.extend_from_slice(&img.px[(y * img.w + x) as usize]);
        }
    }
    // zlib: one fixed-Huffman deflate block, repeats coded as matches 3 bytes (one pixel) back
    let mut z = vec![0x78, 0x01];
    z.extend_from_slice(&deflate_fixed(&raw));
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &raw {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(img.w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

#[test]
#[ignore = "writes notes/terrain/seeds.png; run on purpose"]
fn terrain_pictures_of_twelve_seeds() {
    let gap = 8;
    let w = COLS * (PANEL_W + gap);
    let h = ROWS * (PANEL_H + gap);
    let mut img = Image {
        w,
        h,
        px: vec![[255, 255, 255]; (w * h) as usize],
    };
    for seed in 0..(COLS * ROWS) as u64 {
        let i = seed as i32;
        let p = Plot::generate(seed);
        draw(
            &mut img,
            &p,
            ((i % COLS) * (PANEL_W + gap), (i / COLS) * (PANEL_H + gap)),
            (PANEL_W, PANEL_H),
            PX,
        );
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notes/terrain");
    fs::create_dir_all(&dir).expect("notes/terrain");
    fs::write(dir.join("seeds.png"), png(&img)).expect("write seeds.png");
}

#[test]
#[ignore = "writes notes/terrain/large-*.png; run on purpose"]
fn terrain_pictures_large_one_per_gap_kind() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notes/terrain");
    fs::create_dir_all(&dir).expect("notes/terrain");
    for (kind, name) in [
        (GapKind::Moat, "moat"),
        (GapKind::River, "river"),
        (GapKind::Valley, "valley"),
    ] {
        let p = (0..100u64)
            .map(Plot::generate)
            .find(|p| p.gap_kind == kind)
            .expect("a seed with this gap");
        let lo = p.heights.iter().copied().min().unwrap_or(0);
        let cap = (p.between.start..p.between.end).map(|c| p.arc_cap(c)).max();
        let hi = p.heights.iter().copied().max().unwrap_or(0).max(cap.unwrap_or(0));
        let w = p.heights.len() as i32 * PX_LARGE + 8;
        let h = (hi - lo) / CELL * PX_LARGE + 8 * PX_LARGE + 60;
        let mut img = Image {
            w,
            h,
            px: vec![[255, 255, 255]; (w * h) as usize],
        };
        draw(&mut img, &p, (0, 0), (w, h), PX_LARGE);
        fs::write(dir.join(format!("large-{name}-seed-{}.png", p.seed)), png(&img)).expect("write");
    }
}
