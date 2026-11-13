// Bayeux-style embroidery helpers for canvas 2D: linen ground, laid-and-couched
// fills, stem-stitch lines, running stitch, French knots, stitched lettering
// and the beast-and-bar border. Procedural and deterministic (seeded).
//
// Every size lives in PARAMS (the game will load them from data/stitch.json)
// and every color in the palette (data/palette.json, from palette.json here).
// Sizes are in fabric pixels at zoom 1. Stitch sizes follow S.zoom (camera
// zoom on the fabric) and ignore the current canvas transform, so a scene can
// be laid out in world units under any scale and the thread stays the same
// size: call S.scale(k) when drawing under a transform of scale k.
//
//   const S = Stitch.create(ctx, palette, Stitch.PARAMS, seed);
//   S.linen(0, 0, W, H);
//   S.couch(S.poly(points), "terracotta", { angle: Math.PI / 2 });
//   S.stem(points, "navy", { closed: true });
"use strict";
const Stitch = (() => {
  const PARAMS = {
    zoom: 1,              // camera zoom on the fabric; stitches scale with it
    thread: 3.0,          // laid thread width
    threadGap: 0.08,      // gap between laid threads, as a share of the width
    barEvery: 4.6,        // couching bar spacing, in thread widths
    barWidth: 0.62,       // couching bar width, in thread widths
    tackEvery: 2.6,       // tacks along a bar, in thread widths
    rowWobble: 0.16,      // laid thread wander, in thread widths
    shadeSpread: 0.55,    // 0: every thread mid; 1: full light-to-dark range
    fiber: 0.5,           // twist ticks per laid segment (0 turns them off)
    stemWidth: 3.2,       // stem-stitch cord width
    stemSlant: 0.55,      // stitch angle to the line, radians
    stemLength: 2.3,      // stitch length, in cord widths
    stemStep: 0.85,       // advance per stitch, in cord widths
    runLength: 8,         // running stitch dash
    runGap: 6,            // running stitch gap
    runWidth: 2.2,
    knot: 2.4,            // French knot radius
    letterWeight: 0.085,  // lettering cord width, share of cap height
    letterJitter: 0.07,   // per-letter size and baseline wander
    letterTilt: 0.07,     // per-letter rotation wander, radians
    weave: 1.7,           // linen thread pitch
    slub: 0.07,           // linen thread brightness wander
    creases: 22,          // creases per 1000 px of height
    creaseDepth: 0.55,    // crease shadow strength 0..1
    stains: 14,           // faint foxing blotches per megapixel
    borderBar: 15,        // border bar width
    borderSlant: 0.42,    // border bar lean, radians from upright
  };

  function rng(seed) {
    let s = seed >>> 0;
    return () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; };
  }
  const hexRgb = h => { const n = parseInt(h.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
  const rgbStr = c => `rgb(${c[0] | 0},${c[1] | 0},${c[2] | 0})`;
  const mix = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];

  // Catmull-Rom through points, as a polyline (for curved stitch lines)
  function curve(pts, per = 8, closed = false) {
    const out = [], n = pts.length;
    const P = i => closed ? pts[(i + n) % n] : pts[Math.max(0, Math.min(n - 1, i))];
    const last = closed ? n : n - 1;
    for (let i = 0; i < last; i++) {
      const p0 = P(i - 1), p1 = P(i), p2 = P(i + 1), p3 = P(i + 2);
      for (let k = 0; k < per; k++) {
        const t = k / per, t2 = t * t, t3 = t2 * t;
        out.push([0, 1].map(j => 0.5 * (2 * p1[j] + (-p0[j] + p2[j]) * t + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t2 + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t3)));
      }
    }
    out.push(closed ? out[0].slice() : pts[n - 1].slice());
    return out;
  }
  function ellipse(cx, cy, rx, ry, a0 = 0, a1 = Math.PI * 2, n = 28) {
    const out = [];
    for (let i = 0; i <= n; i++) { const a = a0 + (a1 - a0) * i / n; out.push([cx + Math.cos(a) * rx, cy + Math.sin(a) * ry]); }
    return out;
  }
  function resample(pts, step, closed) {
    const src = closed ? pts.concat([pts[0]]) : pts, out = [];
    let carry = 0;
    for (let i = 1; i < src.length; i++) {
      const [x0, y0] = src[i - 1], [x1, y1] = src[i], len = Math.hypot(x1 - x0, y1 - y0);
      if (!len) continue;
      const dx = (x1 - x0) / len, dy = (y1 - y0) / len;
      for (let d = carry; d <= len; d += step) out.push([x0 + dx * d, y0 + dy * d, dx, dy]);
      carry = step - ((len - carry) % step);
      if (carry === step) carry = 0;
    }
    return out;
  }

  function create(ctx, palette, params = PARAMS, seed = 1066) {
    const P = Object.assign({}, PARAMS, params);
    const R = rng(seed);
    let k = 1; // current canvas transform scale
    const unit = () => P.zoom / k; // one fabric pixel in current coordinates
    const cache = {};
    function wool(name) {
      if (cache[name]) return cache[name];
      const w = palette.wool[name] || (name === "linen" ? palette.linen : null);
      if (!w) throw new Error("no wool " + name);
      return (cache[name] = { light: hexRgb(w.light), mid: hexRgb(w.mid), dark: hexRgb(w.dark) });
    }
    // n shade buckets from dark to light around mid, spread by P.shadeSpread
    function shades(name, n = 5, spread = P.shadeSpread) {
      const w = wool(name), out = [];
      for (let i = 0; i < n; i++) {
        const t = (i / (n - 1) * 2 - 1) * spread;
        out.push(rgbStr(t < 0 ? mix(w.mid, w.dark, -t) : mix(w.mid, w.light, t)));
      }
      return out;
    }
    const pick = n => Math.min(n - 1, Math.floor((R() + R() + R()) / 3 * n));

    // A shape: Path2D plus its points (array of rings) and bounding box.
    function poly(...rings) {
      if (typeof rings[0][0] === "number") rings = [rings];
      const p = new Path2D();
      let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
      for (const ring of rings) {
        ring.forEach(([x, y], i) => { i ? p.lineTo(x, y) : p.moveTo(x, y); x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y); });
        p.closePath();
      }
      return { path: p, rings, box: [x0, y0, x1, y1] };
    }

    // Bare linen: tabby weave, slubbed threads, creases and faint foxing.
    // Drawn in device pixels so the weave stays crisp under any transform.
    function linen(x, y, w, h) {
      const L = wool("linen");
      const m = ctx.getTransform();
      const dx = m.a * x + m.e, dy = m.d * y + m.f, dw = w * m.a, dh = h * m.d;
      ctx.save();
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.beginPath(); ctx.rect(dx, dy, dw, dh); ctx.clip();
      const pitch = P.weave * P.zoom, T = 256;
      const tile = document.createElement("canvas"); tile.width = tile.height = T;
      const g = tile.getContext("2d"), img = g.createImageData(T, T), d = img.data;
      const nT = Math.ceil(T / pitch) + 1, rows = [], cols = [];
      for (let i = 0; i < nT; i++) { rows.push((R() - 0.5) * 2 * P.slub); cols.push((R() - 0.5) * 2 * P.slub * 0.6); }
      for (let py = 0; py < T; py++) for (let px = 0; px < T; px++) {
        const ty = Math.floor(py / pitch), tx = Math.floor(px / pitch);
        const over = (tx + ty) & 1;
        const fy = (py / pitch) % 1, fx = (px / pitch) % 1;
        const round = over ? Math.sin(fy * Math.PI) : Math.sin(fx * Math.PI);
        const v = 1 + (over ? rows[ty] : cols[tx]) + (round - 0.6) * 0.03 + (R() - 0.5) * 0.02;
        const o = (py * T + px) * 4;
        d[o] = L.mid[0] * v; d[o + 1] = L.mid[1] * v; d[o + 2] = L.mid[2] * v; d[o + 3] = 255;
      }
      g.putImageData(img, 0, 0);
      ctx.fillStyle = ctx.createPattern(tile, "repeat");
      ctx.fillRect(dx, dy, dw, dh);
      // long slubs: single weft threads lighter or darker across the cloth
      for (let yy = dy; yy < dy + dh; yy += pitch) {
        if (R() < 0.1) { ctx.fillStyle = `rgba(${R() < 0.5 ? "250,245,235" : "110,95,80"},${0.03 + R() * 0.05})`; ctx.fillRect(dx, yy, dw, pitch * (0.6 + R())); }
      }
      // broad uneven tone, like old cloth under gallery light
      for (let i = 0; i < 18; i++) {
        const sx = dx + R() * dw, sy = dy + R() * dh, r = (200 + R() * 500) * P.zoom;
        const gr = ctx.createRadialGradient(sx, sy, 0, sx, sy, r);
        const c = R() < 0.5 ? "255,250,242" : "140,125,110";
        gr.addColorStop(0, `rgba(${c},${0.05 + R() * 0.05})`); gr.addColorStop(1, `rgba(${c},0)`);
        ctx.fillStyle = gr; ctx.fillRect(sx - r, sy - r, r * 2, r * 2);
      }
      // creases: long soft horizontal folds, a dark valley with a lit ridge above
      const C = palette.crease, sh = hexRgb(C.shadow), dp = hexRgb(C.deep), hl = hexRgb(C.light);
      const n = Math.round(P.creases * dh / 1000 / P.zoom);
      for (let i = 0; i < n; i++) {
        const cy = dy + R() * dh, len = dw * (0.12 + R() * 0.5), cx = dx + R() * dw - len * 0.3;
        const amp = (1 + R() * 3) * P.zoom, ph = R() * 6, freq = (0.004 + R() * 0.006) / P.zoom;
        const depth = P.creaseDepth * (0.35 + R() * 0.65), tilt = (R() - 0.5) * 0.03;
        const path = off => {
          const p = new Path2D();
          for (let s = 0; s <= 60; s++) {
            const xx = cx + len * s / 60, yy = cy + off + Math.sin(xx * freq + ph) * amp + (xx - cx) * tilt;
            s ? p.lineTo(xx, yy) : p.moveTo(xx, yy);
          }
          return p;
        };
        // fade in and out along the crease
        const fade = col => {
          const gr = ctx.createLinearGradient(cx, 0, cx + len, 0);
          gr.addColorStop(0, `rgba(${col},0)`); gr.addColorStop(0.2 + R() * 0.2, `rgba(${col},1)`);
          gr.addColorStop(0.6 + R() * 0.2, `rgba(${col},1)`); gr.addColorStop(1, `rgba(${col},0)`);
          return gr;
        };
        ctx.lineCap = "round";
        ctx.globalAlpha = 0.5 * depth;
        ctx.filter = `blur(${(1.6 * P.zoom).toFixed(1)}px)`;
        ctx.strokeStyle = fade(sh.join(",")); ctx.lineWidth = 4 * P.zoom; ctx.stroke(path(0));
        ctx.strokeStyle = fade(hl.join(",")); ctx.lineWidth = 3 * P.zoom; ctx.stroke(path(-3.5 * P.zoom));
        ctx.filter = "none";
        ctx.globalAlpha = 0.45 * depth;
        ctx.strokeStyle = fade(dp.join(",")); ctx.lineWidth = 0.9 * P.zoom; ctx.stroke(path(0.6 * P.zoom));
        ctx.globalAlpha = 1;
      }
      // foxing: small faint warm spots
      const ns = Math.round(P.stains * dw * dh / 1e6 / P.zoom / P.zoom);
      for (let i = 0; i < ns; i++) {
        const sx = dx + R() * dw, sy = dy + R() * dh, r = (3 + R() * 14) * P.zoom;
        const gr = ctx.createRadialGradient(sx, sy, 0, sx, sy, r);
        gr.addColorStop(0, `rgba(150,115,80,${0.05 + R() * 0.08})`); gr.addColorStop(1, "rgba(150,115,80,0)");
        ctx.fillStyle = gr; ctx.fillRect(sx - r, sy - r, r * 2, r * 2);
      }
      ctx.restore();
    }

    // Laid-and-couched fill. Long laid threads side by side (each thread one
    // shade with a slow drift), then couching threads laid across them at
    // intervals, each tacked down by small stitches that alternate from bar
    // to bar. o.angle: laid direction (0 horizontal). o.bars: wool for the
    // couching threads (default the fill's own, darker). o.phase: bar offset.
    function couch(shape, name, o = {}) {
      const u = unit(), tw = P.thread * u * (o.scale || 1);
      const a = o.angle || 0, ca = Math.cos(a), sa = Math.sin(a);
      const [bx0, by0, bx1, by1] = shape.box;
      const cx = (bx0 + bx1) / 2, cy = (by0 + by1) / 2, rad = Math.hypot(bx1 - bx0, by1 - by0) / 2 + tw * 2;
      const W = wool(name), sh = shades(name, 7, o.spread);
      ctx.save();
      ctx.clip(shape.path);
      ctx.fillStyle = rgbStr(mix(W.dark, [40, 32, 28], 0.12)); ctx.fill(shape.path);
      const toXY = (uu, vv) => [cx + uu * ca - vv * sa, cy + uu * sa + vv * ca];
      const pitch = tw * (1 + P.threadGap), barGap = tw * (o.barEvery || P.barEvery);
      const phase = (o.phase != null ? o.phase : R()) * barGap;
      const buckets = sh.map(() => new Path2D()), hi = new Path2D(), lo = new Path2D(), ticks = new Path2D();
      const seg = tw * 1.6; // laid threads are drawn in short pieces so the shade can drift
      for (let v = -rad; v <= rad; v += pitch) {
        let shade = pick(sh.length), wob = (R() - 0.5) * P.rowWobble * tw;
        let [px, py] = toXY(-rad, v + wob);
        for (let uu = -rad + seg; uu <= rad + seg; uu += seg) {
          if (R() < 0.12) shade = Math.max(0, Math.min(sh.length - 1, shade + (R() < 0.5 ? -1 : 1)));
          wob += (R() - 0.5) * 0.08 * tw; wob *= 0.9;
          const [qx, qy] = toXY(uu, v + wob);
          buckets[shade].moveTo(px, py); buckets[shade].lineTo(qx, qy);
          px = qx; py = qy;
        }
        // light on the upper edge of each thread, shadow under it
        const [h0x, h0y] = toXY(-rad, v - tw * 0.22), [h1x, h1y] = toXY(rad, v - tw * 0.22);
        hi.moveTo(h0x, h0y); hi.lineTo(h1x, h1y);
        const [l0x, l0y] = toXY(-rad, v + tw * 0.4), [l1x, l1y] = toXY(rad, v + tw * 0.4);
        lo.moveTo(l0x, l0y); lo.lineTo(l1x, l1y);
        // wool twist: faint slanted ticks along the thread
        if (P.fiber) for (let uu = -rad + R() * tw * 3; uu < rad; uu += tw * (2 + R() * 3) / P.fiber) {
          const [t0x, t0y] = toXY(uu - tw * 0.25, v - tw * 0.32), [t1x, t1y] = toXY(uu + tw * 0.25, v + tw * 0.32);
          ticks.moveTo(t0x, t0y); ticks.lineTo(t1x, t1y);
        }
      }
      ctx.lineCap = "butt";
      ctx.lineWidth = tw;
      buckets.forEach((b, i) => { ctx.strokeStyle = sh[i]; ctx.stroke(b); });
      ctx.globalAlpha = 0.18; ctx.strokeStyle = rgbStr(W.light); ctx.lineWidth = tw * 0.3; ctx.stroke(hi);
      ctx.globalAlpha = 0.22; ctx.strokeStyle = rgbStr(W.dark); ctx.lineWidth = tw * 0.25; ctx.stroke(lo);
      ctx.globalAlpha = 0.22; ctx.lineWidth = tw * 0.18; ctx.stroke(ticks);
      ctx.globalAlpha = 1;
      // couching threads across the laid work, wavering, then tacks
      const BW = wool(o.bars || name);
      const barShade = o.bars ? BW.mid : mix(BW.dark, BW.mid, 0.45);
      const bars = new Path2D(), barHi = new Path2D(), tacks = new Path2D();
      let bi = 0;
      for (let uu = -rad - phase; uu < rad; uu += barGap * (0.75 + R() * 0.5), bi++) {
        let off = (R() - 0.5) * 0.3 * tw;
        let [px, py] = toXY(uu + off, -rad);
        let ti = 0;
        for (let v = -rad; v <= rad; v += pitch, ti++) {
          off += (R() - 0.5) * 0.3 * tw; off *= 0.95;
          const [qx, qy] = toXY(uu + off, v);
          bars.moveTo(px, py); bars.lineTo(qx, qy);
          const [hx, hy] = toXY(uu + off - tw * 0.12, v);
          barHi.moveTo(hx, hy); barHi.lineTo(...toXY(uu + off - tw * 0.12, v + pitch * 0.5));
          px = qx; py = qy;
          if ((ti + bi) % 2 === 0) { // a tack every other row, staggered bar to bar
            const [k0x, k0y] = toXY(uu + off - tw * 0.42, v + pitch * 0.5), [k1x, k1y] = toXY(uu + off + tw * 0.42, v + pitch * 0.5);
            tacks.moveTo(k0x, k0y); tacks.lineTo(k1x, k1y);
          }
        }
      }
      ctx.lineCap = "round";
      ctx.strokeStyle = rgbStr(mix(barShade, [30, 25, 22], 0.25)); ctx.lineWidth = tw * P.barWidth * 1.25; ctx.globalAlpha = 0.35; ctx.stroke(bars);
      ctx.globalAlpha = 1; ctx.strokeStyle = rgbStr(barShade); ctx.lineWidth = tw * P.barWidth; ctx.stroke(bars);
      ctx.globalAlpha = 0.25; ctx.strokeStyle = rgbStr(BW.light); ctx.lineWidth = tw * 0.18; ctx.stroke(barHi);
      ctx.globalAlpha = 0.7; ctx.strokeStyle = rgbStr(mix(BW.dark, [20, 18, 16], 0.3)); ctx.lineWidth = tw * 0.3; ctx.stroke(tacks);
      ctx.restore();
    }

    // Stem stitch: overlapping slanted stitches along a polyline, which
    // reads as a twisted cord. o.w cord width, o.closed.
    function stem(pts, name, o = {}) {
      const u = unit(), w = (o.w || P.stemWidth) * u;
      const L = w * P.stemLength, step = w * P.stemStep, sl = o.slant != null ? o.slant : P.stemSlant;
      const sh = shades(name, 4, o.spread != null ? o.spread : 0.5);
      const samp = resample(pts, step, o.closed);
      const buckets = sh.map(() => new Path2D()), hi = new Path2D();
      const cs = Math.cos(sl), sn = Math.sin(sl);
      for (const [x, y, dx, dy] of samp) {
        const vx = (dx * cs - dy * sn) * L / 2, vy = (dx * sn + dy * cs) * L / 2;
        const jx = (R() - 0.5) * 0.15 * w, jy = (R() - 0.5) * 0.15 * w;
        const b = buckets[pick(buckets.length)];
        b.moveTo(x - vx + jx, y - vy + jy); b.lineTo(x + vx + jx, y + vy + jy);
        hi.moveTo(x - vx * 0.5 + jx - dy * w * 0.12, y - vy * 0.5 + jy + dx * w * 0.12); hi.lineTo(x + vx * 0.4 + jx - dy * w * 0.12, y + vy * 0.4 + jy + dx * w * 0.12);
      }
      ctx.save();
      ctx.lineCap = "round";
      ctx.lineWidth = w * 0.62;
      buckets.forEach((b, i) => { ctx.strokeStyle = sh[i]; ctx.stroke(b); });
      ctx.globalAlpha = 0.3; ctx.strokeStyle = rgbStr(wool(name).light); ctx.lineWidth = w * 0.2; ctx.stroke(hi);
      ctx.restore();
    }

    // Running stitch: even dashes with linen between.
    function running(pts, name, o = {}) {
      const u = unit(), len = (o.len || P.runLength) * u, gap = (o.gap || P.runGap) * u, w = (o.w || P.runWidth) * u;
      const samp = resample(pts, (len + gap) / 4, o.closed);
      const sh = shades(name, 3, 0.4), b = sh.map(() => new Path2D());
      for (let i = 0; i + 2 < samp.length; i += 4) {
        const p = samp[i], q = samp[i + 2];
        const path = b[pick(b.length)];
        path.moveTo(p[0], p[1]); path.lineTo(q[0] + (R() - 0.5) * w * 0.3, q[1] + (R() - 0.5) * w * 0.3);
      }
      ctx.save(); ctx.lineCap = "round"; ctx.lineWidth = w;
      b.forEach((p, i) => { ctx.strokeStyle = sh[i]; ctx.stroke(p); });
      ctx.restore();
    }

    // French knot: a small wound blob.
    function knot(x, y, name, r) {
      const u = unit(); r = (r || P.knot) * u;
      const W = wool(name);
      ctx.save();
      ctx.fillStyle = rgbStr(W.dark); ctx.beginPath(); ctx.arc(x, y, r, 0, 7); ctx.fill();
      ctx.strokeStyle = rgbStr(W.mid); ctx.lineWidth = r * 0.55;
      ctx.beginPath(); ctx.arc(x, y, r * 0.55, R() * 6, R() * 6 + 4.2); ctx.stroke();
      ctx.fillStyle = rgbStr(W.light); ctx.globalAlpha = 0.5; ctx.beginPath(); ctx.arc(x - r * 0.3, y - r * 0.3, r * 0.3, 0, 7); ctx.fill();
      ctx.restore();
    }

    // Filled-and-outlined motif: couched fill plus stem outline.
    function patch(rings, fill, line, o = {}) {
      const s = poly(...(typeof rings[0][0] === "number" ? [rings] : rings));
      couch(s, fill, o);
      for (const ring of s.rings) stem(ring, line, { closed: true, w: o.lineW });
      return s;
    }

    // Stitched capitals: Romanesque letter strokes in stem stitch, each
    // letter wandering a little. colors: one wool per word, cycling.
    function letters(text, x, y, cap, colors, o = {}) {
      let cx = x, word = 0;
      const tr = o.tracking != null ? o.tracking : 0.2;
      for (const chRaw of text) {
        const ch = chRaw === "U" ? "V" : chRaw;
        if (ch === " ") { cx += cap * 0.45; word++; continue; }
        const col = colors[word % colors.length];
        const g = GLYPHS[ch];
        const s = cap * (1 + (R() - 0.5) * 2 * P.letterJitter), by = y + (R() - 0.5) * 2 * P.letterJitter * cap * 0.6;
        const rot = (R() - 0.5) * 2 * P.letterTilt;
        if (ch === ":" || ch === "·") {
          const dots = ch === ":" ? [0.3, 0.75] : [0.55];
          for (const d of dots) knot(cx + cap * 0.12, by - cap + d * cap, col, cap * 0.07 / unit());
          cx += cap * 0.42; if (ch === ":") word++; continue;
        }
        if (!g) { cx += cap * 0.6; continue; }
        const [gw, strokes] = g;
        for (const st of strokes) {
          const pts = st.map(([px, py]) => {
            const lx = px * s, ly = (py - 1) * s;
            return [cx + lx * Math.cos(rot) - ly * Math.sin(rot), by + lx * Math.sin(rot) + ly * Math.cos(rot)];
          });
          stem(pts.length > 2 && st.curved ? curve(pts, 5) : pts, col, { w: cap * P.letterWeight / unit(), spread: 0.4 });
        }
        cx += (gw + tr) * s;
      }
      return cx;
    }
    function textWidth(text, cap, o = {}) {
      const tr = o.tracking != null ? o.tracking : 0.2;
      let w = 0;
      for (const chRaw of text) {
        const ch = chRaw === "U" ? "V" : chRaw;
        if (ch === " ") w += cap * 0.45; else if (ch === ":" || ch === "·") w += cap * 0.42; else w += ((GLYPHS[ch] || [0.4])[0] + tr) * cap;
      }
      return w;
    }

    // Border band between two red-brown rules: slanted couched bars in
    // turn, small beasts and birds facing in pairs between them, sprigs.
    // o.skip: [[x0, x1], ...] spans left bare (for HUD panels).
    function border(x, y, w, h, o = {}) {
      const u = unit();
      const rule = o.rule || "rule";
      const barW = P.borderBar * u, slant = o.slant != null ? o.slant : P.borderSlant;
      const cell = o.cell || h * 2.1;
      const barCols = o.barColors || ["tan", "rule", "slate", "terracotta", "navy", "ochre"];
      const beastCols = o.beastColors || [["terracotta", "navy"], ["tan", "rule"], ["slate", "tan"], ["navy", "buff"], ["olive", "rule"], ["ochre", "navy"]];
      const skipped = (x0, x1) => (o.skip || []).some(([s0, s1]) => x1 > s0 && x0 < s1);
      const inset = h * 0.06;
      let i = 0;
      const dir = o.alternate ? null : 1;
      for (let bx = x - cell * 0.3; bx < x + w + cell; bx += cell, i++) {
        const lean = (dir || (i % 2 ? -1 : 1)) * Math.tan(slant) * (h - inset * 2);
        const top = y + inset, bot = y + h - inset;
        const x0 = bx - lean / 2, x1 = bx + lean / 2;
        if (!skipped(Math.min(x0, x1) - barW, Math.max(x0, x1) + barW) && bx > x - barW && bx < x + w + barW) {
          const ax = -(bot - top), ay = (x1 - x0), al = Math.hypot(ax, ay), nx = ax / al * barW / 2, ny = ay / al * barW / 2;
          void nx;
          const ring = [[x0 - barW / 2, bot], [x1 - barW / 2, top], [x1 + barW / 2, top], [x0 + barW / 2, bot]];
          const col = barCols[i % barCols.length];
          ctx.save(); ctx.beginPath(); ctx.rect(x, y, w, h); ctx.clip();
          couch(poly(ring), col, { angle: Math.atan2(top - bot, x1 - x0), barEvery: 3.6 });
          stem(ring, i % 3 === 1 ? "navy" : "rule", { closed: true, w: P.stemWidth * 0.75 });
          ctx.restore();
        }
        // motif in the cell to the right of this bar
        const mx = bx + cell / 2, mw = cell - Math.abs(lean) - barW * 2;
        if (mx < x + w - cell * 0.2 && mx > x + cell * 0.2 && !skipped(mx - cell / 2, mx + cell / 2)) {
          const kind = (o.motifs || ["beast", "bird", "sprig", "bird", "beast", "sprig"])[i % 6];
          const facing = (Math.floor(i / 1) % 2) ? -1 : 1;
          const [fc, lc] = beastCols[(i * 5 + 1) % beastCols.length];
          const sz = Math.min(h * 0.78, mw * 0.8);
          motif(kind, mx, y + h - inset - h * 0.06, sz, fc, lc, facing);
        }
      }
      // the rules: thin red-brown stem lines along both edges, slightly uneven
      const ruleLine = yy => { const pts = []; for (let s = 0; s <= 24; s++) pts.push([x + w * s / 24, yy + (R() - 0.5) * 1.6 * u]); return pts; };
      stem(ruleLine(y), rule, { w: P.stemWidth * 0.8 });
      stem(ruleLine(y + h), rule, { w: P.stemWidth * 0.8 });
    }

    // Small border creatures, unit shapes facing right, feet at (0, 0).
    function motif(kind, x, y, size, fill, line, facing = 1) {
      const tf = pts => pts.map(([px, py]) => [x + px * size / 100 * facing, y + py * size / 100]);
      if (kind === "beast") {
        patch(tf(BEAST), fill, line);
        stem(curve(tf(BEAST_TAIL), 6), line, { w: P.stemWidth * 0.85 });
        knot(...tf([[40, -46]])[0], line, 1.4);
        stem(tf([[24, -34], [28, -24], [20, -20]]), line, { w: P.stemWidth * 0.6 });
      } else if (kind === "bird") {
        patch(tf(BIRD), fill, line);
        patch(tf(BIRD_WING), line === "navy" ? "tan" : "slate", line);
        for (const st of BIRD_STRIPES) stem(tf(st), fill, { w: P.stemWidth * 0.7 });
        stem(tf([[4, -6], [3, 0], [-5, 0]]), line, { w: P.stemWidth * 0.7 });
        stem(tf([[12, -7], [13, 0], [19, 0]]), line, { w: P.stemWidth * 0.7 });
        knot(...tf([[33, -43]])[0], line, 1.2);
      } else {
        stem(tf([[0, 0], [0, -34]]), line, { w: P.stemWidth * 0.75 });
        stem(curve(tf([[0, -14], [-10, -22], [-14, -30]]), 4), fill, { w: P.stemWidth * 0.75 });
        stem(curve(tf([[0, -18], [10, -26], [14, -34]]), 4), fill, { w: P.stemWidth * 0.75 });
        for (const p of [[0, -38], [-15, -33], [15, -37]]) knot(...tf([p])[0], fill, 2.4);
        stem(tf([[-8, 0], [8, 0]]), line, { w: P.stemWidth * 0.7 });
      }
    }

    return {
      P, R, ctx, wool, shades, poly, curve, ellipse, linen, couch, stem, running, knot, patch, letters, textWidth, border, motif,
      scale(s) { k = s; },
      unit,
    };
  }

  // --- border creature shapes (units of size/100, facing right)
  const BEAST = [[-40, -28], [-30, -36], [0, -37], [20, -36], [27, -44], [34, -53], [43, -52], [50, -46], [52, -40], [44, -38], [40, -31], [36, -16], [39, 0], [32, 0], [29, -16], [21, -20], [19, -3], [23, 0], [14, 0], [11, -20], [-14, -21], [-17, -3], [-13, 0], [-22, 0], [-24, -18], [-29, -14], [-29, 0], [-36, 0], [-38, -18], [-41, -24]];
  const BEAST_TAIL = [[-40, -27], [-50, -34], [-50, -48], [-42, -54], [-37, -47]];
  const BIRD = [[-38, -12], [-14, -19], [4, -21], [15, -29], [20, -41], [28, -48], [36, -46], [43, -42], [36, -39], [30, -33], [28, -21], [20, -10], [6, -4], [-10, -6], [-40, -4]];
  const BIRD_WING = [[0, -21], [-16, -46], [-6, -50], [-1, -43], [6, -57], [14, -52], [17, -28]];
  const BIRD_STRIPES = [[[-6, -42], [4, -24]], [[4, -50], [9, -26]]];

  // --- letter strokes: [advance, strokes]; x 0..advance, y 0 top .. 1 base
  const arc = (cx, cy, rx, ry, a0, a1, n = 9) => { const o = []; for (let i = 0; i <= n; i++) { const a = a0 + (a1 - a0) * i / n; o.push([cx + Math.cos(a) * rx, cy + Math.sin(a) * ry]); } return o; };
  const D = Math.PI / 180;
  const GLYPHS = {
    A: [0.78, [[[0, 1], [0.39, 0], [0.78, 1]], [[0.18, 0.62], [0.6, 0.62]]]],
    B: [0.66, [[[0.05, 0], [0.05, 1]], [[0.05, 0], [0.42, 0], ...arc(0.42, 0.24, 0.2, 0.24, -90 * D, 90 * D, 6), [0.05, 0.48]], [[0.05, 0.48], [0.45, 0.48], ...arc(0.45, 0.74, 0.22, 0.26, -90 * D, 90 * D, 6), [0.05, 1]]]],
    C: [0.7, [arc(0.42, 0.5, 0.38, 0.5, -50 * D, -310 * D, 12)]],
    D: [0.74, [[[0.05, 0], [0.05, 1]], [[0.05, 0], [0.3, 0], ...arc(0.3, 0.5, 0.42, 0.5, -90 * D, 90 * D, 10), [0.05, 1]]]],
    E: [0.6, [[[0.6, 0], [0.05, 0], [0.05, 1], [0.62, 1]], [[0.05, 0.5], [0.45, 0.5]]]],
    F: [0.58, [[[0.6, 0], [0.05, 0], [0.05, 1]], [[0.05, 0.5], [0.45, 0.5]]]],
    G: [0.74, [arc(0.42, 0.5, 0.38, 0.5, -50 * D, -300 * D, 12), [[0.45, 0.6], [0.78, 0.6], [0.78, 0.98]]]],
    H: [0.72, [[[0.05, 0], [0.05, 1]], [[0.67, 0], [0.67, 1]], [[0.05, 0.5], [0.67, 0.5]]]],
    h: [0.66, [[[0.05, 0], [0.05, 1]], [[0.05, 0.55], ...arc(0.34, 0.62, 0.28, 0.22, -170 * D, -10 * D, 6), [0.62, 1]]]],
    I: [0.16, [[[0.08, 0], [0.08, 1]]]],
    K: [0.66, [[[0.05, 0], [0.05, 1]], [[0.62, 0], [0.05, 0.56]], [[0.25, 0.4], [0.66, 1]]]],
    L: [0.56, [[[0.05, 0], [0.05, 1], [0.58, 1]]]],
    M: [0.92, [[[0.02, 1], [0.1, 0], [0.46, 0.72], [0.82, 0], [0.9, 1]]]],
    N: [0.74, [[[0.05, 1], [0.05, 0], [0.69, 1], [0.69, 0]]]],
    O: [0.8, [arc(0.4, 0.5, 0.38, 0.5, 0, 2 * Math.PI, 16)]],
    P: [0.62, [[[0.05, 1], [0.05, 0], [0.38, 0], ...arc(0.38, 0.25, 0.24, 0.25, -90 * D, 90 * D, 6), [0.05, 0.5]]]],
    R: [0.66, [[[0.05, 1], [0.05, 0], [0.38, 0], ...arc(0.38, 0.25, 0.24, 0.25, -90 * D, 90 * D, 6), [0.05, 0.5]], [[0.32, 0.5], [0.68, 1]]]],
    S: [0.6, [[[0.56, 0.1], [0.42, 0], [0.18, 0.02], [0.06, 0.16], [0.1, 0.36], [0.5, 0.58], [0.58, 0.8], [0.46, 0.98], [0.2, 1], [0.03, 0.9]]]],
    T: [0.72, [[[0, 0], [0.72, 0]], [[0.36, 0], [0.36, 1]]]],
    V: [0.74, [[[0, 0], [0.37, 1], [0.74, 0]]]],
    W: [1.1, [[[0, 0], [0.28, 1], [0.55, 0.2], [0.82, 1], [1.1, 0]]]],
    X: [0.7, [[[0, 0], [0.7, 1]], [[0.7, 0], [0, 1]]]],
    Y: [0.7, [[[0, 0], [0.35, 0.5], [0.7, 0]], [[0.35, 0.5], [0.35, 1]]]],
    0: [0.6, [arc(0.3, 0.5, 0.27, 0.5, 0, 2 * Math.PI, 14)]],
    1: [0.38, [[[0.05, 0.18], [0.26, 0], [0.26, 1]]]],
    2: [0.58, [[[0.04, 0.2], [0.2, 0.02], [0.42, 0.02], [0.55, 0.2], [0.5, 0.42], [0.04, 1], [0.6, 1]]]],
    3: [0.56, [[[0.04, 0.1], [0.26, 0], [0.48, 0.05], [0.54, 0.24], [0.28, 0.47], [0.54, 0.64], [0.54, 0.88], [0.3, 1], [0.03, 0.92]]]],
    4: [0.64, [[[0.5, 1], [0.5, 0], [0, 0.7], [0.66, 0.7]]]],
    5: [0.56, [[[0.56, 0], [0.12, 0], [0.08, 0.44], [0.4, 0.4], [0.56, 0.6], [0.54, 0.88], [0.3, 1], [0.03, 0.92]]]],
    6: [0.58, [[[0.52, 0.04], [0.28, 0.04], [0.07, 0.4], [0.07, 0.8], [0.28, 1], [0.5, 0.95], [0.58, 0.72], [0.46, 0.53], [0.18, 0.55], [0.07, 0.7]]]],
    7: [0.58, [[[0.02, 0], [0.58, 0], [0.2, 1]]]],
    8: [0.58, [arc(0.29, 0.25, 0.22, 0.25, 0, 2 * Math.PI, 10), arc(0.29, 0.74, 0.27, 0.26, 0, 2 * Math.PI, 10)]],
    9: [0.58, [[[0.06, 0.96], [0.3, 0.96], [0.51, 0.6], [0.51, 0.2], [0.3, 0], [0.08, 0.05], [0, 0.28], [0.12, 0.47], [0.4, 0.45], [0.51, 0.3]]]],
  };

  return { PARAMS, create, rng, curve, ellipse, GLYPHS };
})();
if (typeof module !== "undefined") module.exports = Stitch;
