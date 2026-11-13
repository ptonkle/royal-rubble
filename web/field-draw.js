// The playfield's renderer: the one file a chosen look replaces. It draws
// the core's frame (crates/core/src/app/field.rs) on a 2D canvas in a rough
// hand-painted look (R1.3600) by placing pictures baked ahead of time: the
// painted set (tools/look/paint.html, data/look/paint/, sizes in
// data/paint-sprites.json, drawing numbers in data/paint.json) laid name for
// name over the older stitched set (tools/look/stitch.js, data/look/stitch/,
// data/stitch-sprites.json) for the pictures not yet repainted. Every
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
let P = null;
const images = {};
const patterns = {};
const colors = {};
const layers = {};

// field.json, stitch.json, palette.json, stitch-sprites.json; resolves when
// the baked pictures have loaded and been drawn once on `into` (the field's
// own 2D context, cleared again after), so the first view does not stall.
export async function setLook(table, stitch, palette, baked, into, paint) {
  look = table;
  P = paint;
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
  const ww = most(w * P.beam_k, G.beam_min_w);
  const pat = strip(g, name, ww);
  g.save();
  g.transform(ux, uy, -uy, ux, ax, ay);
  g.fillStyle = pat;
  g.fillRect(0, -half(ww), len, ww);
  g.strokeStyle = sp.line;
  g.lineWidth = ww * P.beam_end;
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
  f.royals.forEach((r) => {
    const face = r.face;
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
      // tumbling onto its back about its feet, by the core's turn
      const [c, s] = r.turn.map((v) => v * P.per_mille);
      g.save();
      g.globalAlpha = P.dead_alpha;
      g.translate(r.x, r.y + r.r);
      g.transform(c, s, -s, c, 0, 0);
      place(g, name, 0, 0, k, face);
      g.restore();
      return;
    }
    place(g, name, r.x, r.y + r.r, k, face);
  });
}

// --- the background: sky, hills, water, the plot (cached per view)

// The background is kept on its own canvas while the view holds still and
// drawn straight onto the field while the view moves, which saves laying it
// on a layer only to copy it once (R1.2902).
function background(out, slot, f) {
  const [w, h] = f.size;
  const key = JSON.stringify([w, h, f.cloth, f.trees, f.ground, f.water, f.zone, f.grid]);
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

// A terrain tile's pattern, laid from the core's origin point so it moves
// with the ground.
function tiled(g, name, cx, cy) {
  const pat = pattern(g, "ground-" + name);
  pat.setTransform(new DOMMatrix([P.tile_k, 0, 0, P.tile_k, cx, cy]));
  return pat;
}

function path(g, pts) {
  g.beginPath();
  pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
}

function paint(g, slot, f, w, h) {
  // the sky is laid to the screen, so it holds still while the view moves
  g.drawImage(images.sky, 0, 0, w, h);
  const [cx, cy] = f.cloth;
  // scenery trees, their feet tucked under the ground's edge
  (f.trees || []).forEach((t) => {
    const name = "tree-" + t.look;
    place(g, name, t.x, t.y, t.h / sprites[name].tall);
  });
  const pts = f.ground;
  if (pts.length > 1) {
    const last = pts[pts.length - 1][0];
    const first = pts[0][0];
    // the hills: deep earth, then rock, dirt and grass bands under the edge
    g.save();
    path(g, pts);
    g.lineTo(last, h);
    g.lineTo(first, h);
    g.closePath();
    g.clip();
    g.fillStyle = tiled(g, P.ground_fill, cx, cy);
    g.fillRect(0, 0, w, h);
    for (const [name, a, b] of P.ground_bands.slice().reverse()) {
      const top = pts.map(([x, y]) => [x, y + a]);
      const bot = pts.map(([x, y]) => [x, y + b]);
      path(g, top.concat(bot.slice().reverse()));
      g.closePath();
      g.fillStyle = tiled(g, name, cx, cy);
      g.fill();
      path(g, bot);
      g.strokeStyle = P.band_line;
      g.lineWidth = P.band_line_w;
      g.stroke();
    }
    g.restore();
    // water between the banks
    if (f.water.length) {
      const [[ax, ay], [bx]] = f.water;
      g.save();
      path(g, pts);
      g.lineTo(last, 0);
      g.lineTo(first, 0);
      g.closePath();
      g.clip();
      g.globalAlpha = P.water_alpha;
      g.fillStyle = tiled(g, "water", cx, cy);
      g.fillRect(ax, ay, bx - ax, h - ay);
      g.globalAlpha = 1;
      path(g, [[ax, ay], [bx, ay]]);
      g.strokeStyle = P.water_edge;
      g.lineWidth = P.water_edge_w;
      g.stroke();
      g.restore();
    }
    // the grass edge: a dark rim and a lit top
    g.lineCap = "round";
    g.lineJoin = "round";
    path(g, pts);
    g.strokeStyle = P.grass_edge;
    g.lineWidth = P.grass_edge_w;
    g.stroke();
    path(g, pts.map(([x, y]) => [x, y - P.grass_lit_up]));
    g.strokeStyle = P.grass_lit;
    g.lineWidth = P.grass_lit_w;
    g.stroke();
  }
  // the plot: a fine running stitch round it and a faint grid
  if (f.zone) {
    const [l, t, r, b] = f.zone;
    g.save();
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
    g.strokeRect(l, t, r - l, b - t);
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
  f.debris.forEach((d) => place(g, "debris", d.x, d.y, (d.r * P.debris_k) / sprites.debris.r));
  f.shots.forEach((s) => place(g, s.hot ? "ball-hot" : "ball", s.x, s.y, (s.r * P.ball_k) / sprites.ball.r));
}

function engine(g, f) {
  drawTreb(g, f.engine, { beam, place, pattern, sprites, half, P });
}

function marks(g, f) {
  if (f.ring) {
    g.save();
    g.beginPath();
    g.arc(f.ring.x, f.ring.y, f.ring.r, 0, look.full_turn);
    g.setLineDash([G.trail_len, G.trail_gap]);
    g.lineWidth = G.ring_w;
    g.strokeStyle = f.ring.ok ? wool.navy.mid : color("field-bad");
    g.stroke();
    g.restore();
  }
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
// The map's spots (R1.3607): ore veins in the rock under the ground, and
// trees, a well, ruins or a watchtower marker standing on it, each with its
// badge above. The core gives the spot's ends just under the ground line
// and a width that scales with the view.
function spots(g, f) {
  const pts = f.ground;
  const [cx, cy] = f.cloth;
  const [w, h] = f.size;
  (f.spots || []).forEach((s) => {
    const [ax, ay] = s.a;
    const [bx, by] = s.b;
    const at = (t) => [ax + (bx - ax) * t, ay + (by - ay) * t - s.w];
    const vein = P.spot_veins[s.kind];
    if (vein && pts.length > 1) {
      const top = s.w * P.vein_top;
      const deep = s.w * P.vein_depth;
      const ins = s.w * P.vein_inset;
      g.save();
      path(g, pts);
      g.lineTo(w, h);
      g.lineTo(0, h);
      g.closePath();
      g.clip();
      path(g, [[ax, ay + top], [bx, by + top], [bx - ins, by + deep], [ax + ins, ay + deep]]);
      g.closePath();
      const pat = pattern(g, "vein-" + vein);
      pat.setTransform(new DOMMatrix([P.vein_k, 0, 0, P.vein_k, cx, cy]));
      g.fillStyle = pat;
      g.fill();
      g.strokeStyle = P.vein_edge;
      g.lineWidth = P.vein_edge_w;
      g.stroke();
      g.restore();
    }
    const props = P.spot_props[s.kind] || [];
    const spread = P.spot_spread[props.length] || [];
    props.forEach((name, i) => {
      const [x, y] = at(spread[i]);
      const tall = s.w * (name.startsWith("tree") ? P.tree_tall : P.prop_tall);
      place(g, name, x, y, tall / sprites[name].tall);
    });
    const [mx, my] = at(K.n.half);
    const mr = most(s.w * P.mark_r, P.mark_min);
    place(g, "mark-" + s.kind, mx, my - s.w * P.mark_up - mr, mr / sprites["mark-" + s.kind].r);
  });
}

// Dust and chips from recent impacts, fading by the core's count (R1.3608).
function puffs(g, f) {
  (f.puffs || []).forEach((p) => {
    g.globalAlpha = p.fade * P.per_mille * P.puff_alpha;
    place(g, "puff", p.x, p.y, p.r / sprites.puff.r);
    g.globalAlpha = p.fade * P.per_mille;
    const name = "chip-" + p.kind;
    (p.chips || []).forEach(([x, y]) => place(g, name, x, y, (p.chip_r * P.chip_k) / sprites[name].r));
  });
  g.globalAlpha = 1;
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
  puffs(g, f);
  marks(g, f);
  flag(g, f);
}

// Draws one frame on the canvas's 2D context.
export function drawField(g, f) {
  if (!look || !sprites) return;
  g.save();
  // a big hit shakes the view, by the core's offset
  if (f.shake) g.translate(f.shake[0], f.shake[1]);
  draw(g, f, "main");
  g.restore();
  inset(g, f);
}
