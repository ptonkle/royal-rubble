// The trebuchet, drawn from the core's points (app/field.rs `Engine`,
// physics/trebuchet.rs `pose`) as a heavy timber engine (R1.3606): a second
// frame set back behind the first, thick uprights crossed by braces from
// each foot to the far end of the brace beam, spoked wheels, an iron-banded
// counterweight box, the arm with its iron bands, and a rope sling. Every
// point, width and length comes from the core; sizes are multiples of the
// core's widths from data/paint.json; field-draw.js lends its helpers in `kit`.

const mid = (a, b, k) => [(a[0] + b[0]) * k, (a[1] + b[1]) * k];

function frame(g, kit, e, w) {
  const [f0, f1] = e.feet;
  const [b0, b1] = e.brace;
  const [l0, l1, , , brace] = e.lens;
  const [x0, x1] = e.struts;
  kit.beam(g, f0, b1, w, x0, "frame");
  kit.beam(g, f1, b0, w, x1, "frame");
  kit.beam(g, f0, e.pivot, w, l0, "frame");
  kit.beam(g, f1, e.pivot, w, l1, "frame");
  kit.beam(g, b0, b1, w, brace, "frame");
}

function box(g, kit, corners, w) {
  const P = kit.P;
  g.save();
  g.beginPath();
  corners.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.closePath();
  g.fillStyle = kit.pattern(g, "beam-base");
  g.fill();
  g.lineJoin = "round";
  g.strokeStyle = P.iron;
  g.lineWidth = w * P.treb_band;
  g.stroke();
  const [c0, c1, c2, c3] = corners;
  const h = kit.half(1);
  g.beginPath();
  g.moveTo(...mid(c0, c3, h));
  g.lineTo(...mid(c1, c2, h));
  g.moveTo(...mid(c0, c1, h));
  g.lineTo(...mid(c3, c2, h));
  g.stroke();
  g.restore();
  corners.forEach(([x, y]) => kit.place(g, "knot-navy", x, y, (w * P.treb_rivet) / kit.sprites["knot-navy"].r));
}

export function drawTreb(g, e, kit) {
  const P = kit.P;
  const w = e.arm_w * P.treb_k;
  const [f0, f1] = e.feet;
  const [, , base, arm] = e.lens;
  // the back frame, set back up and to the side and in shade
  const off = e.arm_w * P.treb_back;
  g.save();
  g.translate(off, -off);
  // faded rather than filtered: a canvas filter cost a whole frame (R1.4600)
  g.globalAlpha = P.treb_back_alpha;
  frame(g, kit, e, w);
  g.restore();
  kit.beam(g, f0, f1, w, base, "base");
  e.wheels.forEach(([x, y]) => kit.place(g, "wheel", x, y, (e.wheel_r * P.treb_wheel) / kit.sprites.wheel.r));
  frame(g, kit, e, w);
  // the hinge straps and the box under them
  const [, , q0, q1, r0, r1] = e.struts;
  kit.beam(g, e.weight, e.box[0], e.rope_w * P.treb_strap, q0, "metal");
  kit.beam(g, e.weight, e.box[1], e.rope_w * P.treb_strap, q1, "metal");
  box(g, kit, e.box, e.rope_w * P.treb_strap);
  kit.beam(g, e.weight, e.tip, w, arm, "arm");
  kit.place(g, "knot-ochre", e.pivot[0], e.pivot[1], (w * P.treb_pin) / kit.sprites["knot-ochre"].r);
  kit.place(g, "knot-navy", e.weight[0], e.weight[1], (w * P.treb_pin) / kit.sprites["knot-navy"].r);
  const [s0, s1, s2] = e.sling;
  kit.beam(g, s0, s1, e.sling_w * P.treb_sling, r0, "rope");
  kit.beam(g, s1, s2, e.sling_w * P.treb_sling, r1, "rope");
  if (e.loaded) {
    const s = e.loaded;
    kit.place(g, s.hot ? "ball-hot" : "ball", s.x, s.y, (s.r * P.ball_k) / kit.sprites.ball.r);
  }
}
