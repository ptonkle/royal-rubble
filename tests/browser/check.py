#!/usr/bin/env python3
"""Browser check: the built page in Chromium, Firefox and WebKit.

For each engine it serves dist/ on 127.0.0.1, opens index.html, waits for the
page to set <body data-ready="true">, lets it idle, and then runs the project's
own interactions from tests/browser/interactions.py. The run fails on:

- an uncaught page error;
- a console error;
- any request to another origin, during load, idle or the interactions;
- a Content-Security-Policy violation (a blocked attempt is still an attempt);
- a WebSocket, a popup, a worker or a service worker opened by the page;
- a failed check inside the interactions.

What a pass establishes is what happened on that path, in those engines, on
that run. It does not show that no feature can ever reach the network: a
feature the interactions never start is a feature this check never saw.

    python3 tests/browser/check.py

Environment:
    BUILDER_ENGINES   space-separated engines (default "chromium firefox webkit");
                      add "webkit-iphone" for WebKit as an iPhone 15 with touch.
                      CI may run "chromium" alone (D7); run all three locally
                      before a push.
    BUILDER_IDLE      seconds to idle after load (default 10)
    BUILDER_ORIGIN    check a deployed origin instead of serving dist/, e.g.
                      https://example.github.io/my-tool  (no trailing slash)
"""
import functools
import http.server
import importlib.util
import os
import pathlib
import sys
import time
import threading

from playwright.sync_api import sync_playwright

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import engines  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
ENGINES = os.environ.get("BUILDER_ENGINES", "chromium firefox webkit").split()
IDLE_MS = int(float(os.environ.get("BUILDER_IDLE", "10")) * 1000)

failures = []
# How long after the check navigates a cancelled fetch counts as its doing.
LEAVE_GRACE_S = 3


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def log_message(self, *args):
        pass


def serve():
    handler = functools.partial(Handler, directory=str(DIST))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def load_interactions():
    path = pathlib.Path(__file__).with_name("interactions.py")
    spec = importlib.util.spec_from_file_location("interactions", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_engine(p, engine, base, interactions):
    print(f"\n[{engine}] {base}/")
    scheme_host = base.split("/", 3)
    origin = "/".join(scheme_host[:3])
    seen = {"foreign": [], "console": [], "errors": [], "sockets": [], "popups": [], "workers": []}

    def same_origin(url):
        return url.startswith(origin + "/") or url.startswith("blob:" + origin) or url.startswith("data:")

    # "webkit-iphone": WebKit with Playwright's iPhone 15 profile (touch, a
    # phone viewport). Emulation shows layout and touch events; it cannot show
    # real multitouch, fullscreen or install, which stay a person's check.
    launcher, _, device = engine.partition("-")
    profile = dict(p.devices["iPhone 15"]) if device == "iphone" else {}
    browser = engines.launch(p, launcher)
    context = browser.new_context(accept_downloads=True, **profile)
    context.on("request", lambda r: None if same_origin(r.url) else seen["foreign"].append(r.url))

    def route(r):
        if same_origin(r.request.url):
            r.continue_()
        else:
            r.abort()

    context.route("**/*", route)
    # A request the page's CSP blocks never becomes a request event, so the
    # violation itself is recorded and reported.
    context.add_init_script(
        "window.__builderCsp = [];"
        "document.addEventListener('securitypolicyviolation',"
        " e => window.__builderCsp.push(e.violatedDirective + ' ' + e.blockedURI));"
    )
    page = context.new_page()
    page.on("console", lambda m: seen["console"].append(m.text) if m.type == "error" else None)
    # Leaving a page cancels its fetches still in flight, and Firefox throws
    # each as an uncaught "NetworkError" on the page being left (seen on the
    # live site, where loads are slower: R1.3014). Such an error within a
    # moment of the check navigating is noted, not failed; any other stays.
    left = {"at": -1e9}

    def leaving(go):
        def wrapped(*args, **kwargs):
            left["at"] = time.monotonic()
            return go(*args, **kwargs)
        return wrapped

    page.goto = leaving(page.goto)
    page.reload = leaving(page.reload)

    def page_error(e):
        text = str(e)
        cancelled = "NetworkError when attempting to fetch resource" in text
        if cancelled and time.monotonic() - left["at"] < LEAVE_GRACE_S:
            print(f"  note: a fetch cancelled by leaving the page ({text})")
            return
        seen["errors"].append(text)

    page.on("pageerror", page_error)
    page.on("websocket", lambda ws: seen["sockets"].append(ws.url))
    page.on("worker", lambda w: seen["workers"].append(w.url))
    page.on("popup", lambda pg: seen["popups"].append(pg.url))

    # ?fast skips what only makes a person wait (crates/core/src/app.rs).
    page.goto(base + "/index.html?fast")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    check(True, "page reported ready")
    page.wait_for_timeout(IDLE_MS)
    check(not seen["foreign"], f"no off-origin request during load and {IDLE_MS // 1000}s idle {seen['foreign']}")

    interactions.run(page, check, engine)

    csp = page.evaluate("() => window.__builderCsp || []")
    check(not csp, f"no Content-Security-Policy violation {csp}")
    sw = page.evaluate("() => navigator.serviceWorker ? navigator.serviceWorker.getRegistrations().then(r => r.length) : 0")
    check(not seen["errors"], f"no uncaught page error {seen['errors']}")
    check(not seen["console"], f"no console error {seen['console']}")
    check(not seen["foreign"], f"no off-origin request at any point {seen['foreign']}")
    check(not seen["sockets"], f"no WebSocket {seen['sockets']}")
    check(not seen["popups"], f"no popup {seen['popups']}")
    check(not seen["workers"] and sw == 0, f"no worker or service worker {seen['workers']} sw={sw}")
    browser.close()


def main():
    origin = os.environ.get("BUILDER_ORIGIN", "").rstrip("/")
    server = None
    if not origin:
        if not (DIST / "index.html").exists():
            sys.exit("dist/ is missing: run scripts/build.sh first")
        server = serve()
        host, port = server.server_address[:2]
        origin = f"http://{host}:{port}"
    interactions = load_interactions()
    with sync_playwright() as p:
        for engine in ENGINES:
            run_engine(p, engine, origin, interactions)
    if server:
        server.shutdown()
    if failures:
        print(f"\nBROWSER CHECK FAILED: {len(failures)} problem(s)")
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print(f"\nBROWSER CHECK PASSED in {', '.join(ENGINES)}")


if __name__ == "__main__":
    main()
