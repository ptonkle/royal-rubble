// The playfield's renderer: the one file a chosen look replaces. It draws
// the core's frame (crates/core/src/app/field.rs) on a 2D canvas as Bayeux
// embroidery, with the stitch helpers in web/stitch.js. Every position,
// radius and width comes from the core in pixels; the stitch sizes from
// data/stitch.json, wool colors from data/palette.json, the strain and spot
// threads from field.css (--strain-*, --spot-*). It decides nothing.
//
// Stitching is slow, so every stitched thing is drawn once into an offscreen
// canvas and reused: segments by material, strain and size bucket; figures
// and huts by rank and size; the linen by canvas size; the terrain while the
// view holds still. Only cheap dashed strokes are drawn fresh each frame.

import { create, parts, ellipse } from "./stitch.js";

let look = null;
let K = null;
let G = null;
let S = null;
let colors = {};
const sprites = new Map();
const layers = new Map();
let linenCanvas = null;

// field.json (stroke styles), stitch.json and palette.json.
export function setLook(table, stitch, palette) {
  look = table;
  K = stitch;
  G = stitch.game;
  S = create(document.createElement("canvas").getContext("2d"), palette, stitch, G.seed);
}

function color(name) {
  if (!(name in colors)) colors[name] = getComputedStyle(document.documentElement).getPropertyValue("--" + name).trim();
  return colors[name];
}

// A wool from a field.css color, for the threads the core names by state.
function cssWool(name) {
  return S.addWool(name, color(name));
}

const half = (v) => v * K.n.half;
const bucket = (v, step) => Math.max(step, Math.round(v / step) * step);

// An offscreen canvas drawn once with the stitch helpers; (ox, oy) is the
// point of it that lands on the anchor.
function sprite(key, w, h, ox, oy, draw) {
  let sp = sprites.get(key);
  if (sp) return sp;
  if (sprites.size > G.cache_limit) sprites.clear();
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.ceil(w));
  c.height = Math.max(1, Math.ceil(h));
  S.on(c.getContext("2d"));
  draw(S);
  sp = { c, ox, oy };
  sprites.set(key, sp);
  return sp;
}

function put(g, sp, x, y) {
  g.drawImage(sp.c, Math.round(x - sp.ox), Math.round(y - sp.oy));
}

// --- segments: couched wool along the length, by material

function beamSprite(material, w, len, strain) {
  const wb = bucket(Math.max(w, G.beam_min_w), K.n.two);
  const lb = bucket(len, G.beam_bucket);
  const pad = G.beam_pad;
  const key = "beam|" + material + "|" + wb + "|" + lb + "|" + strain;
  return sprite(key, lb + pad + pad, wb + pad + pad, pad, half(wb) + pad, (s) => {
    const M = K.materials[material] || K.materials.wood;
    const x0 = pad;
    const y0 = pad;
    const x1 = pad + lb;
    const y1 = pad + wb;
    const ym = half(y0 + y1);
    const line = strain === "" ? M.line : cssWool("strain-" + strain);
    const lineW = wb * (strain === "" ? G.beam_line : G.beam_line_strain);
    if (M.rope) {
      s.stem([[x0, ym], [x1, ym]], M.rope, { w: wb * G.rope_w + lineW * (strain === "" ? 0 : 1), slant: G.rope_slant });
      if (strain !== "") s.stem([[x0, ym], [x1, ym]], line, { w: lineW });
      return;
    }
    const ring = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    const shape = s.poly(ring);
    s.couch(shape, M.fill, { angle: 0, scale: G.beam_scale });
    if (M.grain) s.stem([[x0 + lineW, ym], [x1 - lineW, ym]], M.grain, { w: wb * G.beam_wood_grain });
    if (M.courses) {
      for (let x = x0 + wb * G.beam_course; x < x1 - lineW; x += wb * G.beam_course) s.stem([[x, y0], [x, y1]], M.courses, { w: lineW * K.n.half });
    }
    s.stem(ring, line, { closed: true, w: lineW });
    if (M.rivets) {
      for (let x = x0 + wb * G.rivet_every * K.n.half; x < x1; x += wb * G.rivet_every) s.knot(x, ym, M.rivets, wb * G.rivet_r);
    }
  });
}

function beam(g, a, b, w, material, strain) {
  const [ax, ay] = a;
  const [bx, by] = b;
  const len = Math.hypot(bx - ax, by - ay);
  if (len < 1) return;
  const sp = beamSprite(material, w, len, strain === undefined ? "" : strain);
  const lb = sp.c.width - sp.ox - sp.ox;
  const k = len / lb;
  g.save();
  g.translate(ax, ay);
  g.rotate(Math.atan2(by - ay, bx - ax));
  g.drawImage(sp.c, -sp.ox * k, -sp.oy, sp.c.width * k, sp.c.height);
  g.restore();
}

function knotSprite(r, wool) {
  const rb = bucket(r * G.node_r, 1);
  return sprite("knot|" + rb + "|" + wool, rb * K.n.four, rb * K.n.four, rb + rb, rb + rb, (s) => s.knot(rb + rb, rb + rb, wool, rb));
}

function flames(g, a, b, w) {
  const [ax, ay] = a;
  const [bx, by] = b;
  const len = Math.hypot(bx - ax, by - ay);
  const step = Math.max(w, 1) * G.flame_every;
  const sp = flameSprite(Math.max(w, G.beam_min_w));
  for (let t = half(step); t < len; t += step) put(g, sp, ax + ((bx - ax) * t) / len, ay + ((by - ay) * t) / len);
}

function flameSprite(w) {
  const wb = bucket(w, K.n.two);
  const h = wb * G.flame_h;
  const fw = wb * G.flame_w;
  return sprite("flame|" + wb, fw * K.n.four, h + fw, fw + fw, h, (s) => {
    const cx = fw + fw;
    const tongue = (dx, hh, wool, line) =>
      s.patch(s.curve([[cx + dx - fw, h], [cx + dx - half(fw), h - half(hh)], [cx + dx, h - hh], [cx + dx + half(fw), h - half(hh)], [cx + dx + fw, h]], K.n.three, true), wool, line, { angle: Math.PI * K.n.half, scale: G.beam_scale, lineW: wb * G.beam_line });
    tongue(-half(fw), h * G.flame_w, "terracotta", "rule");
    tongue(half(fw), h * G.flame_w, "terracotta", "rule");
    tongue(0, h, "ochre", "terracotta");
  });
}

// --- royals and buildings

function figureSprite(rank, r, dead, f) {
  const rb = bucket(r, G.sprite_bucket);
  const tall = Math.max(rb * G.figure_tall_r, G.figure_min_px);
  const s = tall / G.units;
  const w = tall * G.fig_w;
  const h = tall * G.fig_h;
  const key = "fig|" + rank + "|" + rb + "|" + dead + "|" + f;
  const hut = K.huts[rank];
  if (hut) {
    const hs = Math.max(rb * G.hut_units_r, G.hut_min_px / G.hut_units);
    const hw = G.hut_units * hs;
    return sprite(key, hw + hw, hw + hw, hw, hw + hw - hw * G.figure_pad, (st) => {
      const flags = { [hut.tool]: true, checks: !!hut.checks };
      parts(st, K.hut, { wall: hut.wall, roof: hut.roof, tiles_a: hut.tiles_a, tiles_b: hut.tiles_b }, hw, hw + hw - hw * G.figure_pad, hs, 1, flags);
    });
  }
  const C = K.costumes[rank] || K.costumes.squire;
  return sprite(key, w + w, h, w, h - tall * G.figure_pad, (st) => {
    const flags = {};
    C.flags.forEach((fl) => (flags[fl] = true));
    const cols = dead ? Object.assign({}, C, K.dead_colors) : C;
    parts(st, K.body, cols, w, h - tall * G.figure_pad, s, f, flags);
  });
}

function royals(g, f) {
  const mid = half(f.size[0]);
  f.royals.forEach((r) => {
    const face = r.x > mid ? -1 : 1;
    const sp = figureSprite(r.rank, r.r, r.dead, face);
    const foot = r.y + r.r * (K.huts[r.rank] ? G.hut_foot_r : G.figure_foot_r);
    if (r.dead && !K.huts[r.rank]) {
      g.save();
      g.globalAlpha = G.dead_alpha;
      g.translate(r.x, r.y + r.r);
      g.rotate(G.figure_dead_turn * face);
      g.drawImage(sp.c, -sp.ox, -sp.oy);
      g.restore();
      return;
    }
    put(g, sp, r.x, foot);
  });
}

// --- the background: linen, terrain, water, the plot (cached per view)

function linenLayer(w, h) {
  if (linenCanvas && linenCanvas.width === w && linenCanvas.height === h) return linenCanvas;
  linenCanvas = document.createElement("canvas");
  linenCanvas.width = w;
  linenCanvas.height = h;
  S.on(linenCanvas.getContext("2d"));
  S.linen(0, 0, w, h);
  return linenCanvas;
}

// small humps along the ground, the tapestry's walking line
function scallops(pts, r) {
  const out = [];
  let acc = 0;
  for (let i = 1; i < pts.length; i++) {
    const [px, py] = pts[i - 1];
    const [qx, qy] = pts[i];
    const l = Math.hypot(qx - px, qy - py) || 1;
    for (let d = 0; d < l; d += G.ground_scallop_step) {
      const hgt = Math.abs(Math.sin(((acc + d) / (r + r)) * Math.PI)) * r * G.ground_scallop_h;
      const x = px + ((qx - px) * d) / l;
      const y = py + ((qy - py) * d) / l;
      out.push([x + ((qy - py) / l) * hgt, y - ((qx - px) / l) * hgt]);
    }
    acc += l;
  }
  return out;
}

function fine(pts) {
  // the ground in short steps, so offsets bend smoothly
  const out = [];
  for (let i = 1; i < pts.length; i++) {
    const [px, py] = pts[i - 1];
    const [qx, qy] = pts[i];
    const n = Math.max(1, Math.round(Math.hypot(qx - px, qy - py) / G.wave_step));
    for (let k = 0; k < n; k++) out.push([px + ((qx - px) * k) / n, py + ((qy - py) * k) / n]);
  }
  if (pts.length) out.push(pts[pts.length - 1]);
  return out;
}

function background(slot, f) {
  const [w, h] = f.size;
  const key = JSON.stringify([w, h, f.ground, f.water, f.zone, f.grid]);
  let L = layers.get(slot);
  if (L && L.key === key) return L.c;
  if (!L || L.c.width !== w || L.c.height !== h) {
    L = { c: document.createElement("canvas") };
    L.c.width = w;
    L.c.height = h;
    layers.set(slot, L);
  }
  L.key = key;
  const g = L.c.getContext("2d");
  g.drawImage(linenLayer(w, h), 0, 0);
  S.on(g);
  // terrain: banded mounds under a scalloped ground line
  const pts = fine(f.ground);
  if (pts.length > 1) {
    const under = pts.concat([[pts[pts.length - 1][0], h], [pts[0][0], h]]);
    g.save();
    g.clip(S.poly(under).path);
    for (const [wool, a, b] of K.ground_bands) {
      const top = pts.map(([x, y]) => [x, y + G[a]]);
      const bot = pts.map(([x, y]) => [x, y + G[b]]);
      g.fillStyle = S.tile(wool, 0, G.tile_scale);
      g.fill(S.poly(top.concat(bot.slice().reverse())).path);
      S.cord(top, "navy", G.ground_band_line);
      S.cord(bot, "rule", G.ground_band_line);
    }
    g.restore();
    S.stem(scallops(pts, G.ground_scallop), "slate", { w: G.ground_scallop_w });
    S.stem(pts, "rule", { w: G.ground_line });
  }
  // water: a faint wash, stitched wave lines in several wools, a fish or two
  if (f.water.length && pts.length > 1) {
    g.save();
    g.clip(S.poly(pts.concat([[pts[pts.length - 1][0], 0], [pts[0][0], 0]])).path);
    const [[ax, ay], [bx]] = f.water;
    g.save();
    g.globalAlpha = G.water_alpha;
    g.fillStyle = S.tile("bluegreen", 0, G.tile_scale);
    g.fillRect(ax, ay, bx - ax, h - ay);
    g.restore();
    let i = 0;
    for (let y = ay + G.wave_gap; y < h; y += G.wave_gap, i++) {
      const pts = [];
      for (let x = ax; x <= bx; x += G.wave_step) pts.push([x, y + Math.sin((x / G.wave_len) * Math.PI + i) * G.wave_amp]);
      S.stem(pts, K.wave_wools[i % K.wave_wools.length], { w: G.wave_w });
    }
    for (let x = ax + half(G.fish_every); x < bx - G.fish_len; x += G.fish_every) {
      const y = ay + G.wave_gap * (K.n.two + ((x / G.fish_every) & 1)) + half(G.wave_gap);
      const L2 = G.fish_len;
      S.patch([[x, y], [x + half(L2), y - L2 / K.n.four], [x + L2, y], [x + half(L2), y + L2 / K.n.four], [x, y], [x - L2 / K.n.four, y - L2 / K.n.four], [x - L2 / K.n.four, y + L2 / K.n.four]], "ochre", "navy", { scale: G.tile_scale, lineW: G.wave_w });
    }
    g.restore();
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
  return L.c;
}

// --- shots, debris, the trail

function ballSprite(r, hot) {
  const rb = bucket(r, 1);
  const pad = rb;
  return sprite("ball|" + rb + "|" + hot, (rb + pad) * K.n.two, (rb + pad) * K.n.two, rb + pad, rb + pad, (s) => {
    const c = rb + pad;
    const ring = ellipse(c, c, rb, rb, 0, Math.PI + Math.PI, K.n.eight + K.n.eight);
    s.patch(ring, hot ? "terracotta" : "navy", hot ? "ochre" : "buff", { angle: K.n.half, scale: G.beam_scale, lineW: rb * G.ball_line + 1 });
    s.stem(ellipse(c, c, rb * G.ball_shine, rb * G.ball_shine, Math.PI, Math.PI * K.n.half * K.n.three, K.n.six), hot ? "ochre" : "slate", { w: rb * G.ball_line });
  });
}

function debrisSprite(r) {
  const rb = bucket(r, 1);
  const c = rb + rb;
  return sprite("debris|" + rb, c + c, c + c, c, c, (s) => {
    s.patch([[c - rb, c - half(rb)], [c + half(rb), c - rb], [c + rb, c + half(rb)], [c - half(rb), c + rb]], "tan", "rule", { scale: G.beam_scale, lineW: rb * G.ball_line + 1 });
  });
}

function dashed(g, pts, wool, w, len, gap) {
  if (pts.length < K.n.two) return;
  const W = S.wool(wool);
  g.save();
  g.beginPath();
  pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.lineCap = "round";
  g.setLineDash([len, gap]);
  g.lineWidth = w;
  g.strokeStyle = "rgb(" + W.dark.map((v) => v | 0).join(",") + ")";
  g.stroke();
  g.lineWidth = w * K.n.half;
  g.strokeStyle = "rgb(" + W.light.map((v) => v | 0).join(",") + ")";
  g.stroke();
  g.restore();
}

function balls(g, f) {
  f.debris.forEach((d, i) => {
    const sp = debrisSprite(d.r);
    g.save();
    g.translate(d.x, d.y);
    g.rotate(i * G.debris_turn);
    g.drawImage(sp.c, -sp.ox, -sp.oy);
    g.restore();
  });
  f.shots.forEach((s) => put(g, ballSprite(s.r, s.hot), s.x, s.y));
}

// --- the trebuchet

function engine(g, f) {
  const e = f.engine;
  const [f0, f1] = e.feet;
  const w = e.arm_w;
  beam(g, f0, e.pivot, w, "frame");
  beam(g, f1, e.pivot, w, "frame");
  beam(g, f0, f1, w, "base");
  beam(g, e.weight, e.tip, w, "arm");
  const wr = bucket(e.weight_r, K.n.two);
  put(g, sprite("weight|" + wr, wr * K.n.four, wr * K.n.four, wr + wr, wr + wr, (s) => {
    const c = wr + wr;
    s.patch([[c - wr, c - wr], [c + wr, c - wr], [c + wr, c + wr], [c - wr, c + wr]], "buff", "navy", { scale: G.beam_scale, lineW: wr * G.ball_line + 1 });
    for (let i = 0; i < K.n.four; i++) {
      for (let j = 0; j < K.n.four; j++) {
        if ((i + j) & 1) continue;
        const q = half(wr);
        const x = c - wr + i * q;
        const y = c - wr + j * q;
        s.couch(s.poly([[x, y], [x + q, y], [x + q, y + q], [x, y + q]]), "navy", { scale: G.beam_scale, noBars: true });
      }
    }
  }), e.weight[0], e.weight[1]);
  const [px, py] = e.pivot;
  put(g, knotSprite(w, "ochre"), px, py);
  if (e.loaded) put(g, ballSprite(e.loaded.r, e.loaded.hot), e.loaded.x, e.loaded.y);
}

// --- small marks

function marks(g, f) {
  if (f.ring) {
    const pts = ellipse(f.ring.x, f.ring.y, f.ring.r, f.ring.r, 0, Math.PI + Math.PI, G.units);
    dashed(g, pts, f.ring.ok ? "navy" : cssWool("field-bad"), G.ring_w, G.trail_len, G.trail_gap);
  }
  if (f.preview) {
    const p = f.preview;
    g.globalAlpha = G.preview_alpha;
    beam(g, p.a, p.b, p.w, p.ok ? p.material : "wood", p.ok ? undefined : K.n.four);
    g.globalAlpha = 1;
  }
}

function flag(g, f) {
  if (!f.flag) return;
  const [p0, p1] = f.flag.pole;
  beam(g, p0, p1, G.beam_min_w, "base");
  const cloth = f.flag.cloth;
  const xs = cloth.map(([x]) => x);
  const ys = cloth.map(([, y]) => y);
  const x0 = Math.min(...xs) - G.beam_pad;
  const y0 = Math.min(...ys) - G.beam_pad;
  const key = "flag|" + JSON.stringify(cloth.map(([x, y]) => [x - x0, y - y0]));
  const sp = sprite(key, Math.max(...xs) - x0 + G.beam_pad, Math.max(...ys) - y0 + G.beam_pad, 0, 0, (s) => {
    s.patch(cloth.map(([x, y]) => [x - x0, y - y0]), "terracotta", "navy", { scale: G.beam_scale, lineW: G.beam_min_w * G.flag_line });
  });
  g.drawImage(sp.c, x0, y0);
}

// The map's spots: a band of wool laid along the ground.
function spots(g, f) {
  (f.spots || []).forEach((s) => dashed(g, [s.a, s.b], cssWool("spot-" + s.kind), s.w * G.spot_w, G.zone_run, G.grid_run));
}

function beams(g, f) {
  f.beams.forEach((b) => {
    beam(g, b.a, b.b, b.w, b.material, b.strain);
    if (b.burning) flames(g, b.a, b.b, b.w);
  });
  f.nodes.forEach((n) => put(g, knotSprite(n.r, n.hot ? "ochre" : "navy"), n.x, n.y));
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
  S.on(g);
  S.cord([[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]], "rule", G.inset_edge);
}

function draw(g, f, slot) {
  g.drawImage(background(slot, f), 0, 0);
  // the arm swings down past the ground line: drawn under the ground band
  engine(g, f);
  spots(g, f);
  dashed(g, f.trail, "rule", G.trail_w, G.trail_len, G.trail_gap);
  beams(g, f);
  royals(g, f);
  balls(g, f);
  marks(g, f);
  flag(g, f);
}

// Draws one frame on the canvas's 2D context.
export function drawField(g, f) {
  if (!look || !S) return;
  draw(g, f, "main");
  inset(g, f);
}
