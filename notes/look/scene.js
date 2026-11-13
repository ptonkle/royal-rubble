// Shared scene for the look mockups: one castle, trebuchet, shot and HUD,
// drawn by each style file. Canvas is 1920x1080. Procedural, no art files.
const W = 1920, H = 1080;

function rng(seed) {
  let s = seed >>> 0;
  return () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; };
}

// Terrain: left plateau (trebuchet), valley, moat trench, castle hill.
const GROUND = [
  [0, 770], [180, 766], [420, 762], [600, 772], [720, 800], [840, 842], [930, 858],
  [1000, 852], [1036, 846], [1056, 856], [1074, 884], [1100, 896], [1134, 898], [1160, 888],
  [1176, 860], [1188, 812], [1200, 752], [1216, 714], [1240, 701], [1500, 698], [1800, 700], [1860, 712], [1920, 730],
];
const MOAT = { x0: 1040, x1: 1190, top: 864, bottom: 900 };

// The castle, open-frame like a real build: pillars, slabs, braces, a rope.
// [x, y, w, h, material]
const BLOCKS = [
  // ground storey: stone pillars and a stone floor
  [1240, 520, 34, 180, "stone"], [1400, 520, 34, 180, "stone"],
  [1560, 520, 34, 180, "stone"], [1716, 520, 34, 180, "stone"],
  [1222, 500, 546, 22, "stone"],
  // second storey: wooden posts and floor
  [1262, 362, 18, 138, "wood"], [1470, 362, 18, 138, "wood"], [1668, 362, 18, 138, "wood"],
  [1244, 344, 462, 18, "wood"],
  // keep tower: stone
  [1502, 200, 28, 144, "stone"], [1640, 200, 28, 144, "stone"],
  [1490, 182, 190, 20, "stone"],
  // left turret
  [1244, 262, 70, 82, "stone"],
];
// diagonal braces [x1, y1, x2, y2, material, thickness]
const BRACES = [
  [1280, 362, 1470, 498, "wood", 12],
  [1488, 362, 1668, 498, "wood", 12],
  [1434, 698, 1560, 522, "metal", 12],
];
const CRENELS = [ // [x, y, w, count] tops with merlons
  [1490, 182, 190, 5], [1244, 262, 70, 2],
];
const ROPES = [[1680, 186, 1830, 704]];
const ROOF = [[1236, 262], [1279, 196], [1322, 262]]; // turret's wooden roof
const FLAG = { x: 1585, y: 182, h: 96 };

// Royals stand on floors: [x, footY, kind]
const ROYALS = [
  [1330, 700, "squire"], [1650, 700, "princess"],
  [1360, 500, "queen"], [1580, 500, "prince"],
  [1585, 344, "king"],
];

const TREB = { pivot: [352, 508], baseY: 762, angle: -58 * Math.PI / 180, long: 290, short: 86 };
// shot path: release near the sling, toward the keep
const SHOT = { x: 860, y: 318, r: 15 };
function arc(t) { // the shot's path, t 0..1 from the sling to the keep
  const x0 = 590, y0 = 250, x1 = 1580, y1 = 300, peak = 190;
  return [x0 + (x1 - x0) * t, y0 + (y1 - y0) * t - 4 * peak * t * (1 - t)];
}
function ghostArc(t) { // last shot's path, falling short in the moat
  const x0 = 590, y0 = 250, x1 = 1112, y1 = 866, peak = 210;
  return [x0 + (x1 - x0) * t, y0 + (y1 - y0) * t - 4 * peak * t * (1 - t)];
}
const SHOT_T = 0.36;

function trebParts() {
  const [px, py] = TREB.pivot, a = TREB.angle;
  const longEnd = [px + Math.cos(a) * TREB.long, py + Math.sin(a) * TREB.long];
  const shortEnd = [px - Math.cos(a) * TREB.short, py - Math.sin(a) * TREB.short];
  const pouch = [longEnd[0] + 92, longEnd[1] + 6];
  return { px, py, longEnd, shortEnd, pouch };
}

function groundY(x) {
  for (let i = 1; i < GROUND.length; i++) {
    const [x0, y0] = GROUND[i - 1], [x1, y1] = GROUND[i];
    if (x <= x1) return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
  }
  return GROUND[GROUND.length - 1][1];
}

function groundPath(ctx, bottom = H) {
  ctx.beginPath();
  ctx.moveTo(0, bottom);
  for (const [x, y] of GROUND) ctx.lineTo(x, y);
  ctx.lineTo(W, bottom);
  ctx.closePath();
}

// HUD values (mockup only)
const HUD = { phase: "Build", time: "0:42", gold: 14, mats: [["Wood", 1, 12], ["Stone", 3, 6], ["Metal", 5, 0]] };
