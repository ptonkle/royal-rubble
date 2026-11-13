// The page draws what the core returns and decides nothing.
//
// The core describes the screen (its name, panels and their items), and this
// file builds the DOM from that description without knowing what any panel
// means. Every control sends its action name and value back through `act`,
// the core's one door for buttons, settings, keys and screens; the core
// answers with the next view model. No state lives here: what is drawn is
// what the core last returned. After Crabble's page; the patching below keeps
// every element that is still there, so a redraw never loses a click, an open
// dropdown or the focus. Ported from Fleeced's page.
import init, { Game } from "./pkg/tool_shim.js";
import * as store from "./save/store.js";
import { pump, startNet } from "./online.js";
import { takeGains } from "./settings.js";
import { initField, showField } from "./field.js";
import { play } from "./sound.js";

const el = (id) => document.getElementById(id);
const make = (tag, cls, text) => {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
};

let game = null;
let ui = null;
let panelsText = "";
// the view model's text as last drawn: an unchanged one is not drawn again
let uiText = "";
// the save text as last written: an unchanged one is not written again
let saved = "";
// the core's count of volume slider moves, as last heard
let samplesSeen = null;

// ---- talking to the core ----

// What each menu element does, by its id: { click, change, input } handlers,
// made afresh with each build. The menus listen once, on their box, and look
// the target up here, so an element kept across a rebuild does the latest
// thing.
let handlers = {};

function on(id, type, fn) {
  if (!handlers[id]) handlers[id] = {};
  handlers[id][type] = fn;
}

function dispatch(e) {
  for (let t = e.target; t && t.id !== "menus"; t = t.parentElement) {
    const h = t.id && handlers[t.id] && handlers[t.id][e.type];
    if (h) {
      h(t, e);
      return;
    }
  }
}

function send(name, value) {
  draw(JSON.parse(game.act(name, value)));
}

// ---- items, one builder per type ----

// An icon from the core's set (app/hud_icon.rs): its shape is a mask and
// the stylesheet colors it by name.
function icon(url, name) {
  const i = make("span", "icon");
  i.style.setProperty("--icon", "url(\"" + url + "\")");
  if (name) i.dataset.icon = name;
  return i;
}

function button(item) {
  const b = make("button", item.quiet ? "button quiet" : "button");
  if (item.icon) {
    b.classList.add("with-icon");
    b.append(icon(item.icon, ""));
  }
  b.append(make("span", "button-text", item.text));
  b.type = "button";
  b.id = item.id;
  b.disabled = item.disabled === true;
  if (item.tip) b.title = item.tip;
  on(item.id, "click", () => send(item.id, ""));
  return b;
}

function slider(item) {
  const wrap = make("div", "field slider");
  const label = make("label", "", item.label);
  const input = make("input");
  input.type = "range";
  input.id = item.id;
  input.setAttribute("min", item.min);
  input.setAttribute("max", item.max);
  input.setAttribute("step", item.step);
  input.value = item.value;
  label.htmlFor = item.id;
  on(item.id, "input", (t) => send(item.id, t.value));
  const shown = make("span", "slider-value", item.text);
  shown.id = item.id + ".text";
  wrap.append(label, input, shown);
  return wrap;
}

function toggle(item) {
  const wrap = make("label", "field toggle");
  const input = make("input");
  input.type = "checkbox";
  input.id = item.id;
  input.checked = item.value;
  input.disabled = item.disabled === true;
  on(item.id, "change", (t) => send(item.id, t.checked ? "on" : "off"));
  wrap.append(input, make("span", "", item.label));
  return wrap;
}

function select(item) {
  const wrap = make("div", "field select");
  const input = make("select");
  item.options.forEach((o) => {
    const opt = make("option", "", o.text);
    opt.value = o.value;
    input.append(opt);
  });
  input.value = item.value;
  input.id = item.id;
  input.disabled = item.disabled === true;
  on(item.id, "change", (t) => send(item.id, t.value));
  if (item.label) {
    const label = make("label", "", item.label);
    label.htmlFor = item.id;
    wrap.append(label);
  }
  wrap.append(input);
  return wrap;
}

function text(item) {
  const wrap = make("div", "field text");
  const label = make("label", "", item.label);
  const input = make("input");
  input.type = "text";
  input.id = item.id;
  input.value = item.value;
  input.maxLength = item.max;
  input.spellcheck = false;
  input.autocomplete = "off";
  if (item.placeholder) input.placeholder = item.placeholder;
  label.htmlFor = item.id;
  on(item.id, "change", (t) => send(item.id, t.value));
  wrap.append(label, input);
  return wrap;
}

function number(item) {
  const wrap = make("div", "field number");
  const label = make("label", "", item.label);
  const input = make("input");
  // the core's text in place of the value: a disabled field showing words
  input.type = item.text ? "text" : "number";
  input.id = item.id;
  input.setAttribute("min", item.min);
  input.setAttribute("max", item.max);
  input.value = item.text || item.value;
  input.disabled = item.disabled === true;
  label.htmlFor = item.id;
  on(item.id, "change", (t) => send(item.id, t.value));
  wrap.append(label, input);
  return wrap;
}

// A long pasted code. A field sends on every change, so a paste is in the
// core before the button beside it is pressed.
function codeBox(item) {
  const wrap = make("div", "field code-box");
  const label = make("label", "", item.label);
  const box = make("textarea");
  box.id = item.id;
  box.value = item.value;
  box.spellcheck = false;
  box.autocomplete = "off";
  box.readOnly = item.read_only === true;
  label.htmlFor = item.id;
  if (!box.readOnly) on(item.id, "input", (t) => send(item.id, t.value));
  wrap.append(label, box);
  return wrap;
}

function lines(item) {
  const box = make("ul", "lines");
  if (item.id) box.id = item.id;
  item.lines.forEach((l) => box.append(make("li", "", l)));
  return box;
}

// Each version, its date and its lines (a minor version drawn smaller).
function notes(item) {
  const box = make("div", "notes");
  box.id = item.id;
  item.entries.forEach((e) => {
    const sec = make("section", e.minor ? "note minor" : "note");
    const head = make("h3", "note-head");
    head.append(make("span", "note-version", e.version), make("span", "note-date", e.date));
    const ul = make("ul");
    e.lines.forEach((l) => ul.append(make("li", "", l)));
    sec.append(head, ul);
    box.append(sec);
  });
  return box;
}

// Puts `text` on the clipboard. Without the clipboard (an insecure page, a
// refusal), the text is copied through a scratch field; if that fails too,
// the shown code, if any, is left selected for the reader to copy.
function clip(text, shown) {
  const fallback = () => {
    const box = make("textarea", "copy-scratch");
    box.value = text;
    box.readOnly = true;
    document.body.append(box);
    box.select();
    let done = false;
    try {
      done = document.execCommand("copy");
    } catch (e) {
      // the selection below is the last resort
    }
    box.remove();
    const again = shown && el(shown.id);
    if (!done && again) window.getSelection().selectAllChildren(again);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).catch(fallback);
  else fallback();
}

// A Copy button, and the text it copies shown big beside it when the core
// gives one (the code). The core says what to copy; the clipboard is the
// page's job. `body[data-copied]` holds what was copied, for the checks (a
// headless clipboard may refuse to be read).
function copy(item) {
  const b = make("button", "button quiet", item.text);
  b.type = "button";
  b.id = item.id;
  const code = item.value ? make("span", "copy-code", item.value) : null;
  on(item.id, "click", () => {
    document.body.dataset.copied = item.copy;
    clip(item.copy, code);
  });
  if (!code) return b;
  code.id = item.id + ".code";
  const wrap = make("div", "field copy-field");
  wrap.append(make("label", "", item.label), code, b);
  return wrap;
}

// A row of choices (the material bar, the shot bar): each shows its key
// small and sends its value; the chosen one is marked for the stylesheet.
function bar(item) {
  const row = make("div", "bar");
  row.id = item.id;
  let group = "";
  item.options.forEach((o) => {
    const b = make("button", "bar-option");
    b.type = "button";
    b.id = item.id + "." + o.value;
    b.disabled = o.disabled === true;
    if (o.selected) b.dataset.selected = "on";
    if (o.tip) b.title = o.tip;
    if (o.group) {
      if (group && o.group !== group) row.append(make("span", "bar-gap"));
      group = o.group;
    }
    b.append(make("span", "bar-key", o.key));
    if (o.icon) b.append(icon(o.icon, o.icon_name));
    if (o.count) b.append(make("span", "bar-count", o.count));
    b.append(make("span", "bar-text", o.text));
    const cost = make("span", "bar-cost");
    if (o.cost) cost.append(make("span", "coin"), make("span", "", o.cost));
    b.append(cost);
    on(b.id, "click", () => send(item.id, o.value));
    row.append(b);
  });
  return row;
}

// An upgrade card: name, tier and text; a press sends its value.
function card(item) {
  const b = make("button", "card");
  b.type = "button";
  b.id = item.id;
  b.disabled = item.disabled === true;
  b.dataset.tier = item.look;
  if (item.picked) b.dataset.picked = "on";
  if (item.icon) b.append(icon(item.icon, item.look));
  b.append(make("span", "card-name", item.name), make("span", "card-tier", item.tier), make("span", "card-text", item.text));
  on(item.id, "click", () => send("match.pick", item.value));
  return b;
}

// A top-bar value: icon and text.
function stat(item) {
  const box = make("div", "stat");
  box.id = item.id;
  if (item.tip) box.title = item.tip;
  box.append(icon(item.icon, item.icon_name), make("span", "stat-text", item.text));
  return box;
}

// The phase clock, big; the stylesheet pulses it in its last seconds.
function timer(item) {
  const box = make("div", "timer");
  box.id = item.id;
  if (item.urgent) box.dataset.urgent = "on";
  box.append(icon(item.icon, "timer"), make("span", "timer-phase", item.phase), make("span", "timer-text", item.text));
  return box;
}

// A filled bar with an icon, a label on the icon and a text (level and XP).
function meter(item) {
  const box = make("div", "meter");
  box.id = item.id;
  if (item.tip) box.title = item.tip;
  const badge = make("span", "meter-badge");
  badge.append(icon(item.icon, item.icon_name), make("span", "meter-label", item.label));
  const fill = make("progress");
  fill.max = item.max;
  fill.value = item.value;
  box.append(badge, fill, make("span", "meter-text", item.text));
  return box;
}

// Every player's health: a heart, the number, a bar and the name.
function health(item) {
  const box = make("ol", "health");
  box.id = item.id;
  item.rows.forEach((r) => {
    const row = make("li", "health-row");
    if (r.me) row.dataset.me = "on";
    if (r.out) row.dataset.out = "on";
    if (r.look) row.dataset.look = r.look;
    const fill = make("progress");
    fill.max = r.max;
    fill.value = r.value;
    row.append(icon(item.icon, "heart"), make("span", "health-value", r.text), make("span", "health-name", r.name), fill);
    box.append(row);
  });
  return box;
}

const builders = { stat, timer, meter, health, button, slider, toggle, select, text, number, lines, notes, copy, code_box: codeBox, bar, card };

// One lobby seat: who sits there and the controls this browser may use.
function seat(item) {
  const row = make("div", "seat");
  row.id = item.id;
  row.dataset.look = item.look;
  if (item.ready) row.dataset.ready = "on";
  if (item.me) row.dataset.me = "on";
  const who = make("div", "seat-who");
  who.append(make("span", "seat-name", item.name));
  if (item.note) who.append(make("span", "seat-note", item.note));
  row.append(who);
  if (item.role) row.append(make("span", "seat-role", item.role));
  const controls = make("div", "seat-controls");
  item.controls.forEach((c) => controls.append(builders[c.type](c)));
  row.append(controls);
  return row;
}

builders.seat = seat;

// A word that is a glossary term shows its definition on hover.
function hint(node) {
  const found = ui.hints.find((h) => h.term === node.textContent);
  // a core tip wins over the glossary
  if (found && !node.title) {
    node.dataset.term = found.term;
    node.title = found.definition;
  }
}

// The panels as the core gives them, built as a fresh tree off the page.
function buildMenus() {
  handlers = {};
  const box = make("div", "panels");
  (ui.panels || []).forEach((p) => {
    const panel = make("section", "panel " + p.kind);
    if (p.heading) panel.append(make(p.kind === "title" ? "h1" : "h2", "", p.heading));
    if (p.sub) panel.append(make("p", "sub", p.sub));
    p.items.forEach((item) => panel.append(builders[item.type](item)));
    box.append(panel);
  });
  box.querySelectorAll("h1, h2, label, button").forEach(hint);
  return box;
}

// ---- patching: keep every element that is still there ----

// Two nodes are the same element if they have the same tag and id.
function same(a, b) {
  return a.nodeType === b.nodeType && a.nodeName === b.nodeName && (a.nodeType !== Node.ELEMENT_NODE || a.id === b.id);
}

// Makes `old` (on the page) match `fresh` (just built), keeping each element
// that is still there: a rebuilt element under the mouse between press and
// release loses the click, a rebuilt select closes while open, and a rebuilt
// control loses the focus.
function patch(old, fresh) {
  if (old.nodeType !== Node.ELEMENT_NODE) {
    if (old.nodeValue !== fresh.nodeValue) old.nodeValue = fresh.nodeValue;
    return;
  }
  [...old.attributes].forEach((a) => {
    if (!fresh.hasAttribute(a.name)) old.removeAttribute(a.name);
  });
  [...fresh.attributes].forEach((a) => {
    if (old.getAttribute(a.name) !== a.value) old.setAttribute(a.name, a.value);
  });
  patchChildren(old, fresh);
  // the live state, after the children (a select's value needs its options)
  if (old.tagName === "INPUT" || old.tagName === "SELECT" || old.tagName === "TEXTAREA") {
    // a field being typed in keeps its text until it sends it (the lobby
    // redraws many times a second)
    if (old.value !== fresh.value && old !== editing) old.value = fresh.value;
    if (old.checked !== fresh.checked) old.checked = fresh.checked;
  }
  if (old.disabled !== undefined && old.disabled !== fresh.disabled) old.disabled = fresh.disabled;
}

function patchChildren(old, fresh) {
  const want = [...fresh.childNodes];
  want.forEach((f, i) => {
    const o = old.childNodes[i];
    if (!o) old.append(f);
    else if (same(o, f)) patch(o, f);
    else old.replaceChild(f, o);
  });
  while (old.childNodes.length > want.length) old.lastChild.remove();
}

// An open dropdown: the browser closes a select's list when the page around
// it changes, so the menus wait while one is open, and catch up when it
// closes.
let openSelect = null;

// The text field typed in since it last sent its text, if any.
let editing = null;

function selectClosed() {
  if (!openSelect) return;
  openSelect = null;
  drawMenus();
}

function watchSelects() {
  document.addEventListener("pointerdown", (e) => {
    const s = e.target.closest && e.target.closest("#menus select");
    if (s && s !== openSelect) openSelect = s;
    else selectClosed();
  }, true);
  // after the change is sent (a redraw first would put back the old value)
  document.addEventListener("change", () => setTimeout(selectClosed, 0), true);
  document.addEventListener("focusout", (e) => {
    if (e.target === openSelect) selectClosed();
  }, true);
  document.addEventListener("keydown", (e) => {
    if (!openSelect) {
      const open = e.key === " " || (e.key === "ArrowDown" && e.altKey);
      if (e.target.matches && e.target.matches("#menus select") && open) openSelect = e.target;
    } else if (e.key === "Escape" || e.key === "Enter" || e.key === "Tab") {
      setTimeout(selectClosed, 0);
    }
  }, true);
}

function drawMenus() {
  if (openSelect) {
    if (openSelect.isConnected) return;
    openSelect = null;
  }
  const text = JSON.stringify(ui.panels || []);
  if (text === panelsText) return;
  panelsText = text;
  const menus = el("menus");
  const kept = document.activeElement;
  const focused = kept && menus.contains(kept) ? kept.id : "";
  const caret = focused && kept.selectionStart !== undefined ? kept.selectionStart : null;
  patchChildren(menus, buildBox());
  // an element that had to be replaced gets the focus back (and a text field
  // its caret); when it is gone (a new screen), the screen's first control
  // takes the focus, so the keyboard carries on from there
  if (!focused || kept.isConnected) return;
  const again = el(focused) || menus.querySelector("button, input, select");
  if (!again) return;
  again.focus();
  if (caret !== null && again.setSelectionRange && again.type === "text") again.setSelectionRange(caret, caret);
}

// A throwaway parent, so the patch compares like with like.
function buildBox() {
  const outer = make("main");
  outer.append(buildMenus());
  return outer;
}

// ---- drawing the view model ----

// A data attribute, written only when it changes.
function mark(name, value) {
  const text = String(value);
  if (document.body.dataset[name] !== text) document.body.dataset[name] = text;
}

function draw(result) {
  if (result.error) {
    el("message").textContent = result.error;
    return;
  }
  ui = result.ok;
  // a save this build refused goes aside before anything is written over it
  if (ui.keep_refused) {
    store.keepRefused(ui.keep_refused);
    send("save.kept", "");
    return;
  }
  if (ui.offer_old_file) {
    store.saveFile("royal-rubble-old-save.json", store.refused());
    send("save.offered", "");
    return;
  }
  if (ui.save !== saved) {
    saved = ui.save;
    store.write(saved);
  }
  // drawn from an action's answer: the next frame's text is drawn afresh
  uiText = "";
  document.title = ui.title;
  mark("screen", ui.screen);
  mark("capture", ui.capture ? "on" : "");
  mark("sync", ui.sync);
  takeGains(ui.master_gain, ui.music_gain);
  if (ui.hash !== location.hash) history.replaceState(null, "", ui.hash || location.pathname + location.search);
  const icon = el("tab-icon");
  if (icon.getAttribute("href") !== ui.icon) icon.setAttribute("href", ui.icon);
  const version = el("version");
  if (version.textContent !== ui.version) version.textContent = ui.version;
  version.disabled = !ui.version_opens;
  version.hidden = !ui.version_shown;
  el("message").textContent = ui.status;
  drawMenus();
  showField(ui);
  // a volume slider moved: a sample at the new level
  if (samplesSeen !== null && ui.samples !== samplesSeen) play(ui.sample_cue);
  samplesSeen = ui.samples;
  // in a match the keys are the game's: a menu button left with the focus
  // would take Space or Enter as a press
  if (ui.capture && document.activeElement !== document.body) document.activeElement.blur();
}

// ---- input and the frame loop ----

function bindInput() {
  ["click", "change", "input"].forEach((type) => el("menus").addEventListener(type, dispatch));
  el("version").addEventListener("click", () => send("version", ""));
  watchSelects();
  document.addEventListener("input", (e) => {
    if (e.target.matches && e.target.matches("#menus input[type=text], #menus input[type=number]")) editing = e.target;
  }, true);
  // before the change is sent, so the core's answer is drawn into it
  document.addEventListener("change", (e) => {
    if (e.target === editing) editing = null;
  }, true);
  document.addEventListener("keydown", (e) => pressKey(e, true));
  document.addEventListener("keyup", (e) => pressKey(e, false));
  // Alt can take the window's focus, and its key-up then lands elsewhere
  window.addEventListener("blur", () => {
    held = 0;
  });
}

// The input bits held now, and those tapped since the last read, so a press
// released inside one frame still reaches the core. The core says which key
// is which bit.
let held = 0;
let tapped = 0;
// when the latest press went down (the event's timestamp): the core times
// a release within a tick from it
let pressedAt;

function pressKey(e, down) {
  // Esc does what the core names for this screen (pause, back)
  if (e.code === "Escape" && down && ui && ui.escape && !openSelect) {
    e.preventDefault();
    send(ui.escape, "");
    return;
  }
  const bit = ui && ui.capture && ui.keys ? ui.keys[e.code] : undefined;
  if (bit === undefined) return;
  e.preventDefault();
  held = down ? held | bit : held & ~bit;
  if (down) {
    tapped |= bit;
    pressedAt = e.timeStamp;
  }
}

function readPress() {
  return pressedAt;
}

function readKeys() {
  const bits = held | tapped;
  tapped = 0;
  return bits;
}

// Each frame: the view model again while it is live.
function frame() {
  pump();
  if (ui && ui.live) {
    const text = game.ui();
    if (text !== uiText) {
      draw(JSON.parse(text));
      uiText = text;
    }
  }
  requestAnimationFrame(frame);
}

// What the core cannot know: a seed and this tab's id (the core has no
// randomness), the build's stamp, the page's address, and the checks' flags.
function sendFlags() {
  const params = new URLSearchParams(location.search);
  send("flag.seed", String(crypto.getRandomValues(new Uint32Array(1))[0]));
  send("flag.client", crypto.randomUUID());
  send("flag.build", new URL(import.meta.url).searchParams.get("v") || "dev");
  send("flag.page_url", location.origin + location.pathname);
  // `?fast`: the browser checks skip what only makes a person wait
  if (params.has("fast")) send("flag.fast", "");
  // `?net=off`: rooms open with no transport, so a check can draw a lobby
  if (params.get("net") === "off") send("flag.net_off", "");
  // `?codes`: the invite-code buttons, for the online check and a blocked relay
  if (params.has("codes")) send("flag.codes", "");
}

async function main() {
  await init();
  const strings = await (await fetch("data/strings.json")).text();
  const notes = await (await fetch("data/patch-notes.json")).text();
  const glossary = await (await fetch("data/glossary.md")).text();
  game = new Game(strings, notes, glossary);
  const upgrades = await (await fetch("data/upgrades.json")).text();
  bindInput();
  await initField(send);
  // read before the first draw, which writes the address's hash
  const invited = new URLSearchParams(location.hash.slice(1)).get("room");
  // the settings from an earlier visit, checked by the core before the
  // first draw writes the defaults back
  saved = store.read();
  send("save.load", saved);
  if (ui === null) return;
  send("data.upgrades", upgrades);
  startNet(game, readKeys, readPress);
  sendFlags();
  if (invited) send("flag.room", invited);
  requestAnimationFrame(frame);
  document.body.dataset.ready = "true";
}

main();
