#!/usr/bin/env python3
"""Write notes/sound/preview.html: a listening page with every sound, one
button each, playing live through the game's own synth (web/sound/*.js) with
the compiled programs and noise inline, so the single file works anywhere.

    python3 tools/sound/author.py && python3 tools/sound/compile.py && python3 tools/sound/preview.py
"""
import base64
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]


def module_body(path):
    text = (ROOT / path).read_text()
    text = re.sub(r"^import .*?;\n", "", text, flags=re.M | re.S)
    return re.sub(r"^export ", "", text, flags=re.M)


PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Royal Rubble Sounds</title>
<style>
:root { --bg: #f4efe4; --ink: #2b2118; --muted: #6b5a48; --card: #fffaf0; --line: #d8cbb3; --accent: #8a2f1d; --on: #f6e2b8; }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { --bg: #1d1915; --ink: #efe6d6; --muted: #b3a48e; --card: #2a241e; --line: #4a3f33; --accent: #e08a62; --on: #4a3520; } }
:root[data-theme="dark"] { --bg: #1d1915; --ink: #efe6d6; --muted: #b3a48e; --card: #2a241e; --line: #4a3f33; --accent: #e08a62; --on: #4a3520; }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 16px/1.4 Georgia, "Times New Roman", serif; }
main { max-width: 960px; margin: 0 auto; padding: 24px 16px 48px; }
h1 { margin: 0 0 4px; font-size: 28px; }
p.lead { margin: 0 0 20px; color: var(--muted); }
.levels { display: flex; flex-wrap: wrap; gap: 16px 32px; margin: 0 0 24px; padding: 12px 16px; background: var(--card); border: 1px solid var(--line); border-radius: 8px; }
.levels label { display: flex; align-items: center; gap: 10px; }
h2 { font-size: 18px; margin: 24px 0 8px; color: var(--accent); }
.grid { display: flex; flex-wrap: wrap; gap: 8px; }
button { font: inherit; font-size: 15px; padding: 8px 14px; border: 1px solid var(--line); border-radius: 6px; background: var(--card); color: var(--ink); cursor: pointer; }
button:hover { border-color: var(--accent); }
button[aria-pressed="true"] { background: var(--on); border-color: var(--accent); }
button small { color: var(--muted); margin-left: 6px; }
</style>
</head>
<body>
<main>
<h1>Royal Rubble sounds</h1>
<p class="lead">Every sound is built from code as it plays; nothing is recorded. Loops and music play until pressed again. Music crossfades between tracks.</p>
<div class="levels">
<label>Volume <input id="vol" type="range" min="0" max="100" value="100"></label>
<label>Music <input id="mus" type="range" min="0" max="100" value="100"></label>
</div>
<div id="lists"></div>
</main>
<script type="module">
__SYNTH__
__SOUND__
const BANK = __BANK__;
const NOISE = Uint8Array.from(atob("__NOISE__"), (c) => c.charCodeAt(0)).buffer;
initSoundFrom(BANK, NOISE);
const kinds = Object.fromEntries(BANK.programs.map((p) => [p.id, p.kind]));
const lists = document.getElementById("lists");
const pressed = new Map();
for (const section of BANK.sections) {
  const h = document.createElement("h2");
  h.textContent = section.label;
  const grid = document.createElement("div");
  grid.className = "grid";
  for (const id of section.ids) {
    const b = document.createElement("button");
    b.textContent = BANK.labels[id];
    const kind = kinds[id];
    if (kind !== "cue") {
      b.setAttribute("aria-pressed", "false");
      const tag = document.createElement("small");
      tag.textContent = kind === "music" ? "music" : "loop";
      b.append(tag);
    }
    b.addEventListener("click", () => {
      if (kind === "cue") return play(id);
      const on = b.getAttribute("aria-pressed") !== "true";
      if (kind === "loop") {
        b.setAttribute("aria-pressed", String(on));
        return on ? startLoop(id) : stopLoop(id);
      }
      if (id === BANK.setup.urgent_track) {
        b.setAttribute("aria-pressed", String(on));
        return urgent(on);
      }
      document.querySelectorAll("button[data-music]").forEach((x) => x.setAttribute("aria-pressed", "false"));
      b.setAttribute("aria-pressed", String(on));
      music(on ? id : null);
    });
    if (kind === "music" && id !== BANK.setup.urgent_track) b.dataset.music = "1";
    grid.append(b);
  }
  lists.append(h, grid);
}
document.getElementById("vol").addEventListener("input", (e) => setVolume(e.target.value / 100));
document.getElementById("mus").addEventListener("input", (e) => setMusicVolume(e.target.value / 100));
</script>
</body>
</html>
"""


def main():
    bank = (ROOT / "data" / "sound-programs.json").read_text().strip()
    noise = base64.b64encode((ROOT / "data" / "sound-noise.wav").read_bytes()).decode()
    page = (PAGE.replace("__SYNTH__", module_body("web/synth.js"))
            .replace("__SOUND__", module_body("web/sound.js"))
            .replace("__BANK__", bank).replace("__NOISE__", noise))
    out = ROOT / "notes" / "sound" / "preview.html"
    out.write_text(page)
    print(f"{out.relative_to(ROOT)}: {len(page) // 1024} KB")


if __name__ == "__main__":
    main()
