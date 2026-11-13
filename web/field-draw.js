// The playfield's renderer: the one file a chosen look replaces. It draws
// the core's frame (crates/core/src/app/field.rs) on a 2D canvas. Every
// position, radius and line width comes from the core in pixels; colors come
// from theme.css (--field-*), and the few stroke styles from data/field.json.
// It decides nothing.

let look = null;
let colors = null;

// The stroke styles, from data/field.json.
export function setLook(table) {
  look = table;
}

// The theme's colors, read once (theme.css is the look's one file).
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
  const [h] = [f.size[1]];
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
  g.lineWidth = look.edge_width;
  g.stroke();
}

function trail(g, f) {
  if (!f.trail.length) return;
  poly(g, f.trail);
  g.strokeStyle = color("field-trail");
  g.lineWidth = look.trail_width;
  g.setLineDash(look.trail_dash);
  g.stroke();
  g.setLineDash([]);
}

function beams(g, f) {
  g.lineCap = "round";
  f.beams.forEach((b) => {
    g.strokeStyle = b.strain === undefined ? color("mat-" + b.material) : color("strain-" + b.strain);
    g.lineWidth = b.w;
    line(g, b.a, b.b);
    if (b.burning) {
      g.strokeStyle = color("field-fire");
      g.setLineDash(look.fire_dash);
      line(g, b.a, b.b);
      g.setLineDash([]);
    }
  });
  f.nodes.forEach((n) => {
    circle(g, n.x, n.y, n.r);
    g.fillStyle = color(n.hot ? "field-anchor" : "field-node");
    g.fill();
  });
  g.lineCap = "butt";
}

function royals(g, f) {
  f.royals.forEach((r) => {
    circle(g, r.x, r.y, r.r);
    g.fillStyle = color(r.dead ? "royal-dead" : "royal-" + r.rank);
    g.fill();
    g.strokeStyle = color("royal-edge");
    g.lineWidth = look.edge_width;
    g.stroke();
  });
}

function engine(g, f) {
  const e = f.engine;
  g.lineCap = "round";
  g.strokeStyle = color("engine-frame");
  g.lineWidth = e.arm_w;
  line(g, e.feet[0], e.pivot);
  line(g, e.feet[1], e.pivot);
  line(g, e.feet[0], e.feet[1]);
  g.strokeStyle = color("engine-arm");
  line(g, e.weight, e.tip);
  circle(g, e.weight[0], e.weight[1], e.weight_r);
  g.fillStyle = color("engine-weight");
  g.fill();
  g.lineCap = "butt";
  if (e.loaded) {
    circle(g, e.loaded.x, e.loaded.y, e.loaded.r);
    g.fillStyle = color(e.loaded.hot ? "field-fire" : "field-shot");
    g.fill();
  }
}

function balls(g, f) {
  f.debris.forEach((d) => {
    circle(g, d.x, d.y, d.r);
    g.fillStyle = color("field-debris");
    g.fill();
  });
  f.shots.forEach((s) => {
    circle(g, s.x, s.y, s.r);
    g.fillStyle = color(s.hot ? "field-fire" : "field-shot");
    g.fill();
  });
}

function marks(g, f) {
  if (f.ring) {
    circle(g, f.ring.x, f.ring.y, f.ring.r);
    g.strokeStyle = color(f.ring.ok ? "field-ring" : "field-bad");
    g.lineWidth = look.ring_width;
    g.setLineDash(look.ring_dash);
    g.stroke();
    g.setLineDash([]);
  }
  if (f.preview) {
    const p = f.preview;
    g.lineCap = "round";
    g.globalAlpha = look.preview_alpha;
    g.strokeStyle = p.ok ? color("mat-" + p.material) : color("field-bad");
    g.lineWidth = p.w;
    line(g, p.a, p.b);
    g.globalAlpha = 1;
    g.lineCap = "butt";
  }
}

function flag(g, f) {
  g.strokeStyle = color("flag-pole");
  g.lineWidth = look.pole_width;
  line(g, f.flag.pole[0], f.flag.pole[1]);
  poly(g, f.flag.cloth);
  g.closePath();
  g.fillStyle = color("flag-cloth");
  g.fill();
}

// Draws one frame on the canvas's 2D context.
export function drawField(g, f) {
  if (!look) return;
  sky(g, f);
  zone(g, f);
  // the arm swings down past the ground line: the ground hides that part
  engine(g, f);
  land(g, f);
  trail(g, f);
  beams(g, f);
  royals(g, f);
  balls(g, f);
  marks(g, f);
  flag(g, f);
}
