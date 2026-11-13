// The synth: plays the compiled sounds with Web Audio, and decides nothing.
// There are no recordings. Ported from Fleeced's web/audio.js.
//
// tools/sound/compile.py turns data/sounds.json into data/sound-programs.json:
// every cue, loop and music track as layers (a wave, schedules for gain and
// frequency, filters, a slow wobble in pitch and a throb in level), all times
// in seconds, every value final. This file holds no numbers; tools/sound/
// render.py replays the same schedules offline, so keep the two in step.
//
// The page calls web/sound/sound.js, not this file.

// A parameter's schedule: { m: the AudioParam method, t: seconds, v, tc }.
function schedule(param, points, from) {
  (points || []).forEach((p) => param[p.m](p.v, from + p.t, p.tc));
}

function source(context, buffer, layer) {
  if (layer.wave === "noise") {
    const s = context.createBufferSource();
    s.buffer = buffer;
    s.loop = true;
    return s;
  }
  const o = context.createOscillator();
  if (layer.wave === "periodic") {
    const real = Float32Array.from(layer.periodic.real);
    const imag = Float32Array.from(layer.periodic.imag);
    o.setPeriodicWave(context.createPeriodicWave(real, imag, { disableNormalization: true }));
  } else {
    o.type = layer.wave;
  }
  o.frequency.value = 0;
  o.detune.value = 0;
  return o;
}

// A slow sine added to a parameter.
function lfo(context, rate, depth, param) {
  const o = context.createOscillator();
  const d = context.createGain();
  o.frequency.value = rate;
  d.gain.value = depth;
  o.connect(d);
  d.connect(param);
  return o;
}

// Builds one layer into `out`, its schedule counted from `from`. A steady
// layer (a loop's bed) runs until stopped. Returns what it started and when
// it ends.
export function buildLayer(context, buffer, layer, out, from, noiseAt) {
  const src = source(context, buffer, layer);
  const parts = [src];
  let tail = src;
  (layer.filters || []).forEach((f) => {
    const b = context.createBiquadFilter();
    b.type = f.type;
    b.Q.value = f.q;
    b.gain.value = f.gain;
    b.frequency.value = 0;
    schedule(b.frequency, f.freq, from);
    tail.connect(b);
    tail = b;
  });
  (layer.throb || []).forEach((t) => {
    const g = context.createGain();
    g.gain.value = 1;
    parts.push(lfo(context, t.rate, t.depth, g.gain));
    tail.connect(g);
    tail = g;
  });
  const level = context.createGain();
  level.gain.value = 0;
  schedule(level.gain, layer.gain, from);
  if (src.frequency) {
    schedule(src.frequency, layer.freq, from);
    (layer.wobble || []).forEach((w) => parts.push(lfo(context, w.rate, w.cents, src.detune)));
  }
  tail.connect(level);
  level.connect(out);
  const end = from + layer.stop;
  parts.forEach((p) => {
    p.start(from + layer.start, noiseAt);
    if (!layer.steady) p.stop(end);
  });
  return { parts, level, end: layer.steady ? null : end };
}

// Every layer of a one-shot program, starting at `from`.
export function buildProgram(context, buffer, program, out, from) {
  return program.layers.map((layer) => buildLayer(context, buffer, layer, out, from, program.noise_at));
}

// A safety net against many sounds at once: it leaves the mix alone until it
// nears full scale.
export function makeLimiter(context, spec) {
  const c = context.createDynamicsCompressor();
  c.threshold.value = spec.threshold;
  c.knee.value = spec.knee;
  c.ratio.value = spec.ratio;
  c.attack.value = spec.attack;
  c.release.value = spec.release;
  return c;
}

// A loop or music track that repeats with no seam: its steady layers start
// once and run on; its other layers are built a little ahead of time, rep
// after rep, so a long track never builds thousands of nodes at once.
export function startRepeat(context, buffer, program, out, lookahead) {
  const from = context.currentTime;
  const r = { program, out, origin: from, rep: 0, next: 0, live: [], lookahead };
  r.live = program.layers
    .filter((l) => l.steady)
    .map((l) => buildLayer(context, buffer, l, out, from, program.noise_at));
  r.moving = program.layers.filter((l) => !l.steady);
  feedRepeat(context, buffer, r);
  return r;
}

export function feedRepeat(context, buffer, r) {
  const now = context.currentTime;
  const horizon = now + r.lookahead;
  if (!r.moving.length) return;
  let base = r.origin + r.rep * r.program.length;
  while (base + r.moving[r.next].start < horizon) {
    r.live.push(buildLayer(context, buffer, r.moving[r.next], r.out, base, r.program.noise_at));
    r.next += 1;
    if (r.next === r.moving.length) {
      r.next = 0;
      r.rep += 1;
      base = r.origin + r.rep * r.program.length;
    }
  }
  r.live = r.live.filter((v) => v.end === null || v.end > now);
}

export function stopRepeat(r, at) {
  r.live.forEach((v) => v.parts.forEach((p) => p.stop(at)));
  r.live = [];
  r.moving = [];
}
