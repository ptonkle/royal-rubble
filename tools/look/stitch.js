// Bayeux-style embroidery helpers for canvas 2D: linen ground, laid-and-couched
// fills, stem-stitch lines, running stitch, French knots and the stitched
// motifs (figures, huts) the playfield and the look renders share.
// Procedural and deterministic (seeded). The production copy of
// notes/look/stitch/stitch.js. Every size and tuning value lives in
// data/stitch.json (K below) and every color in data/palette.json; the page
// holds no numbers of its own.
//
//   const S = create(ctx, palette, K, seed);
//   S.linen(0, 0, W, H);
//   S.couch(S.poly(points), "terracotta", { angle: Math.PI / K.n.two });
//   S.stem(points, "navy", { closed: true });

const TAU = Math.PI + Math.PI;

export function rng(K, seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * K.rng.mul + K.rng.add) >>> 0;
    return s / K.rng.mod;
  };
}

function hexRgb(K, h) {
  const n = parseInt(h.slice(1), K.hex.base);
  return [n >> K.hex.r_shift, (n >> K.hex.g_shift) & K.hex.mask, n & K.hex.mask];
}
const rgbStr = ([r, g, b]) => "rgb(" + (r | 0) + "," + (g | 0) + "," + (b | 0) + ")";
const rgba = ([r, g, b], a) => "rgba(" + (r | 0) + "," + (g | 0) + "," + (b | 0) + "," + a + ")";
const mix = ([ar, ag, ab], [br, bg, bb], t) => [ar + (br - ar) * t, ag + (bg - ag) * t, ab + (bb - ab) * t];

// Catmull-Rom through points, as a polyline (for curved stitch lines).
export function curve(K, pts, per, closed) {
  const out = [];
  const n = pts.length;
  const c = K.cr;
  const P = (i) => (closed ? pts[(i + n) % n] : pts[Math.max(0, Math.min(n - 1, i))]);
  const last = closed ? n : n - 1;
  const at = (a, b, d, e, t, t2, t3) =>
    c.half * (c.two * b + (d - a) * t + (c.two * a - c.five * b + c.four * d - e) * t2 + (c.three * b - a - c.three * d + e) * t3);
  for (let i = 0; i < last; i++) {
    const [ax, ay] = P(i - 1);
    const [bx, by] = P(i);
    const [dx, dy] = P(i + 1);
    const [ex, ey] = P(i + 1 + 1);
    for (let k = 0; k < per; k++) {
      const t = k / per;
      const t2 = t * t;
      const t3 = t2 * t;
      out.push([at(ax, bx, dx, ex, t, t2, t3), at(ay, by, dy, ey, t, t2, t3)]);
    }
  }
  out.push(closed ? out[0].slice() : pts[n - 1].slice());
  return out;
}

export function ellipse(cx, cy, rx, ry, a0, a1, n) {
  const out = [];
  for (let i = 0; i <= n; i++) {
    const a = a0 + ((a1 - a0) * i) / n;
    out.push([cx + Math.cos(a) * rx, cy + Math.sin(a) * ry]);
  }
  return out;
}

// Points every `step` along a polyline, each with its unit direction.
function resample(pts, step, closed) {
  const src = closed ? pts.concat([pts[0]]) : pts;
  const out = [];
  let carry = 0;
  for (let i = 1; i < src.length; i++) {
    const [x0, y0] = src[i - 1];
    const [x1, y1] = src[i];
    const len = Math.hypot(x1 - x0, y1 - y0);
    if (!len) continue;
    const dx = (x1 - x0) / len;
    const dy = (y1 - y0) / len;
    for (let d = carry; d <= len; d += step) out.push([x0 + dx * d, y0 + dy * d, dx, dy]);
    carry = step - ((len - carry) % step);
    if (carry === step) carry = 0;
  }
  return out;
}

// A polyline moved sideways by d (positive: to the left of its direction).
export function offset(pts, d) {
  return pts.map(([px, py], i) => {
    const [ax, ay] = pts[Math.max(0, i - 1)];
    const [bx, by] = pts[Math.min(pts.length - 1, i + 1)];
    const dx = bx - ax;
    const dy = by - ay;
    const l = Math.hypot(dx, dy) || 1;
    return [px - (dy / l) * d, py + (dx / l) * d];
  });
}

export function create(context, palette, K, seed) {
  const P = K.stitch;
  const n = K.n;
  const R = rng(K, seed);
  let ctx = context;
  let k = 1; // the current canvas transform's scale
  const unit = () => P.zoom / k;
  const cache = {};
  const extra = {};
  function wool(name) {
    if (cache[name]) return cache[name];
    const w = extra[name] || palette.wool[name] || (name === "linen" ? palette.linen : null);
    if (!w) throw new Error("no wool " + name);
    cache[name] = { light: hexRgb(K, w.light), mid: hexRgb(K, w.mid), dark: hexRgb(K, w.dark) };
    return cache[name];
  }
  // A wool made from one color (a strain thread from field.css).
  function addWool(name, hex) {
    if (extra[name] && extra[name].src === hex) return name;
    const m = hexRgb(K, hex);
    const hx = (c) => "#" + c.map((v) => Math.round(v).toString(K.hex.base).padStart(n.two, "0")).join("");
    extra[name] = { src: hex, mid: hex, light: hx(mix(m, K.white, P.wool_lift)), dark: hx(mix(m, K.black, P.wool_drop)) };
    delete cache[name];
    shadeCache.clear();
    return name;
  }
  const shadeCache = new Map();
  // n shade buckets from dark to light around mid
  function shades(name, count, spread) {
    const key = name + "|" + count + "|" + spread;
    if (shadeCache.has(key)) return shadeCache.get(key);
    const w = wool(name);
    const out = [];
    for (let i = 0; i < count; i++) {
      const t = ((i / (count - 1)) * n.two - 1) * spread;
      out.push(rgbStr(t < 0 ? mix(w.mid, w.dark, -t) : mix(w.mid, w.light, t)));
    }
    shadeCache.set(key, out);
    return out;
  }
  // a shade index, bunched toward the middle
  const pick = (count) => Math.min(count - 1, Math.floor(((R() + R() + R()) / n.three) * count));

  // A shape: Path2D plus its rings and bounding box.
  function poly(...rings) {
    if (typeof rings[0][0] === "number") rings = [rings];
    const p = new Path2D();
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const ring of rings) {
      ring.forEach(([x, y], i) => {
        if (i) p.lineTo(x, y);
        else p.moveTo(x, y);
        x0 = Math.min(x0, x);
        y0 = Math.min(y0, y);
        x1 = Math.max(x1, x);
        y1 = Math.max(y1, y);
      });
      p.closePath();
    }
    return { path: p, rings, box: [x0, y0, x1, y1] };
  }

  // Bare linen: tabby weave, slubbed threads, creases and faint foxing, in
  // device pixels so the weave stays crisp under any transform.
  function linen(x, y, w, h) {
    const L = wool("linen");
    const Q = K.linen;
    const m = ctx.getTransform();
    const dx = m.a * x + m.e;
    const dy = m.d * y + m.f;
    const dw = w * m.a;
    const dh = h * m.d;
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.beginPath();
    ctx.rect(dx, dy, dw, dh);
    ctx.clip();
    const pitch = Q.weave * P.zoom;
    const T = Q.tile;
    const tile = document.createElement("canvas");
    tile.width = T;
    tile.height = T;
    const g = tile.getContext("2d");
    const img = g.createImageData(T, T);
    const d = img.data;
    const nT = Math.ceil(T / pitch) + 1;
    const rows = [];
    const cols = [];
    for (let i = 0; i < nT; i++) {
      rows.push((R() - n.half) * n.two * Q.slub);
      cols.push((R() - n.half) * n.two * Q.slub * Q.warp_share);
    }
    const [lr, lg, lb] = L.mid;
    for (let py = 0; py < T; py++) {
      for (let px = 0; px < T; px++) {
        const ty = Math.floor(py / pitch);
        const tx = Math.floor(px / pitch);
        const over = (tx + ty) & 1;
        const fy = (py / pitch) % 1;
        const fx = (px / pitch) % 1;
        const round = over ? Math.sin(fy * Math.PI) : Math.sin(fx * Math.PI);
        const v = 1 + (over ? rows[ty] : cols[tx]) + (round - Q.round_mid) * Q.round_amp + (R() - n.half) * Q.grain;
        let o = (py * T + px) * n.four;
        d[o++] = lr * v;
        d[o++] = lg * v;
        d[o++] = lb * v;
        d[o] = Q.opaque;
      }
    }
    g.putImageData(img, 0, 0);
    ctx.fillStyle = ctx.createPattern(tile, "repeat");
    ctx.fillRect(dx, dy, dw, dh);
    // long slubs: single weft threads lighter or darker across the cloth
    for (let yy = dy; yy < dy + dh; yy += pitch) {
      if (R() < Q.slub_share) {
        ctx.fillStyle = rgba(R() < n.half ? K.linen_light : K.linen_dark, Q.slub_alpha + R() * Q.slub_alpha);
        ctx.fillRect(dx, yy, dw, pitch * (Q.slub_min + R()));
      }
    }
    // broad uneven tone, like old cloth under gallery light
    for (let i = 0; i < Q.tones; i++) {
      const sx = dx + R() * dw;
      const sy = dy + R() * dh;
      const r = (Q.tone_min + R() * Q.tone_span) * P.zoom;
      const gr = ctx.createRadialGradient(sx, sy, 0, sx, sy, r);
      const c = R() < n.half ? K.linen_light : K.linen_dark;
      gr.addColorStop(0, rgba(c, Q.tone_alpha + R() * Q.tone_alpha));
      gr.addColorStop(1, rgba(c, 0));
      ctx.fillStyle = gr;
      ctx.fillRect(sx - r, sy - r, r * n.two, r * n.two);
    }
    // creases: long soft folds, a dark valley with a lit ridge above
    const C = palette.crease;
    const sh = hexRgb(K, C.shadow);
    const dp = hexRgb(K, C.deep);
    const hl = hexRgb(K, C.light);
    const count = Math.round((Q.creases * dh) / Q.per_px / P.zoom);
    for (let i = 0; i < count; i++) {
      const cy = dy + R() * dh;
      const len = dw * (Q.crease_min + R() * Q.crease_span);
      const cx = dx + R() * dw - len * Q.crease_lead;
      const amp = (1 + R() * Q.crease_amp) * P.zoom;
      const ph = R() * TAU;
      const freq = (Q.crease_freq + R() * Q.crease_freq) / P.zoom;
      const depth = Q.crease_depth * (Q.crease_floor + R() * (1 - Q.crease_floor));
      const tilt = (R() - n.half) * Q.crease_tilt;
      const path = (off) => {
        const p = new Path2D();
        for (let s = 0; s <= Q.crease_steps; s++) {
          const xx = cx + (len * s) / Q.crease_steps;
          const yy = cy + off + Math.sin(xx * freq + ph) * amp + (xx - cx) * tilt;
          if (s) p.lineTo(xx, yy);
          else p.moveTo(xx, yy);
        }
        return p;
      };
      const fade = (col) => {
        const gr = ctx.createLinearGradient(cx, 0, cx + len, 0);
        gr.addColorStop(0, rgba(col, 0));
        gr.addColorStop(Q.fade_in + R() * Q.fade_in, rgba(col, 1));
        gr.addColorStop(Q.fade_out + R() * Q.fade_in, rgba(col, 1));
        gr.addColorStop(1, rgba(col, 0));
        return gr;
      };
      ctx.lineCap = "round";
      ctx.globalAlpha = Math.min(1, Q.valley_alpha * depth);
      ctx.filter = "blur(" + (Q.blur * P.zoom).toFixed(1) + "px)";
      ctx.strokeStyle = fade(sh);
      ctx.lineWidth = Q.valley_w * P.zoom;
      ctx.stroke(path(0));
      ctx.strokeStyle = fade(hl);
      ctx.lineWidth = Q.ridge_w * P.zoom;
      ctx.stroke(path(-Q.ridge_off * P.zoom));
      ctx.filter = "none";
      ctx.globalAlpha = Math.min(1, Q.deep_alpha * depth);
      ctx.strokeStyle = fade(dp);
      ctx.lineWidth = Q.deep_w * P.zoom;
      ctx.stroke(path(Q.deep_off * P.zoom));
      ctx.globalAlpha = 1;
    }
    // foxing: small faint warm spots
    const ns = Math.round((Q.stains * dw * dh) / Q.per_mpx / P.zoom / P.zoom);
    for (let i = 0; i < ns; i++) {
      const sx = dx + R() * dw;
      const sy = dy + R() * dh;
      const r = (Q.stain_min + R() * Q.stain_span) * P.zoom;
      const gr = ctx.createRadialGradient(sx, sy, 0, sx, sy, r);
      gr.addColorStop(0, rgba(K.foxing, Q.stain_alpha + R() * Q.stain_alpha));
      gr.addColorStop(1, rgba(K.foxing, 0));
      ctx.fillStyle = gr;
      ctx.fillRect(sx - r, sy - r, r * n.two, r * n.two);
    }
    ctx.restore();
  }

  // Laid-and-couched fill. Long laid threads side by side along o.angle, each
  // drifting in shade with twist ticks and a fuzz of loose fibers; then
  // couching bars across them at uneven intervals, each tacked down by small
  // stitches that alternate from bar to bar. o.bars: wool for the bars.
  function couch(shape, name, o = {}) {
    const u = unit();
    const tw = P.thread * u * (o.scale || 1);
    const a = o.angle || 0;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const [bx0, by0, bx1, by1] = shape.box;
    const cx = (bx0 + bx1) / n.two;
    const cy = (by0 + by1) / n.two;
    const rad = Math.hypot(bx1 - bx0, by1 - by0) / n.two + tw * n.two;
    const W = wool(name);
    const sh = shades(name, P.shade_count, o.spread != null ? o.spread : P.shade_spread);
    ctx.save();
    ctx.clip(shape.path);
    ctx.fillStyle = rgbStr(mix(W.dark, K.shadow, P.under_dark));
    ctx.fill(shape.path);
    const toXY = (uu, vv) => [cx + uu * ca - vv * sa, cy + uu * sa + vv * ca];
    const pitch = tw * (1 + P.thread_gap);
    const barGap = tw * (o.barEvery || P.bar_every);
    const phase = (o.phase != null ? o.phase : R()) * barGap;
    const buckets = sh.map(() => new Path2D());
    const hi = new Path2D();
    const lo = new Path2D();
    const ticks = new Path2D();
    const twist = new Path2D();
    const fuzz = new Path2D();
    const seg = tw * P.seg;
    for (let v = -rad; v <= rad; v += pitch) {
      let shade = pick(sh.length);
      let wob = (R() - n.half) * P.row_wobble * tw;
      let [px, py] = toXY(-rad, v + wob);
      for (let uu = -rad + seg; uu <= rad + seg; uu += seg) {
        if (R() < P.drift) shade = Math.max(0, Math.min(sh.length - 1, shade + (R() < n.half ? -1 : 1)));
        wob += (R() - n.half) * P.wander * tw;
        wob *= P.settle;
        const [qx, qy] = toXY(uu, v + wob);
        buckets[shade].moveTo(px, py);
        buckets[shade].lineTo(qx, qy);
        px = qx;
        py = qy;
      }
      const [h0x, h0y] = toXY(-rad, v - tw * P.hi_off);
      const [h1x, h1y] = toXY(rad, v - tw * P.hi_off);
      hi.moveTo(h0x, h0y);
      hi.lineTo(h1x, h1y);
      const [l0x, l0y] = toXY(-rad, v + tw * P.lo_off);
      const [l1x, l1y] = toXY(rad, v + tw * P.lo_off);
      lo.moveTo(l0x, l0y);
      lo.lineTo(l1x, l1y);
      // wool twist: slanted ticks along the thread, a lit fiber beside each
      for (let uu = -rad + R() * tw; uu < rad; uu += (tw * (P.twist_min + R() * P.twist_span)) / P.fiber) {
        const [t0x, t0y] = toXY(uu - tw * P.tick_back, v - tw * P.tick_h);
        const [t1x, t1y] = toXY(uu + tw * P.tick_fwd, v + tw * P.tick_h);
        ticks.moveTo(t0x, t0y);
        ticks.lineTo(t1x, t1y);
        const [w0x, w0y] = toXY(uu + tw * P.lit_back, v - tw * P.lit_h);
        const [w1x, w1y] = toXY(uu + tw * P.lit_fwd, v + tw * P.lit_h);
        twist.moveTo(w0x, w0y);
        twist.lineTo(w1x, w1y);
        if (R() < P.fuzz) {
          const fa = R() * TAU;
          const fl = tw * (P.fuzz_min + R() * P.fuzz_span);
          const [f0x, f0y] = toXY(uu, v + (R() - n.half) * tw);
          fuzz.moveTo(f0x, f0y);
          fuzz.lineTo(f0x + Math.cos(fa) * fl, f0y + Math.sin(fa) * fl);
        }
      }
    }
    ctx.lineCap = "butt";
    ctx.lineWidth = tw;
    buckets.forEach((b, i) => {
      ctx.strokeStyle = sh[i];
      ctx.stroke(b);
    });
    ctx.globalAlpha = P.hi_alpha;
    ctx.strokeStyle = rgbStr(W.light);
    ctx.lineWidth = tw * P.hi_w;
    ctx.stroke(hi);
    ctx.globalAlpha = P.lo_alpha;
    ctx.strokeStyle = rgbStr(W.dark);
    ctx.lineWidth = tw * P.lo_w;
    ctx.stroke(lo);
    ctx.globalAlpha = P.tick_alpha;
    ctx.lineWidth = tw * P.tick_w;
    ctx.stroke(ticks);
    ctx.globalAlpha = P.lit_alpha;
    ctx.strokeStyle = rgbStr(W.light);
    ctx.lineWidth = tw * P.lit_w;
    ctx.stroke(twist);
    ctx.globalAlpha = P.fuzz_alpha;
    ctx.lineWidth = tw * P.fuzz_w;
    ctx.stroke(fuzz);
    ctx.globalAlpha = 1;
    if (!o.noBars) {
      // couching bars across the laid work, wavering, then tacks
      const BW = wool(o.bars || name);
      const barShade = o.bars ? BW.mid : mix(BW.dark, BW.mid, P.bar_mid);
      const bars = new Path2D();
      const barHi = new Path2D();
      const tacks = new Path2D();
      let bi = 0;
      for (let uu = -rad - phase; uu < rad; uu += barGap * (P.bar_min + R() * P.bar_span), bi++) {
        let off = (R() - n.half) * P.bar_jog * tw;
        const lean = (R() - n.half) * P.bar_lean;
        let [px, py] = toXY(uu + off, -rad);
        let ti = 0;
        for (let v = -rad; v <= rad; v += pitch, ti++) {
          off += (R() - n.half) * P.bar_jog * tw + lean * pitch;
          off *= P.bar_settle;
          const [qx, qy] = toXY(uu + off, v);
          bars.moveTo(px, py);
          bars.lineTo(qx, qy);
          const [hx, hy] = toXY(uu + off - tw * P.bar_hi_off, v);
          const [gx, gy] = toXY(uu + off - tw * P.bar_hi_off, v + pitch * n.half);
          barHi.moveTo(hx, hy);
          barHi.lineTo(gx, gy);
          px = qx;
          py = qy;
          if (((ti + bi) & 1) === 0) {
            const [k0x, k0y] = toXY(uu + off - tw * P.tack, v + pitch * n.half);
            const [k1x, k1y] = toXY(uu + off + tw * P.tack, v + pitch * n.half);
            tacks.moveTo(k0x, k0y);
            tacks.lineTo(k1x, k1y);
          }
        }
      }
      ctx.lineCap = "round";
      ctx.strokeStyle = rgbStr(mix(barShade, K.shadow, P.bar_shadow_dark));
      ctx.lineWidth = tw * P.bar_width * P.bar_shadow_w;
      ctx.globalAlpha = P.bar_shadow_alpha;
      ctx.stroke(bars);
      ctx.globalAlpha = 1;
      ctx.strokeStyle = rgbStr(barShade);
      ctx.lineWidth = tw * P.bar_width;
      ctx.stroke(bars);
      ctx.globalAlpha = P.bar_hi_alpha;
      ctx.strokeStyle = rgbStr(BW.light);
      ctx.lineWidth = tw * P.bar_hi_w;
      ctx.stroke(barHi);
      ctx.globalAlpha = P.tack_alpha;
      ctx.strokeStyle = rgbStr(mix(BW.dark, K.shadow, P.tack_dark));
      ctx.lineWidth = tw * P.tack_w;
      ctx.stroke(tacks);
    }
    ctx.restore();
  }

  // Stem stitch: overlapping slanted stitches along a polyline, each its own
  // shade with a dark gap and a lit crest, which reads as a twisted cord.
  function stem(pts, name, o = {}) {
    const u = unit();
    const w = (o.w || P.stem_width) * u;
    const L = w * P.stem_length;
    const step = w * P.stem_step;
    const sl = o.slant != null ? o.slant : P.stem_slant;
    const sh = shades(name, P.stem_shades, o.spread != null ? o.spread : P.stem_spread);
    const samp = resample(pts, step, o.closed);
    if (!samp.length) return;
    const buckets = sh.map(() => new Path2D());
    const hi = new Path2D();
    const gap = new Path2D();
    const cs = Math.cos(sl);
    const sn = Math.sin(sl);
    for (const [x, y, dx, dy] of samp) {
      const vx = ((dx * cs - dy * sn) * L) / n.two;
      const vy = ((dx * sn + dy * cs) * L) / n.two;
      const jx = (R() - n.half) * P.stem_jitter * w;
      const jy = (R() - n.half) * P.stem_jitter * w;
      const b = buckets[pick(buckets.length)];
      b.moveTo(x - vx + jx, y - vy + jy);
      b.lineTo(x + vx + jx, y + vy + jy);
      const ox = -dy * w * P.stem_hi_side;
      const oy = dx * w * P.stem_hi_side;
      hi.moveTo(x - vx * P.stem_hi_a + jx + ox, y - vy * P.stem_hi_a + jy + oy);
      hi.lineTo(x + vx * P.stem_hi_b + jx + ox, y + vy * P.stem_hi_b + jy + oy);
      gap.moveTo(x + vx + jx, y + vy + jy);
      gap.lineTo(x + vx * P.stem_gap_end + jx + dx * step * n.half, y + vy * P.stem_gap_end + jy + dy * step * n.half);
    }
    ctx.save();
    ctx.lineCap = "round";
    ctx.lineWidth = w * P.stem_thread;
    buckets.forEach((b, i) => {
      ctx.strokeStyle = sh[i];
      ctx.stroke(b);
    });
    ctx.globalAlpha = P.stem_gap_alpha;
    ctx.strokeStyle = rgbStr(mix(wool(name).dark, K.shadow, P.stem_gap_dark));
    ctx.lineWidth = w * P.stem_gap_w;
    ctx.stroke(gap);
    ctx.globalAlpha = P.stem_hi_alpha;
    ctx.strokeStyle = rgbStr(wool(name).light);
    ctx.lineWidth = w * P.stem_hi_w;
    ctx.stroke(hi);
    ctx.restore();
  }

  // A cheap cord for lines redrawn every frame (the ground line while the
  // camera moves): a dark core, a mid dashed wrap and a lit dashed crest.
  function cord(pts, name, w) {
    const W = wool(name);
    const path = new Path2D();
    pts.forEach(([x, y], i) => (i ? path.lineTo(x, y) : path.moveTo(x, y)));
    ctx.save();
    ctx.lineCap = "butt";
    ctx.lineJoin = "round";
    ctx.strokeStyle = rgbStr(W.dark);
    ctx.lineWidth = w;
    ctx.stroke(path);
    ctx.strokeStyle = rgbStr(W.mid);
    ctx.lineWidth = w * P.cord_wrap;
    ctx.setLineDash([w * P.cord_dash, w * P.cord_gap]);
    ctx.stroke(path);
    ctx.strokeStyle = rgbStr(W.light);
    ctx.globalAlpha = P.cord_hi_alpha;
    ctx.lineWidth = w * P.cord_hi;
    ctx.lineDashOffset = w * P.cord_hi_shift;
    ctx.setLineDash([w * P.cord_hi_dash, w * (P.cord_dash + P.cord_gap - P.cord_hi_dash)]);
    ctx.stroke(path);
    ctx.restore();
  }

  // Running stitch: even dashes with linen between.
  function running(pts, name, o = {}) {
    const u = unit();
    const len = (o.len || P.run_length) * u;
    const gp = (o.gap || P.run_gap) * u;
    const w = (o.w || P.run_width) * u;
    const samp = resample(pts, (len + gp) / n.four, o.closed);
    const sh = shades(name, n.three, P.run_spread);
    const b = sh.map(() => new Path2D());
    for (let i = 0; i + n.two < samp.length; i += n.four) {
      const [px, py] = samp[i];
      const [qx, qy] = samp[i + n.two];
      const path = b[pick(b.length)];
      path.moveTo(px, py);
      path.lineTo(qx + (R() - n.half) * w * P.run_jitter, qy + (R() - n.half) * w * P.run_jitter);
    }
    ctx.save();
    ctx.lineCap = "round";
    ctx.lineWidth = w;
    b.forEach((p, i) => {
      ctx.strokeStyle = sh[i];
      ctx.stroke(p);
    });
    ctx.restore();
  }

  // French knot: a small wound blob.
  function knot(x, y, name, r) {
    const u = unit();
    const rr = (r || P.knot) * u;
    const W = wool(name);
    ctx.save();
    ctx.fillStyle = rgbStr(W.dark);
    ctx.beginPath();
    ctx.arc(x, y, rr, 0, TAU);
    ctx.fill();
    ctx.strokeStyle = rgbStr(W.mid);
    ctx.lineWidth = rr * P.knot_wind;
    ctx.beginPath();
    const a0 = R() * TAU;
    ctx.arc(x, y, rr * P.knot_wind, a0, a0 + P.knot_sweep);
    ctx.stroke();
    ctx.fillStyle = rgbStr(W.light);
    ctx.globalAlpha = P.knot_hi_alpha;
    ctx.beginPath();
    ctx.arc(x - rr * P.knot_hi, y - rr * P.knot_hi, rr * P.knot_hi, 0, TAU);
    ctx.fill();
    ctx.restore();
  }

  // Filled-and-outlined motif: couched fill plus stem outline.
  function patch(rings, fill, line, o = {}) {
    const s = poly(...(typeof rings[0][0] === "number" ? [rings] : rings));
    couch(s, fill, o);
    for (const ring of s.rings) stem(ring, line, { closed: true, w: o.lineW });
    return s;
  }

  // A couched-fill pattern for broad areas redrawn every frame: laid threads
  // along `angle` in one square tile.
  const tiles = new Map();
  function tile(name, angle, scale) {
    const key = name + "|" + angle + "|" + scale;
    if (tiles.has(key)) return tiles.get(key);
    const T = K.tile.size;
    const c = document.createElement("canvas");
    c.width = T;
    c.height = T;
    const keep = ctx;
    const keepK = k;
    ctx = c.getContext("2d");
    k = 1;
    couch(poly([[0, 0], [T, 0], [T, T], [0, T]]), name, { angle, scale, phase: 0 });
    const pat = keep.createPattern(c, "repeat");
    ctx = keep;
    k = keepK;
    tiles.set(key, pat);
    return pat;
  }

  return {
    P,
    K,
    R,
    get ctx() {
      return ctx;
    },
    on(c) {
      ctx = c;
      k = 1;
    },
    wool,
    addWool,
    shades,
    poly,
    curve: (pts, per, closed) => curve(K, pts, per || K.n.eight, closed),
    ellipse,
    offset,
    linen,
    couch,
    stem,
    cord,
    running,
    knot,
    patch,
    tile,
    scale(s) {
      k = s;
    },
    unit,
  };
}

// --- Motifs drawn from data (data/stitch.json "figures" and "huts"), in
// units of a figure 100 tall with its feet at (0, 0), facing right.

// Draws a list of parts at (x, foot), `s` pixels per unit, f = 1 or -1 to
// face right or left. colors maps a part's role to a wool.
export function parts(S, list, colors, x, foot, s, f, flags) {
  const K = S.K;
  const T = (pts) => pts.map(([px, py]) => [x + px * s * f, foot + py * s]);
  const col = (role) => colors[role] || role;
  const line = S.P.stem_width * K.figure.line;
  for (const p of list) {
    if (p.when && !flags[p.when]) continue;
    if (p.unless && flags[p.unless]) continue;
    const pts = p.pts ? T(p.curve ? curve(K, p.pts, p.curve, p.kind === "patch") : p.pts) : null;
    if (p.kind === "patch") {
      S.patch(pts, col(p.fill), col(p.line || "navy"), { angle: (p.angle || 0) * f, scale: p.scale || K.figure.fill, lineW: line * (p.w || 1) });
    } else if (p.kind === "stem") {
      S.stem(pts, col(p.color), { w: line * (p.w || 1), closed: p.closed });
    } else if (p.kind === "knot") {
      const [[kx, ky]] = T([p.at]);
      S.knot(kx, ky, col(p.color), p.r * s * K.figure.knot_unit);
    } else if (p.kind === "bare") {
      // bare linen (a face or a hand), outlined
      const g = S.ctx;
      g.save();
      g.fillStyle = S.K.bare;
      g.beginPath();
      pts.forEach(([a, b], i) => (i ? g.lineTo(a, b) : g.moveTo(a, b)));
      g.closePath();
      g.fill();
      g.restore();
      S.stem(pts, col(p.line || "rule"), { closed: true, w: line * (p.w || 1) });
    } else if (p.kind === "tiles") {
      // scale-tiled roof: rows of little arcs clipped to the shape
      const shape = S.poly(pts);
      S.couch(shape, col(p.fill), { scale: K.figure.fill, noBars: true });
      const g = S.ctx;
      g.save();
      g.clip(shape.path);
      const [x0, y0, x1, y1] = shape.box;
      const r = p.r * s;
      let row = 0;
      for (let yy = y0 + r; yy < y1 + r; yy += r * K.figure.tile_rise, row++) {
        for (let xx = x0 - r + (row & 1) * r; xx < x1 + r; xx += r + r) {
          S.stem(ellipse(xx, yy, r, r * K.figure.tile_tall, 0, Math.PI, K.n.six), col(row & 1 ? p.a : p.b), { w: line * K.figure.tile_w });
        }
      }
      g.restore();
      S.stem(pts, col(p.line || "navy"), { closed: true, w: line });
    } else if (p.kind === "checks") {
      // a checkered panel, as on the tapestry's towers
      const [[ax, ay], [bx, by]] = T(p.box);
      const nx = p.n;
      const cw = (bx - ax) / nx;
      const ny = Math.max(1, Math.round(Math.abs(by - ay) / Math.abs(cw)));
      const ch = (by - ay) / ny;
      for (let i = 0; i < nx; i++) {
        for (let j = 0; j < ny; j++) {
          if ((i + j) & 1) continue;
          const cx = ax + i * cw;
          const cy = ay + j * ch;
          S.couch(S.poly([[cx, cy], [cx + cw, cy], [cx + cw, cy + ch], [cx, cy + ch]]), col(p.color), { scale: K.figure.fill, noBars: true });
        }
      }
    }
  }
}
