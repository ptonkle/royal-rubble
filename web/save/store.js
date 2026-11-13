// The one page file that may touch browser storage (kits/saves, D17 changed).
// It keeps the text the core's save.rs writes and hands it back. It never
// looks inside: whether a save is usable, and what to do with one that is
// not, are the core's to decide.

const key = "save:" + location.pathname;

export function read() {
  try { return localStorage.getItem(key) || ""; } catch (e) { return ""; }
}

export function write(text) {
  try { localStorage.setItem(key, text); return true; } catch (e) { return false; }
}

// A save the core refused is kept beside the new game, never overwritten,
// so the player can take it to a build that reads it.
export function keepRefused(text) {
  try { localStorage.setItem(key + ":refused", text); } catch (e) { /* the file offer below still works */ }
}

export function refused() {
  try { return localStorage.getItem(key + ":refused") || ""; } catch (e) { return ""; }
}

// The long-term copy: a file the player keeps, and can load on any device.
export function saveFile(name, text) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
  a.download = name;
  a.click();
  URL.revokeObjectURL(a.href);
}

export function openFile() {
  return new Promise(resolve => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,application/json";
    input.onchange = () => (input.files[0] ? input.files[0].text().then(resolve) : resolve(""));
    input.click();
  });
}
