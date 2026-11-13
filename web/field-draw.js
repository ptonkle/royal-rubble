// The playfield's renderer: the one file a chosen look replaces. It draws
// the core's frame (crates/core/src/app/field.rs) on a 2D canvas. Every
// position, radius and line width comes from the core in pixels; colors come
// from field.css (--field-*), and the few stroke styles from data/field.json.
// It decides nothing. The look leans toward the Bayeux tapestry the person
// picked: `stem` and the running-stitch trail follow notes/look/tapestry.html.

let look = null;
let colors = null;

// The stroke styles, from data/field.json.
export function setLook(table) {
  look = table;
}

// The field's colors, read once (field.css).
function color(name) {
  if (!colors) colors = {};
  if (!(name in colors)) colors[name] = getComputedStyle(document.documentElement).getPropertyValue("--" + name).trim();
  return colors[name];
}

function line(g, a, b) {
  g.beginPath();
  g.moveTo(a[0], a[1]);
  g.lineTo(b[0], b[1]);
  g.stroke();
}

function circle(g, x, y, r) {
  g.beginPath();
  g.arc(x, y, r, 0, look.full_turn);
}

function dashed(g, dash) {
  g.setLineDash(dash);
  g.stroke();
  g.setLineDash([]);
}

// Stem stitch: a navy cord around a wool line, with a pale thread twisted
// along it (after the mockup's `stem`, without its color math).
function stem(g, a, b, w, wool) {
  g.lineCap = "round";
  g.strokeStyle = color("field-outline");
  g.lineWidth = w + look.outline;
  line(g, a, b);
  g.strokeStyle = wool;
  g.lineWidth = w;
  line(g, a, b);
  g.strokeStyle = color("field-thread");
  g.lineWidth = look.thread_width;
  dashed(g, look.thread_dash);
  g.lineCap = "butt";
}

// A filled circle with a stitched edge.
function bead(g, x, y, r, wool) {
  circle(g, x, y, r);
  g.fillStyle = wool;
  g.fill();
  g.strokeStyle = color("field-outline");
  g.lineWidth = look.edge_width;
  g.stroke();
}

function poly(g, points) {
  g.beginPath();
  points.forEach((p, i) => (i ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1])));
}

function sky(g, f) {
  g.fillStyle = color("field-sky");
  g.fillRect(0, 0, f.size[0], f.size[1]);
}

function zone(g, f) {
  if (!f.zone) return;
  const [l, t, r, b] = f.zone;
  g.fillStyle = color("field-zone");
  g.fillRect(l, t, r - l, b - t);
  if (f.grid > look.grid_min_px) {
    g.strokeStyle = color("field-grid");
    g.lineWidth = look.grid_width;
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
  g.strokeStyle = color("field-zone-edge");
  g.lineWidth = look.edge_width;
  g.setLineDash(look.zone_dash);
  g.strokeRect(l, t, r - l, b - t);
  g.setLineDash([]);
}

function land(g, f) {
  const h = f.size[1];
  if (f.water.length) {
    g.fillStyle = color("field-water");
    const [a, b] = f.water;
    g.fillRect(a[0], a[1], b[0] - a[0], h - a[1]);
  }
  const pts = f.ground;
  if (!pts.length) return;
  poly(g, pts);
  g.lineTo(pts[pts.length - 1][0], h);
  g.lineTo(pts[0][0], h);
  g.closePath();
  g.fillStyle = color("field-ground");
  g.fill();
  poly(g, pts);
  g.strokeStyle = color("field-ground-edge");
  g.lineWidth = look.ground_width;
  g.stroke();
  g.strokeStyle = color("field-thread");
  g.lineWidth = look.thread_width;
  dashed(g, look.thread_dash);
}

// The last shot's path as a running stitch.
function trail(g, f) {
  if (!f.trail.length) return;
  poly(g, f.trail);
  g.strokeStyle = color("field-trail");
  g.lineWidth = look.trail_width;
  g.lineCap = "round";
  dashed(g, look.trail_dash);
  g.lineCap = "butt";
}

function beams(g, f) {
  f.beams.forEach((b) => {
    stem(g, b.a, b.b, b.w, b.strain === undefined ? color("mat-" + b.material) : color("strain-" + b.strain));
    if (b.burning) {
      g.strokeStyle = color("field-fire");
      g.lineWidth = b.w;
      g.beginPath();
      g.moveTo(b.a[0], b.a[1]);
      g.lineTo(b.b[0], b.b[1]);
      dashed(g, look.fire_dash);
    }
  });
  f.nodes.forEach((n) => bead(g, n.x, n.y, n.r, color(n.hot ? "field-anchor" : "field-node")));
}

function royals(g, f) {
  f.royals.forEach((r) => bead(g, r.x, r.y, r.r, color(r.dead ? "royal-dead" : "royal-" + r.rank)));
}

function engine(g, f) {
  const e = f.engine;
  const frame = color("engine-frame");
  stem(g, e.feet[0], e.pivot, e.arm_w, frame);
  stem(g, e.feet[1], e.pivot, e.arm_w, frame);
  stem(g, e.feet[0], e.feet[1], e.arm_w, frame);
  stem(g, e.weight, e.tip, e.arm_w, color("engine-arm"));
  bead(g, e.weight[0], e.weight[1], e.weight_r, color("engine-weight"));
  if (e.loaded) bead(g, e.loaded.x, e.loaded.y, e.loaded.r, color(e.loaded.hot ? "field-fire" : "field-shot"));
}

function balls(g, f) {
  f.debris.forEach((d) => bead(g, d.x, d.y, d.r, color("field-debris")));
  f.shots.forEach((s) => bead(g, s.x, s.y, s.r, color(s.hot ? "field-fire" : "field-shot")));
}

function marks(g, f) {
  if (f.ring) {
    circle(g, f.ring.x, f.ring.y, f.ring.r);
    g.strokeStyle = color(f.ring.ok ? "field-ring" : "field-bad");
    g.lineWidth = look.ring_width;
    dashed(g, look.ring_dash);
  }
  if (f.preview) {
    const p = f.preview;
    g.globalAlpha = look.preview_alpha;
    stem(g, p.a, p.b, p.w, p.ok ? color("mat-" + p.material) : color("field-bad"));
    g.globalAlpha = 1;
  }
}

function flag(g, f) {
  if (!f.flag) return;
  g.strokeStyle = color("flag-pole");
  g.lineWidth = look.pole_width;
  line(g, f.flag.pole[0], f.flag.pole[1]);
  poly(g, f.flag.cloth);
  g.closePath();
  g.fillStyle = color("flag-cloth");
  g.fill();
  g.strokeStyle = color("field-outline");
  g.lineWidth = look.edge_width;
  g.stroke();
}

// The map's spots: a band of wool laid along the ground.
function spots(g, f) {
  (f.spots || []).forEach((s) => {
    g.strokeStyle = color("spot-" + s.kind);
    g.lineWidth = s.w;
    g.beginPath();
    g.moveTo(s.a[0], s.a[1]);
    g.lineTo(s.b[0], s.b[1]);
    dashed(g, look.spot_dash);
  });
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
  drawField(g, f.inset.frame);
  g.restore();
  g.strokeStyle = color("field-outline");
  g.lineWidth = look.inset_edge;
  g.strokeRect(x, y, w, h);
}

// Draws one frame on the canvas's 2D context.
export function drawField(g, f) {
  if (!look) return;
  sky(g, f);
  zone(g, f);
  // the arm swings down past the ground line: the ground hides that part
  engine(g, f);
  land(g, f);
  spots(g, f);
  trail(g, f);
  beams(g, f);
  royals(g, f);
  balls(g, f);
  marks(g, f);
  flag(g, f);
  inset(g, f);
}
