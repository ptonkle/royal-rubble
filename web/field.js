// The playfield's page side: the canvas, the pointer, and the sounds the core
// names. It decides nothing: pointer positions go to the core in pixels, the
// core answers with a frame (crates/core/src/app/field.rs), the renderer
// (field-draw.js) draws it, and each cue the core lists is played once.
import { drawField, setLook } from "./field-draw.js";
import * as sound from "./sound.js";

let canvas = null;
let g = null;
let send = null;
let sizeSent = "";
// the newest cue played: the core counts its cues up
let lastCue = null;
let loopsOn = new Set();
let showing = false;

// The pointer on the canvas, in its pixels (the core reads whole pixels).
function at(e) {
  return `${e.offsetX},${e.offsetY}`;
}

export async function initField(theSend) {
  send = theSend;
  canvas = document.getElementById("field");
  g = canvas.getContext("2d");
  const json = async (path) => (await fetch(path)).json();
  const names = ["field", "stitch", "palette", "stitch-sprites", "paint", "paint-sprites"];
  const [table, stitch, palette, baked, paint, painted] = await Promise.all(names.map((n) => json("data/" + n + ".json")));
  await setLook(table, stitch, palette, Object.assign(baked, painted), g, paint);
  sound.initSound();
  canvas.addEventListener("pointerdown", (e) => {
    canvas.setPointerCapture(e.pointerId);
    send("field.down", `${at(e)},${e.button}`);
  });
  canvas.addEventListener("pointermove", (e) => send("field.move", at(e)));
  canvas.addEventListener("pointerup", (e) => send("field.up", at(e)));
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
}

function fit() {
  const size = `${canvas.clientWidth},${canvas.clientHeight}`;
  if (size === sizeSent) return;
  sizeSent = size;
  canvas.width = canvas.clientWidth;
  canvas.height = canvas.clientHeight;
  send("field.size", size);
}

function sounds(f) {
  const newest = f.cues.length ? f.cues[f.cues.length - 1].n : 0;
  // cues from before this screen opened are not played
  if (lastCue !== null) f.cues.forEach((c) => c.n > lastCue && sound.play(c.name, c.count ? { count: c.count } : undefined));
  if (lastCue === null || newest > lastCue) lastCue = newest;
  const want = new Set(f.loops);
  want.forEach((name) => loopsOn.has(name) || sound.startLoop(name));
  loopsOn.forEach((name) => want.has(name) || sound.stopLoop(name));
  loopsOn = want;
  sound.music(f.music);
}

// Called with each view model: draws the field while the core gives one.
export function showField(ui) {
  if (!canvas) return;
  const f = ui.field;
  canvas.hidden = !f;
  if (!f) {
    if (showing) {
      loopsOn.forEach((name) => sound.stopLoop(name));
      loopsOn = new Set();
      sound.music(null);
      lastCue = null;
    }
    showing = false;
    return;
  }
  showing = true;
  fit();
  // where the buildable area is, for the browser checks to aim at
  canvas.dataset.zone = f.zone ? f.zone.join(",") : "";
  // decimal strings from the core ("0.800"); Web Audio's arithmetic reads them
  sound.setVolume(ui.sound_share);
  sound.setMusicVolume(ui.music_share);
  drawField(g, f);
  sounds(f);
}
