// Writes the menu chrome's tapestry art into web/look.css: the linen, the
// border band and the wool patches buttons are stitched from, each an SVG
// data URL in a custom property (--look-linen, --look-border, --look-wool-*)
// that theme.css uses. Data URLs keep the art in a stamped stylesheet and
// put no address in web/ (online_fence.rs).
//
//     node tools/look/tapestry_svgs.mjs
//
// The stitch drawing (laid(), stem()) is ported from notes/look/tapestry.html;
// the colors and the look follow photographs of the real tapestry
// (../ctc-journal/img/look/ref/): a faded palette read from data/look.json,
// dense laid-and-couched fills, thin outlines in a contrasting thread, and a
// sparse border of thin rules, slanted bars and small facing beasts.
// Generated files are committed; rerun this after changing it.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const C = JSON.parse(readFileSync(join(ROOT, "data", "look.json"), "utf8")).palette;

function rng(seed) {
  let s = seed >>> 0;
  return () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; };
}
function shade(hex, k) {
  const n = parseInt(hex.slice(1), 16), r = n >> 16, g = (n >> 8) & 255, b = n & 255;
  const f = v => Math.max(0, Math.min(255, Math.round(v * k)));
  return "#" + [r, g, b].map(v => f(v).toString(16).padStart(2, "0")).join("");
}
const r1 = v => Math.round(v * 10) / 10;
const poly = pts => "M" + pts.map(([x, y]) => `${r1(x)} ${r1(y)}`).join("L") + "Z";
// the thread an outline is worked in, by fill: the tapestry rarely outlines
// a shape in its own color, and often not in dark blue either
const EDGE = { [C.brick]: C.olive, [C.tan]: C.brick, [C.navy]: C.buff, [C.slate]: C.navy, [C.olive]: C.navy, [C.sage]: C.brick, [C.buff]: C.slate };

let clipN = 0;
// laid-and-couched fill: close rows of wool at `angle`, tied down by short
// couching stitches across them; rows cover a disc of `reach` round (cx, cy)
function laid(R, d, col, angle, cx, cy, reach, gap = 2.2) {
  const id = "c" + clipN++;
  const s = Math.sin(angle), co = Math.cos(angle);
  let out = `<clipPath id="${id}"><path d="${d}"/></clipPath><g clip-path="url(#${id})"><path d="${d}" fill="${col}"/><g stroke-width="1.5" opacity=".6">`;
  for (let t = -reach; t < reach; t += gap) {
    const c = R() < .5 ? shade(col, 1.14) : shade(col, .86);
    out += `<line x1="${r1(cx - co * reach - s * t)}" y1="${r1(cy - s * reach + co * t)}" x2="${r1(cx + co * reach - s * t)}" y2="${r1(cy + s * reach + co * t)}" stroke="${c}"/>`;
  }
  out += `</g><g stroke="${shade(col, .7)}" stroke-width="1.1" stroke-dasharray="1.6 2.6" opacity=".75">`;
  for (let t = -reach; t < reach; t += 6) {
    out += `<line x1="${r1(cx - s * reach + co * t)}" y1="${r1(cy + co * reach + s * t)}" x2="${r1(cx + s * reach + co * t)}" y2="${r1(cy - co * reach + s * t)}"/>`;
  }
  return out + "</g></g>";
}
// stem stitch: a thin twisted cord, lighter slanted stitches along it
function stem(d, col, w = 2) {
  return `<path d="${d}" fill="none" stroke="${col}" stroke-width="${w}" stroke-linejoin="round" stroke-linecap="round"/>` +
    `<path d="${d}" fill="none" stroke="${shade(col, 1.45)}" stroke-width="${r1(w * .45)}" stroke-dasharray="2 2.6" opacity=".6" stroke-linecap="round"/>`;
}
const svg = (w, h, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>\n`;
const props = [];
// percent-encodes what would end the url() or read as an address
const encode = text => text.trim().replace(/[%#<>"':]/g, c => "%" + c.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0"));
function write(name, text) {
  props.push(`  --look-${name}: url("data:image/svg+xml,${encode(text)}");`);
}

// --- linen: a fine plain weave with soft stains and long horizontal creases.
// Creases wave once per tile width, so the tile repeats without a seam.
{
  const R = rng(1066), T = 480;
  let b = `<rect width="${T}" height="${T}" fill="${C.linen}"/>`;
  for (let y = 0; y < T; y += 2) b += `<rect y="${y}" width="${T}" height=".8" fill="#6e5a46" opacity="${(.025 + R() * .045).toFixed(3)}"/>`;
  for (let x = 0; x < T; x += 2) b += `<rect x="${x}" width=".8" height="${T}" fill="#fffaf2" opacity="${(.03 + R() * .05).toFixed(3)}"/>`;
  for (let i = 0; i < 5; i++) {
    const x = R() * T, y = R() * T, rx = 90 + R() * 140, ry = 20 + R() * 40;
    for (const dx of [-T, 0, T]) for (const dy of [-T, 0, T])
      b += `<ellipse cx="${r1(x + dx)}" cy="${r1(y + dy)}" rx="${r1(rx)}" ry="${r1(ry)}" fill="#8a7058" opacity=".025"/>`;
  }
  for (let i = 0; i < 9; i++) {
    const y = 20 + R() * (T - 40), amp = 1 + R() * 3, ph = R() * 6.283;
    let d = "";
    for (let x = 0; x <= T; x += 12) d += (x ? "L" : "M") + `${x} ${r1(y + Math.sin(x / T * 6.283 + ph) * amp)}`;
    b += `<path d="${d}" fill="none" stroke="#6e5a46" stroke-width="1.4" opacity="${(.08 + R() * .08).toFixed(2)}"/>`;
    b += `<path d="${d}" transform="translate(0 1.6)" fill="none" stroke="#fffaf2" stroke-width="1.2" opacity=".28"/>`;
  }
  write("linen", svg(T, T, b));
}

// --- border band: thin red-brown rules, slanted bars leaning in turn so they
// make open triangles, and a small beast or a sprig in every other one.
// Beasts face each other in pairs. One tile holds eight bars.
{
  const R = rng(1066), h = 96, P = 150, N = 8, Wd = P * N, top = 6, bot = h - 6;
  const BEAST = {
    lion: "M-30 4C-30-8-20-14-6-13L12-13C14-20 20-24 26-22C31-20 32-14 29-10L27-6C26 0 24 4 22 8L24 22L19 22L15 9L-12 9L-15 22L-20 22L-21 8C-26 6-30 8-34 2C-38-4-40-12-36-18C-37-10-34-4-30 4Z",
    bird: "M-24 10C-17 1-10-4-2-6C1-14 6-22 12-22C16-22 18-18 16-14L23-12L15-10C14-3 10 4 4 8L6 22L3 22L0 10L-4 10L-6 22L-9 22L-8 10C-14 10-21 13-28 17Z",
    hound: "M-32 0C-26-8-12-8 4-8L16-12C18-18 24-20 30-17L34-15L29-13C28-6 24-2 20 2L23 22L18 22L13 4L-12 4L-17 22L-22 22L-22 4C-28 4-34 6-38-2Z",
  };
  const WING = "M-6-4C0-12 8-14 12-8";
  const fills = [C.brick, C.tan, C.slate, C.olive, C.navy, C.sage];
  let b = stem(`M0 ${top}H${Wd}`, C.brick, 1.8) + stem(`M0 ${bot}H${Wd}`, C.brick, 1.8);
  const L = 56;
  for (let k = 0; k < N; k++) {
    const x = k * P, lean = k % 2 ? 1 : -1, foot = x + (lean < 0 ? L : 0), head = x + (lean < 0 ? 0 : L);
    const col = fills[(k * 2) % 6], bar = [[foot, bot - 1], [foot + 9, bot - 1], [head + 9, top + 1], [head, top + 1]];
    b += laid(R, poly(bar), col, Math.atan2(top - bot, head - foot), (foot + head) / 2 + 4, h / 2, 60) + stem(poly(bar), EDGE[col], 1.4);
    // the triangle opening upward between this bar and the next holds the motif
    const cx = x + L / 2 + P / 2, cy = h / 2 + (k % 2 ? 4 : 8);
    if (k % 4 === 1) { // a sprig: a stem and three small leaves
      b += stem(`M${cx} ${bot - 2}V${cy - 14}M${cx} ${cy - 2}l-7-6M${cx} ${cy - 2}l7-6M${cx} ${cy - 12}l-5-5M${cx} ${cy - 12}l5-5`, C.sage, 1.6);
      continue;
    }
    const kind = ["lion", "bird", "hound"][k % 3], fill = fills[(k * 2 + 1) % 6], s = .9, flip = k % 4 === 2 ? -1 : 1;
    b += `<g transform="translate(${cx} ${cy}) scale(${flip * s} ${s})">`;
    b += laid(R, BEAST[kind], fill, kind === "bird" ? .5 : 0, 0, 0, 44) + stem(BEAST[kind], EDGE[fill], 1.8);
    if (kind === "bird") b += stem(WING, EDGE[fill], 1.6);
    b += `<circle cx="${kind === "bird" ? 10 : kind === "lion" ? 24 : 26}" cy="${kind === "bird" ? -17 : -17}" r="1.6" fill="${C.navy}"/></g>`;
  }
  write("border", svg(Wd, h, b));
}

// --- wool patches: a laid-and-couched tile per color, for buttons and seats.
// 24 units: rows every 2, couching every 6, so the tile repeats cleanly.
for (const name of ["brick", "buff", "sage", "tan"]) {
  const R = rng(1066), T = 24, col = C[name];
  let b = `<rect width="${T}" height="${T}" fill="${col}"/><g stroke-width="1.4" opacity=".6">`;
  for (let y = 1; y < T; y += 2) b += `<line x1="0" y1="${y}" x2="${T}" y2="${y}" stroke="${R() < .5 ? shade(col, 1.14) : shade(col, .86)}"/>`;
  b += `</g><g stroke="${shade(col, .7)}" stroke-width="1.1" stroke-dasharray="1.6 2.4" opacity=".75">`;
  for (let x = 3; x < T; x += 6) b += `<line x1="${x}" y1="${(x % 4) * .5}" x2="${x}" y2="${T + 4}"/>`;
  write(`wool-${name}`, svg(T, T, b + "</g>"));
}

const css = `/* Generated by tools/look/tapestry_svgs.mjs from data/look.json: the
   tapestry art as data URLs. Do not edit; rerun the tool. */
:root {
${props.join("\n")}
}
`;
writeFileSync(join(ROOT, "web", "look.css"), css);
console.log("web/look.css", css.length, "bytes");
