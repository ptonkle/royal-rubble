// The trebuchet, drawn from the core's points (app/field.rs `Engine`,
// physics/trebuchet.rs `pose`): wheels, the base, the A-frame and its brace,
// the arm on its pivot, the counterweight box on its hinge, and the sling
// with the shot in its pouch. Every point, width and length comes from the
// core; field-draw.js lends its stitch helpers in `kit`.

function wheel(g, kit, [x, y], r) {
  g.save();
  g.beginPath();
  g.arc(x, y, r, 0, kit.look.full_turn);
  g.fillStyle = kit.pattern(g, "wool-olive");
  g.fill();
  g.restore();
  kit.place(g, "knot-navy", x, y, r / kit.sprites["knot-navy"].r);
}

function box(g, kit, corners, w) {
  g.save();
  g.beginPath();
  corners.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
  g.closePath();
  g.fillStyle = kit.pattern(g, "wool-terracotta");
  g.fill();
  g.restore();
  kit.dashed(g, [...corners, corners[0]], kit.wool.terracotta.dark, kit.wool.terracotta.light, w, w, 0);
}

export function drawTreb(g, e, kit) {
  const [f0, f1] = e.feet;
  const [l0, l1, base, arm, brace] = e.lens;
  e.wheels.forEach((w) => wheel(g, kit, w, e.wheel_r));
  kit.beam(g, f0, f1, e.arm_w, base, "base");
  kit.beam(g, f0, e.pivot, e.arm_w, l0, "frame");
  kit.beam(g, f1, e.pivot, e.arm_w, l1, "frame");
  kit.beam(g, e.brace[0], e.brace[1], e.arm_w, brace, "frame");
  // the hinge rod and the box under it
  kit.dashed(g, [e.weight, e.box[0]], kit.wool.navy.dark, kit.wool.navy.light, e.rope_w, e.rope_w, 0);
  kit.dashed(g, [e.weight, e.box[1]], kit.wool.navy.dark, kit.wool.navy.light, e.rope_w, e.rope_w, 0);
  box(g, kit, e.box, e.rope_w);
  kit.beam(g, e.weight, e.tip, e.arm_w, arm, "arm");
  kit.place(g, "knot-ochre", e.pivot[0], e.pivot[1], e.arm_w / kit.sprites["knot-ochre"].r);
  kit.place(g, "knot-navy", e.weight[0], e.weight[1], e.rope_w / kit.sprites["knot-navy"].r);
  kit.dashed(g, e.sling, kit.wool.tan.dark, kit.wool.tan.light, e.rope_w, e.rope_w, 0);
  if (e.loaded) {
    const s = e.loaded;
    kit.place(g, s.hot ? "ball-hot" : "ball", s.x, s.y, s.r / kit.sprites.ball.r);
  }
}
