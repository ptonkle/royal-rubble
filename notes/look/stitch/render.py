"""Render a page under notes/look/stitch to a PNG, or print what it returns.

Serves ~/slop over local http (so a page can read the journal's reference
photos into a canvas untainted) and opens it in Chromium on an invisible
display (tests/browser/engines.py). From the repo root:
  .venv/bin/python -I notes/look/stitch/render.py PAGE OUT.png [W H]
  .venv/bin/python -I notes/look/stitch/render.py PAGE --json   # prints window.RESULT
The page sets window.DONE = true when it has finished drawing. Query strings
after PAGE (page.html?zoom=2) are passed through.
"""
import functools
import http.server
import os
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(REPO, "tests", "browser"))
import engines  # noqa: E402
from playwright.sync_api import sync_playwright  # noqa: E402

SLOP = os.path.expanduser("~/slop")


def main():
    arg = sys.argv[1]
    page, _, query = arg.partition("?")
    page_path = os.path.realpath(page)
    out = sys.argv[2]
    w, h = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (1920, 1080)
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=SLOP)
    handler.log_message = lambda *a: None
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{srv.server_port}/" + os.path.relpath(page_path, SLOP) + (("?" + query) if query else "")
    with sync_playwright() as p:
        b = engines.launch(p, "chromium")
        pg = b.new_page(viewport={"width": w, "height": h})
        pg.on("console", lambda m: print("console:", m.text, file=sys.stderr))
        pg.on("pageerror", lambda e: print("pageerror:", e, file=sys.stderr))
        pg.goto(url)
        pg.wait_for_function("window.DONE === true", timeout=180000)
        if out == "--json":
            print(pg.evaluate("JSON.stringify(window.RESULT, null, 1)"))
        else:
            pg.screenshot(path=out)
        b.close()
    srv.shutdown()


if __name__ == "__main__":
    main()
