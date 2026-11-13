#!/usr/bin/env python3
"""Screenshots of the built game at 1920x1080: the menu, settings, What's New
and a hosted lobby.

    BUILDER_PYTHON=... $BUILDER_PYTHON scripts/shot.py [OUT_DIR]   (default notes/shots)

Chromium runs through tests/browser/engines.py: on the GPU in an invisible Xvfb
window, never a window on the person's screen. It opens the page with
`?fast&net=off`, so rooms open with no transport and nothing leaves the origin.
Any page or console error, and any off-origin request or socket, fails the run.
Ported from Fleeced's.
"""
import functools
import http.server
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "browser"))
import engines  # noqa: E402

SIZE = {"width": 1920, "height": 1080}
TIMEOUT_MS = 15000


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *args):
        pass


def by_id(name):
    return f"[id='{name}']"


def main():
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "notes" / "shots"
    out.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(Handler, directory=str(ROOT / "dist"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f"http://127.0.0.1:{server.server_address[1]}"
    url = f"{origin}/index.html?fast&net=off"
    problems = []

    with sync_playwright() as p:
        browser = engines.launch(p, "chromium")
        page = browser.new_page(viewport=SIZE)
        page.set_default_timeout(TIMEOUT_MS)
        page.on("pageerror", lambda e: problems.append(f"page error: {e}"))
        page.on("console", lambda m: problems.append(f"console: {m.text}") if m.type == "error" else None)
        page.on("websocket", lambda ws: problems.append(f"socket: {ws.url}"))
        page.on(
            "request",
            lambda r: None if r.url.startswith((origin + "/", "data:", "blob:")) else problems.append(f"off-origin: {r.url}"),
        )

        def shot(name):
            page.wait_for_timeout(150)
            page.screenshot(path=str(out / f"{name}.png"))
            print("shot", name, "on", page.evaluate("document.body.dataset.screen"))

        page.goto(url)
        page.wait_for_selector("body[data-ready='true']", state="attached")
        shot("shell-menu")
        page.click(by_id("menu.settings"))
        page.wait_for_selector("body[data-screen='settings']", state="attached")
        shot("shell-settings")
        page.click(by_id("menu.back"))
        page.click("#version")
        page.wait_for_selector("body[data-screen='notes']", state="attached")
        shot("shell-notes")
        page.click(by_id("menu.back"))
        page.wait_for_selector("body[data-screen='title']", state="attached")
        page.click(by_id("menu.host"))
        page.wait_for_selector("body[data-screen='lobby']", state="attached")
        page.fill(by_id("lobby.players"), "5")
        page.locator(by_id("lobby.players")).blur()
        page.wait_for_selector(by_id("seat.4"))
        page.select_option(by_id("seat.2.level"), "hard")
        page.select_option(by_id("seat.3.level"), "easy")
        page.fill(by_id("lobby.name"), "Guinevere")
        page.locator(by_id("lobby.name")).blur()
        shot("shell-lobby")
        browser.close()
    server.shutdown()
    if problems:
        print("SHOT RUN FAILED:", *problems, sep="\n  ")
        sys.exit(1)


if __name__ == "__main__":
    main()
