"""Bakes the playfield's stitched textures (tools/look/bake.html, drawn with
tools/look/stitch.js) into web/look/stitch/ and writes their sizes and
anchors to data/stitch-sprites.json. Chromium runs on an invisible display
(tests/browser/engines.py). From the repo root:
  .venv/bin/python -I tools/look/bake_stitch.py
"""
import base64
import functools
import http.server
import json
import pathlib
import sys
import threading

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests" / "browser"))
import engines  # noqa: E402
from playwright.sync_api import sync_playwright  # noqa: E402


def main():
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(ROOT))
    handler.log_message = lambda *a: None
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    with sync_playwright() as p:
        b = engines.launch(p, "chromium")
        pg = b.new_page()
        pg.on("pageerror", lambda e: print("pageerror:", e, file=sys.stderr))
        pg.goto(f"http://127.0.0.1:{srv.server_port}/tools/look/bake.html")
        pg.wait_for_function("window.DONE === true", timeout=180000)
        result = pg.evaluate("window.RESULT")
        b.close()
    srv.shutdown()
    out = ROOT / "web" / "look" / "stitch"
    out.mkdir(parents=True, exist_ok=True)
    for old in out.iterdir():
        old.unlink()
    for name, url in result["files"].items():
        (out / name).write_bytes(base64.b64decode(url.split(",", 1)[1]))
    sprites = {"_about": "Written by tools/look/bake_stitch.py: each baked texture's file, size in pixels, anchor (ax, ay) and the size it was baked at (tall or r), so web/field-draw.js can scale it to what the core gives."}
    sprites.update(result["sprites"])
    (ROOT / "data" / "stitch-sprites.json").write_text(json.dumps(sprites, indent=1) + "\n")
    print(len(result["files"]), "files")


if __name__ == "__main__":
    main()
