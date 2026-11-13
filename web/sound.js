// The game's sound, for the page: effects, loops and music. It decides
// nothing about the game; the shell and match screens say what to play.
// Every number is in data/sounds.json (compiled to data/sound-programs.json).
//
//   initSound()                 once, at start; sound wakes on the first press
//   play(name, opts)            a one-shot; name is a cue or a group
//                               (impact_wood, impact_stone, impact_metal,
//                               debris) or a tier (collapse, with opts.count);
//                               opts.gain scales it
//   startLoop(name, key)        a looping effect (shot_whoosh, fire_crackle),
//   stopLoop(key)               keyed so several can run at once
//   music(name)                 crossfades to music_build or music_siege;
//                               music(null) fades the music out
//   urgent(on)                  the last-seconds layer over the siege music
//   setVolume(fraction)         main volume, 0 to 1 (the slider's share)
//   setMusicVolume(fraction)    music volume, 0 to 1, under the main one
//   setEffectsVolume(fraction)  effects volume, 0 to 1, under the main one
//   soundLabels()               { id: label } for a sound list
import { buildProgram, makeLimiter, startRepeat, feedRepeat, stopRepeat } from "./synth.js";

let bank = null;
let byId = null;
let noiseBytes = null;
let ctx = null;
let noise = null;
let master = null;
let effects = null;
let musicBus = null;
let waiting = [];
let timer = null;
const turn = new Map();
const lastPlayed = new Map();
const loops = new Map();
const tracks = new Map();
let current = null;
const level = { main: 1, music: 1, effects: 1 };

function mark(name, value) {
  document.body.dataset[name] = value;
}

// A slider's share heard as loudness: on a square, as ears hear it.
function felt(fraction, scale) {
  return fraction * fraction * scale;
}

function applyLevels() {
  if (!ctx) return;
  const now = ctx.currentTime;
  const s = bank.setup;
  master.gain.setTargetAtTime(felt(level.main, s.master_scale), now, s.smooth);
  musicBus.gain.setTargetAtTime(felt(level.music, s.music_gain), now, s.smooth);
  effects.gain.setTargetAtTime(felt(level.effects, s.sfx_gain), now, s.smooth);
}

export function setVolume(fraction) {
  level.main = fraction;
  applyLevels();
}

export function setMusicVolume(fraction) {
  level.music = fraction;
  applyLevels();
}

export function setEffectsVolume(fraction) {
  level.effects = fraction;
  applyLevels();
}

// Whether sound can play now. What is asked for between the first press and
// the noise being ready waits for it.
function ready(then) {
  if (ctx && !noise) waiting.push(then);
  return ctx && noise;
}

// A group plays its cues in turn; a tier picks by count.
function resolve(name, opts) {
  const group = bank.groups[name];
  if (group) {
    const k = turn.get(name) || 0;
    turn.set(name, k + 1 < group.length ? k + 1 : 0);
    return group[k];
  }
  const tiers = bank.tiers[name];
  if (tiers) {
    const count = (opts && opts.count) || 1;
    let pick = tiers[0][1];
    tiers.forEach(([least, id]) => {
      if (count >= least) pick = id;
    });
    return pick;
  }
  return name;
}

export function play(name, opts) {
  if (!bank || !ready(() => play(name, opts))) return;
  const id = resolve(name, opts);
  const program = byId.get(id);
  if (!program) return;
  const now = ctx.currentTime;
  if (lastPlayed.has(id) && now - lastPlayed.get(id) < bank.setup.repeat_gap) return;
  lastPlayed.set(id, now);
  let out = effects;
  if (opts && opts.gain !== undefined) {
    out = ctx.createGain();
    out.gain.value = opts.gain;
    out.connect(effects);
  }
  buildProgram(ctx, noise, program, out, now);
  mark("lastSound", id);
}

export function startLoop(name, key) {
  if (!bank || !ready(() => startLoop(name, key))) return;
  const k = key || name;
  if (loops.has(k)) return;
  const program = byId.get(name);
  if (!program) return;
  loops.set(k, startRepeat(ctx, noise, program, effects, bank.setup.lookahead));
  tick();
}

export function stopLoop(key) {
  const r = loops.get(key);
  if (!r) return;
  stopRepeat(r, ctx.currentTime + bank.setup.smooth);
  loops.delete(key);
}

// One music track on its own fader, so two can cross.
function startTrack(id) {
  const bus = ctx.createGain();
  bus.gain.value = 0;
  bus.connect(musicBus);
  bus.gain.setTargetAtTime(1, ctx.currentTime, bank.setup.crossfade);
  const r = startRepeat(ctx, noise, byId.get(id), bus, bank.setup.lookahead);
  tracks.set(id, { bus, r });
}

function fadeTrack(id) {
  const t = tracks.get(id);
  if (!t) return;
  const now = ctx.currentTime;
  t.bus.gain.setTargetAtTime(0, now, bank.setup.crossfade);
  stopRepeat(t.r, now + bank.setup.fade_out);
  tracks.delete(id);
}

export function music(name) {
  if (!bank || !ready(() => music(name))) return;
  if (name === current) return;
  if (current) fadeTrack(current);
  urgentOff();
  current = name;
  if (name) startTrack(name);
  mark("music", name || "");
  tick();
}

function urgentOff() {
  fadeTrack(bank.setup.urgent_track);
}

export function urgent(on) {
  if (!bank || !ready(() => urgent(on))) return;
  const id = bank.setup.urgent_track;
  if (!on) urgentOff();
  else if (!tracks.has(id)) {
    startTrack(id);
    tracks.get(id).bus.gain.setTargetAtTime(bank.setup.urgent_gain, ctx.currentTime, bank.setup.crossfade);
  }
  tick();
}

function tick() {
  if (!ctx) return;
  loops.forEach((r) => feedRepeat(ctx, noise, r));
  tracks.forEach((t) => feedRepeat(ctx, noise, t.r));
}

export function soundLabels() {
  return bank ? bank.labels : {};
}

export function soundSections() {
  return bank ? bank.sections : [];
}

function wake() {
  if (!bank) return;
  if (!ctx) {
    ctx = new AudioContext();
    master = ctx.createGain();
    effects = ctx.createGain();
    musicBus = ctx.createGain();
    const limiter = makeLimiter(ctx, bank.setup.limiter);
    effects.connect(master);
    musicBus.connect(master);
    master.connect(limiter);
    limiter.connect(ctx.destination);
    applyLevels();
    timer = setInterval(tick, bank.setup.tick_ms);
    ctx.decodeAudioData(noiseBytes.slice(0)).then((buffer) => {
      noise = buffer;
      mark("audio", "started");
      waiting.forEach((then) => then());
      waiting = [];
    });
  }
  if (ctx.state === "suspended") ctx.resume();
}

// Loads the compiled sounds and the noise from data/. Browsers start sound
// only after a press, so the context starts on the first one. `base` is the
// path to data/ from the page, "data/" by default.
export async function initSound(base) {
  const at = base || "data/";
  bank = await (await fetch(at + "sound-programs.json")).json();
  noiseBytes = await (await fetch(at + "sound-noise.wav")).arrayBuffer();
  byId = new Map(bank.programs.map((p) => [p.id, p]));
  ["pointerdown", "pointerup", "keydown"].forEach((type) => window.addEventListener(type, wake, true));
  mark("audio", "waiting");
}

// For the preview page, which has its data inline.
export function initSoundFrom(theBank, theNoiseBytes) {
  bank = theBank;
  noiseBytes = theNoiseBytes;
  byId = new Map(bank.programs.map((p) => [p.id, p]));
  ["pointerdown", "pointerup", "keydown"].forEach((type) => window.addEventListener(type, wake, true));
  mark("audio", "waiting");
}
