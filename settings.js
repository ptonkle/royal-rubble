// The player's sound settings as the core last gave them, for the sound
// module (web/sound/) to read. It decides nothing: the core turns each slider
// into a gain (crates/core/src/save.rs `master_gain`, `music_gain`) and hands
// it over as a decimal string, so no number lives here.
//
//   import { gains, onGains } from "./settings.js?v=2f288b949a2a";
//   gains()            -> { master: "0.150", music: "0.250" }
//   onGains(fn)        -> fn(gains()) now and on every change
//
// The music's gain sits under the main one: the sound module multiplies the
// two. The same values are on <html data-master-gain data-music-gain>.

let current = { master: "", music: "" };
const listeners = [];

export function gains() {
  return { ...current };
}

export function onGains(fn) {
  listeners.push(fn);
  if (current.master) fn(gains());
}

/** Called by app.js with each view model. */
export function takeGains(master, music) {
  if (master === current.master && music === current.music) return;
  current = { master, music };
  const root = document.documentElement.dataset;
  root.masterGain = master;
  root.musicGain = music;
  listeners.forEach((fn) => fn(gains()));
}
