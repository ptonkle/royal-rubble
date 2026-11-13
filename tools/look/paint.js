// Painting helpers for the hand-painted look (R1.3600): a seeded random
// source and a few brush tools on a 2D canvas, used by tools/look/paint.html
// to bake the playfield's pictures ahead of time. The page only places the
// baked pictures; this file is a bake tool, not page code, so it holds its
// own numbers.

export function rng(seed) {
  let a = seed >>> 0;
  const r = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  r.range = (lo, hi) => lo + (hi - lo) * r();
  r.pick = (list) => list[Math.floor(r() * list.length)];
  return r;
}

export function hex(c) {
  const n = parseInt(c.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

// A color c moved toward white (k > 0) or black (k < 0), as css.
export function shade(c, k, alpha) {
  const [r, g, b] = typeof c === "string" ? hex(c) : c;
  const t = k > 0 ? 255 : 0;
  const m = Math.abs(k);
  const f = (v) => Math.round(v + (t - v) * m);
  return `rgba(${f(r)},${f(g)},${f(b)},${alpha === undefined ? 1 : alpha})`;
}

// Draws `fn(x, y)` at a point and at its copies one tile over, so a tile
// of size w by h repeats without a seam.
export function wrap(w, h, x, y, fn) {
  for (const dx of [-w, 0, w]) for (const dy of [-h, 0, h]) fn(x + dx, y + dy);
}

// A rough blob: a circle with a ragged edge, filled.
export function blob(g, r, x, y, rad, fill, rough) {
  const n = 9 + Math.floor(rad / 3);
  g.beginPath();
  for (let i = 0; i <= n; i++) {
    const a = (i / n) * Math.PI * 2;
    const rr = rad * (1 - (rough || 0.25) * r());
    const px = x + Math.cos(a) * rr;
    const py = y + Math.sin(a) * rr;
    i ? g.lineTo(px, py) : g.moveTo(px, py);
  }
  g.closePath();
  g.fillStyle = fill;
  g.fill();
}

// A polygon with its edges nudged, so nothing reads as clean vector work.
export function rough(g, r, pts, amt) {
  g.beginPath();
  pts.forEach(([x, y], i) => {
    const px = x + r.range(-amt, amt);
    const py = y + r.range(-amt, amt);
    i ? g.lineTo(px, py) : g.moveTo(px, py);
  });
  g.closePath();
}

// A wobbly stroke along points.
export function line(g, r, pts, w, color, amt) {
  g.beginPath();
  pts.forEach(([x, y], i) => {
    const px = x + r.range(-amt, amt);
    const py = y + r.range(-amt, amt);
    i ? g.lineTo(px, py) : g.moveTo(px, py);
  });
  g.lineWidth = w;
  g.strokeStyle = color;
  g.lineCap = "round";
  g.lineJoin = "round";
  g.stroke();
}

// Speckles over a rectangle: n dots in random tones of `base`, the painted
// grit that keeps a fill from looking flat. With tile set, each wraps.
export function grit(g, r, x0, y0, w, h, n, base, spread, size, alpha, tile) {
  for (let i = 0; i < n; i++) {
    const x = x0 + r() * w;
    const y = y0 + r() * h;
    const c = shade(base, r.range(-spread, spread), alpha);
    const s = r.range(size * 0.4, size);
    const dot = (px, py) => {
      g.fillStyle = c;
      g.fillRect(px, py, s, s * r.range(0.6, 1.2));
    };
    tile ? wrap(w, h, x, y, dot) : dot(x, y);
  }
}

// Soft dabs: translucent round strokes, the brush texture of a painted fill.
export function dabs(g, r, x0, y0, w, h, n, base, spread, size, alpha, tile) {
  for (let i = 0; i < n; i++) {
    const x = x0 + r() * w;
    const y = y0 + r() * h;
    const c = shade(base, r.range(-spread, spread), alpha);
    const rad = r.range(size * 0.5, size);
    const sx = r.range(1, 2.2);
    const d = (px, py) => {
      g.save();
      g.translate(px, py);
      g.scale(sx, 1);
      g.beginPath();
      g.arc(0, 0, rad, 0, Math.PI * 2);
      g.fillStyle = c;
      g.fill();
      g.restore();
    };
    tile ? wrap(w, h, x, y, d) : d(x, y);
  }
}
