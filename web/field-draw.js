// The playfield's renderer: the one file a chosen look replaces. It draws
// the core's frame (crates/core/src/app/field.rs) on a 2D canvas as Bayeux
// embroidery by placing pictures baked ahead of time with the stitch helpers
// (tools/look/stitch.js, baked by tools/look/bake_stitch.py into
// data/look/stitch/; sizes and anchors in data/stitch-sprites.json). Every
// position, radius, width and segment length comes from the core in pixels;
// scales from data/stitch.json; wool colors from data/palette.json; strain
// and spot threads from field.css. It decides nothing and does no math of
// its own beyond scaling what it is given.

import { drawTreb } from "./treb-draw.js";

let look = null;
let G = null;
let K = null;
let wool = null;
let sprites = null;
const images = {};
const patterns = {};
const colors = {};
const layers = {};

// field.json, stitch.json, palette.json, stitch-sprites.json; resolves when
// the baked pictures have loaded and been drawn once on `into` (the field's
// own 2D context, cleared again after), so the first view does not stall.
export async function setLook(table, stitch, palette, baked, into) {
  look = table;
  K = stitch;
  G = stitch.game;
  wool = palette.wool;
  const loads = [];
  for (const [name, sp] of Object.entries(baked)) {
    if (!sp.file) continue;
    const img = new Image();
    img.src = sp.file;
    loads.push(img.decode().then(() => createImageBitmap(img)).then((bm) => (images[name] = bm)));
  }
  await Promise.all(loads);
  sprites = baked;
  warm(into);
}

// Draws every picture and pattern once on the field's canvas and reads one
// pixel back, so the GPU has them all before the first view appears and
// the first frame does not stall on uploads (R1.2900).
function warm(g) {
  for (const name of Object.keys(images)) {
    const sp = sprites[name];
    g.drawImage(images[name], 0, 0, sp.w, sp.h);
    g.fillStyle = pattern(g, name);
    g.fillRect(0, 0, sp.w, sp.h);
  }
  g.setLineDash([G.band_dash, G.band_gap]);
  g.beginPath();
  g.moveTo(0, 0);
  g.lineTo(sprites.linen.w, sprites.linen.h);
  g.stroke();
  g.getImageData(0, 0, 1, 1);
  g.setLineDash([]);
  g.clearRect(0, 0, g.canvas.width, g.canvas.height);
  getComputedStyle(document.documentElement).getPropertyValue("--field-bad");
}

function color(name) {
  if (!(name in colors)) colors[name] = getComputedStyle(document.documentElement).getPropertyValue("--" + name).trim();
  return colors[name];
}

function pattern(g, name) {
  if (!patterns[name]) patterns[name] = g.createPattern(images[name], "repeat");
  return patterns[name];
}

// A beam's strip pattern scaled to a width ww, made once per width.
function strip(g, name, ww) {
  const key = name + " " + ww;
  if (!patterns[key]) {
    const k = ww / sprites[name].h;
    patterns[key] = g.createPattern(images[name], "repeat");
    patterns[key].setTransform(new DOMMatrix([k, 0, 0, k, 0, -half(ww)]));
  }
  return patterns[key];
}

const half = (v) => v * K.n.half;
const most = (a, b) => (a > b ? a : b);

// A sprite scaled by k with its anchor on (x, y); face -1 mirrors it.
function place(g, name, x, y, k, face) {
  const sp = sprites[name];
  g.save();
  g.translate(x, y);
  if (face) g.scale(face, 1);
  g.drawImage(images[name], -sp.ax * k, -sp.ay * k, sp.w * k, sp.h * k);
  g.restore();
}

// --- segments: the material's strip laid from a to b, len from the core

function beam(g, a, b, w, len, material, strain) {
  if (len < 1) return;
  const [ax, ay] = a;
  const [bx, by] = b;
  const ux = (bx - ax) / len;
  const uy = (by - ay) / len;
  const name = "beam-" + (K.materials[material] ? material : "wood");
  const sp = sprites[name];
  const ww = most(w, G.beam_min_w);
  const pat = strip(g, name, ww);
  g.save();
  g.transform(ux, uy, -uy, ux, ax, ay);
  g.fillStyle = pat;
  g.fillRect(0, -half(ww), len, ww);
  g.strokeStyle = sp.line;
  g.lineWidth = ww * G.beam_line;
  if (material !== "rope") {
    g.beginPath();
    g.moveTo(0, -half(ww));
    g.lineTo(0, half(ww));
    g.moveTo(len, -half(ww));
    g.lineTo(len, half(ww));
    g.stroke();
  }
  if (strain !== undefined) {
    g.strokeStyle = color("strain-" + strain);
    g.lineWidth = ww * G.beam_line_strain;
    g.setLineDash([G.strain_dash, half(G.strain_dash)]);
    g.strokeRect(0, -half(ww), len, ww);
  }
  g.restore();
}

function flames(g, b) {
  const [ax, ay] = b.a;
  const [bx, by] = b.b;
  if (b.len < 1) return;
  const step = most(b.w, G.beam_min_w) * G.flame_every;
  const k = (most(b.w, G.beam_min_w) * G.flame_h) / (sprites.flame.r + sprites.flame.r);
  for (let t = half(step); t < b.len; t += step) place(g, "flame", ax + ((bx - ax) * t) / b.len, ay + ((by - ay) * t) / b.len, k);
}

function beams(g, f) {
  f.beams.forEach((b) => {
    beam(g, b.a, b.b, b.w, b.len, b.material, b.strain);
    if (b.burning) flames(g, b);
  });
  f.nodes.forEach((n) => place(g, n.hot ? "knot-ochre" : "knot-navy", n.x, n.y, (n.r * G.node_r) / sprites["knot-navy"].r));
}

// --- royals and buildings

function royals(g, f) {
  f.royals.forEach((r) => figure(g, f, r));
}

function figure(g, f, r) {
  const mid = half(f.size[0]);
  const face = r.x > mid ? -1 : 1;
  if (K.huts[r.rank]) {
    const name = "hut-" + r.rank;
    const tall = most(r.r * G.hut_tall_r, G.hut_min_px);
    place(g, name, r.x, r.y + r.r * G.hut_foot_r, tall / sprites[name].tall, face);
    return;
  }
  const rank = K.costumes[r.rank] ? r.rank : "squire";
  const name = "fig-" + rank + (r.dead ? "-dead" : "");
  const k = r.tall / sprites[name].tall;
  if (r.dead) {
    // fallen: turned a quarter onto the floor
    g.save();
    g.globalAlpha = G.dead_alpha;
    g.translate(r.x, r.y + r.r);
    g.transform(0, face, -face, 0, 0, 0);
    place(g, name, 0, 0, k);
    g.restore();
    return;
  }
  place(g, name, r.x, r.y + r.r, k, face);
}

// The zone's outline from the build marks, when there are any.
function edgeOf(f) {
  return f.build ? f.build.zone_edge : null;
}

function trace(g, pts) {
  g.beginPath();
  pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.closePath();
}

// --- the background: linen, terrain, water, the plot (cached per view)

function stroke(g, pts, wool_, w, dash, gap) {
  g.beginPath();
  pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.lineCap = "butt";
  g.lineJoin = "round";
  g.setLineDash([]);
  g.strokeStyle = wool[wool_].dark;
  g.lineWidth = w;
  g.stroke();
  g.setLineDash([dash, gap]);
  g.strokeStyle = wool[wool_].mid;
  g.lineWidth = w * K.stitch.cord_wrap;
  g.stroke();
  g.setLineDash([]);
}

// The background is kept on its own canvas while the view holds still and
// drawn straight onto the field while the view moves, which saves laying it
// on a layer only to copy it once (R1.2902).
function background(out, slot, f) {
  const [w, h] = f.size;
  const key = JSON.stringify([w, h, f.cloth, f.trees, f.ground, f.water, f.zone, f.grid, edgeOf(f)]);
  let L = layers[slot];
  if (!L || L.c.width !== w || L.c.height !== h) {
    L = { c: document.createElement("canvas"), key: "", last: "" };
    L.c.width = w;
    L.c.height = h;
    layers[slot] = L;
  }
  const moving = key !== L.last;
  L.last = key;
  if (key !== L.key && moving) {
    out.save();
    paint(out, slot, f, w, h);
    out.restore();
    return;
  }
  if (key !== L.key) {
    L.key = key;
    const g = L.c.getContext("2d");
    g.clearRect(0, 0, w, h);
    paint(g, slot, f, w, h);
  }
  out.drawImage(L.c, 0, 0);
}

function paint(g, slot, f, w, h) {
  // the cloth is laid from the core's origin point, so it moves with the ground
  const [cx, cy] = f.cloth;
  const lin = pattern(g, "linen");
  lin.setTransform(new DOMMatrix([1, 0, 0, 1, cx, cy]));
  g.fillStyle = lin;
  g.fillRect(0, 0, w, h);
  // the top border band, behind the HUD; the main view only
  if (slot === "main") {
    const bh = h * G.border_share;
    const k = bh / sprites.border.h;
    const bp = pattern(g, "border");
    bp.setTransform(new DOMMatrix([k, 0, 0, k, cx, 0]));
    g.fillStyle = bp;
    g.fillRect(0, 0, w, bh);
  }
  // scenery trees, their feet tucked under the ground's edge
  (f.trees || []).forEach((t) => {
    const name = "tree-" + t.look;
    place(g, name, t.x, t.y, t.h / sprites[name].tall);
  });
  const pts = f.ground;
  if (pts.length > 1) {
    const last = pts[pts.length - 1][0];
    const first = pts[0][0];
    // banded mounds: laid wool under the ground line, stitched edges
    g.save();
    g.beginPath();
    pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
    g.lineTo(last, h);
    g.lineTo(first, h);
    g.closePath();
    g.clip();
    // solid earth down to the bottom, the stitched bands laid over it
    g.fillStyle = pattern(g, "wool-" + K.ground_fill);
    g.fillRect(0, 0, w, h);
    // a band's lower edge is the next band's upper edge, stitched over it,
    // so only the last band's lower edge is sewn (R1.2901)
    const lastBand = K.ground_bands[K.ground_bands.length - 1];
    for (const band of K.ground_bands) {
      const [name, a, b] = band;
      const top = pts.map(([x, y]) => [x, y + G[a]]);
      const bot = pts.map(([x, y]) => [x, y + G[b]]);
      g.beginPath();
      top.concat(bot.slice().reverse()).forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
      g.closePath();
      g.fillStyle = pattern(g, "wool-" + name);
      g.fill();
      stroke(g, top, "navy", G.ground_band_line, G.band_dash, G.band_gap);
      if (band === lastBand) stroke(g, bot, "rule", G.ground_band_line, G.band_dash, G.band_gap);
    }
    g.restore();
    // water: waves above the ground between the banks
    if (f.water.length) {
      const [[ax, ay], [bx]] = f.water;
      g.save();
      g.beginPath();
      pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
      g.lineTo(last, 0);
      g.lineTo(first, 0);
      g.closePath();
      g.clip();
      g.globalAlpha = G.water_alpha;
      g.fillStyle = pattern(g, "wool-bluegreen");
      g.fillRect(ax, ay, bx - ax, h - ay);
      g.globalAlpha = 1;
      const wp = pattern(g, "water");
      wp.setTransform(new DOMMatrix([1, 0, 0, 1, ax, ay]));
      g.fillStyle = wp;
      g.fillRect(ax, ay, bx - ax, h - ay);
      g.restore();
    }
    // the walking line: a row of small humps, then the stem-stitched edge
    const up = pts.map(([x, y]) => [x, y - G.scallop_up]);
    g.beginPath();
    up.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
    g.lineCap = "round";
    g.setLineDash([half(G.scallop_dash), G.scallop_dash]);
    g.strokeStyle = wool.slate.mid;
    g.lineWidth = G.scallop_w;
    g.stroke();
    g.setLineDash([]);
    stroke(g, pts, "rule", G.ground_line, G.ground_dash, G.ground_gap);
  }
  // the plot: a fine running stitch round it and a faint grid
  if (f.zone) {
    const [l, t, r, b] = f.zone;
    const edge = edgeOf(f);
    g.save();
    if (edge) {
      trace(g, edge);
      g.clip();
    }
    if (f.grid > look.grid_min_px) {
      g.globalAlpha = G.grid_alpha;
      g.strokeStyle = color("field-grid-thread");
      g.lineWidth = G.grid_width;
      g.setLineDash([G.grid_run, G.grid_gap]);
      g.beginPath();
      for (let x = l; x <= r; x += f.grid) {
        g.moveTo(x, t);
        g.lineTo(x, b);
      }
      for (let y = b; y >= t; y -= f.grid) {
        g.moveTo(l, y);
        g.lineTo(r, y);
      }
      g.stroke();
    }
    g.globalAlpha = G.zone_alpha;
    g.strokeStyle = color("field-zone-thread");
    g.lineWidth = G.zone_width;
    g.lineCap = "round";
    g.setLineDash([G.zone_run, G.zone_gap]);
    if (edge) {
      trace(g, edge);
      g.stroke();
    } else {
      g.strokeRect(l, t, r - l, b - t);
    }
    g.restore();
  }
}

// --- shots, debris, the trail, small marks

function dashed(g, pts, dark, light, w, len, gap) {
  if (pts.length < K.n.two) return;
  g.save();
  g.beginPath();
  pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.lineCap = "round";
  g.setLineDash([len, gap]);
  g.lineWidth = w;
  g.strokeStyle = dark;
  g.stroke();
  g.lineWidth = half(w);
  g.strokeStyle = light;
  g.stroke();
  g.restore();
}

function balls(g, f) {
  f.debris.forEach((d) => place(g, "debris", d.x, d.y, d.r / sprites.debris.r));
  f.shots.forEach((s) => place(g, s.hot ? "ball-hot" : "ball", s.x, s.y, s.r / sprites.ball.r));
}

function engine(g, f) {
  drawTreb(g, f.engine, { beam, place, dashed, pattern, sprites, wool, look });
}

function ring(g, r) {
  g.save();
  g.beginPath();
  g.arc(r.x, r.y, r.r, 0, look.full_turn);
  g.setLineDash([G.trail_len, G.trail_gap]);
  g.lineWidth = G.ring_w;
  g.strokeStyle = r.ok ? wool.navy.mid : color("field-bad");
  g.stroke();
  g.restore();
}

function marks(g, f) {
  const b = f.build;
  if (b && b.keep_out) b.keep_out.forEach((r) => ring(g, r));
  if (b && b.ghost) {
    g.save();
    g.globalAlpha = G.preview_alpha;
    figure(g, f, b.ghost);
    g.restore();
  }
  if (f.ring) ring(g, f.ring);
  if (f.preview) {
    const p = f.preview;
    g.globalAlpha = G.preview_alpha;
    beam(g, p.a, p.b, p.w, p.len, p.material, p.ok ? undefined : K.n.four);
    g.globalAlpha = 1;
  }
}

function flag(g, f) {
  if (!f.flag) return;
  const [[px, py], [qx, qy]] = f.flag.pole;
  dashed(g, [[px, py], [qx, qy]], wool.navy.dark, wool.navy.light, look.pole_width, look.pole_width, 0);
  g.save();
  g.beginPath();
  f.flag.cloth.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.closePath();
  g.fillStyle = pattern(g, "wool-terracotta");
  g.fill();
  g.setLineDash([G.band_dash, G.band_gap]);
  g.strokeStyle = wool.navy.mid;
  g.lineWidth = look.edge_width;
  g.stroke();
  g.restore();
}

// The map's spots: a band of wool laid along the ground.
function spots(g, f) {
  (f.spots || []).forEach((s) => dashed(g, [s.a, s.b], color("spot-" + s.kind), color("spot-" + s.kind), s.w * G.spot_w, G.zone_run, G.grid_run));
}

// A second view in a framed box (your own castle during a siege).
function inset(g, f) {
  if (!f.inset) return;
  const [x, y] = f.inset.at;
  const [w, h] = f.inset.frame.size;
  g.save();
  g.translate(x, y);
  g.beginPath();
  g.rect(0, 0, w, h);
  g.clip();
  draw(g, f.inset.frame, "inset");
  g.restore();
  g.save();
  g.setLineDash([G.band_dash, G.band_gap]);
  g.strokeStyle = wool.rule.mid;
  g.lineWidth = G.inset_edge;
  g.strokeRect(x, y, w, h);
  g.restore();
}

function draw(g, f, slot) {
  background(g, slot, f);
  engine(g, f);
  spots(g, f);
  dashed(g, f.trail, wool.rule.dark, wool.rule.light, G.trail_w, G.trail_len, G.trail_gap);
  beams(g, f);
  royals(g, f);
  balls(g, f);
  marks(g, f);
  flag(g, f);
}

// Draws one frame on the canvas's 2D context.
export function drawField(g, f) {
  if (!look || !sprites) return;
  // a fresh canvas each frame, so nothing from the last frame or mode stays
  g.clearRect(0, 0, g.canvas.width, g.canvas.height);
  draw(g, f, "main");
  inset(g, f);
}
