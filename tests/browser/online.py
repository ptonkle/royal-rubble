#!/usr/bin/env python3
"""Online check: two browsers meet by pasted codes and play the real match.

Ported from Fleeced's tests/browser/online.py (the online kit's check). It
sits beside the browser check, not inside it, because check.py fails on any
WebSocket or off-origin request and must keep doing so.

How it stays on the origin: `?ice=none` drops the STUN servers so the two
browsers connect on host candidates alone, and the pasted-code path carries
the whole handshake in the codes, so no relay is contacted.

It fails on: a page or console error, any request off the origin, any
WebSocket, a browser that never reaches the match, the two browsers
disagreeing (`body[data-sync]` is `round : tick : checksum`, from the core),
or a browser reporting it is out of sync.

    python3 tests/browser/online.py [chromium firefox ...]   (default: chromium)
"""
import functools
import http.server
import json
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

import engines

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())

SELECTORS = {
    "online": "[id='menu.join']",
    "host": "[id='online.paste_host']",
    "offer_out": "[id='online.offer_out']",
    "join": "[id='online.paste_join']",
    "offer_in": "[id='online.offer_in']",
    "make_answer": "[id='online.make_answer']",
    "answer_out": "[id='online.answer_out']",
    "answer_in": "[id='online.answer_in']",
    "accept": "[id='online.accept_answer']",
    "status": "#message",
    "ready": "[id='lobby.ready']",
    "start": "[id='lobby.start']:not([disabled])",
    "match_ready": "[id='match.ready']",
}
LOBBY = "body[data-screen='lobby']"
MATCH = "body[data-screen='match']"
HOST_SEES_JOINER = "[id='seat.1']:not([data-look='empty'])"
TIMEOUT_MS = 15000

failures = []


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *args):
        pass


def serve():
    handler = functools.partial(Quiet, directory=str(DIST))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


LAUNCH = {
    "chromium": {"args": ["--disable-features=WebRtcHideLocalIpsWithMdns"]},
    "firefox": {"firefox_user_prefs": {"media.peerconnection.ice.obfuscate_host_addresses": False}},
}


def open_page(browser, origin, who, seen):
    context = browser.new_context(viewport={"width": 1280, "height": 720})
    page = context.new_page()
    page.on("console", lambda m: seen["errors"].append(f"{who}: {m.text}") if m.type == "error" else None)
    page.on("pageerror", lambda e: seen["errors"].append(f"{who}: {e}"))
    page.on("websocket", lambda ws: seen["sockets"].append(f"{who}: {ws.url}"))
    page.on(
        "request",
        lambda r: None if r.url.startswith((origin + "/", "blob:", "data:")) else seen["offsite"].append(r.url),
    )
    page.goto(origin + "/index.html?ice=none&fast&codes")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=TIMEOUT_MS)
    return page


def sync(page):
    return page.get_attribute("body", "data-sync") or ""


def run(p, engine, origin):
    print(f"\n[{engine}] online, two browsers, pasted codes, the real match")
    browser = engines.launch(p, engine, **LAUNCH.get(engine, {}))
    seen = {"errors": [], "sockets": [], "offsite": []}
    host = open_page(browser, origin, "host", seen)
    joiner = open_page(browser, origin, "joiner", seen)
    s = SELECTORS

    host.click(s["online"])
    host.click(s["host"])
    host.wait_for_selector(s["offer_out"], timeout=TIMEOUT_MS)
    offer = host.input_value(s["offer_out"])
    check(bool(offer), "the host shows a code to send")

    joiner.click(s["online"])
    joiner.click(s["join"])
    joiner.fill(s["offer_in"], offer)
    joiner.click(s["make_answer"])
    joiner.wait_for_selector(s["answer_out"], timeout=TIMEOUT_MS)
    answer = joiner.input_value(s["answer_out"])
    check(bool(answer), "the joiner shows a reply code")

    host.fill(s["answer_in"], answer)
    host.click(s["accept"])
    joiner.wait_for_selector(LOBBY, state="attached", timeout=TIMEOUT_MS)
    host.wait_for_selector(HOST_SEES_JOINER, state="attached", timeout=TIMEOUT_MS)
    check(True, "the host seats the joiner")

    joiner.click(s["ready"])
    host.click(s["start"], timeout=TIMEOUT_MS)
    for who, page in (("host", host), ("joiner", joiner)):
        page.wait_for_selector(MATCH, state="attached", timeout=TIMEOUT_MS)
        check(True, f"the {who} reaches the match")

    # Both press Ready: the build ends on both browsers, so each one's input
    # reached the other through the host.
    for page in (host, joiner):
        page.click(s["match_ready"], timeout=TIMEOUT_MS)
    states = ["", ""]
    for _ in range(600):
        states = [sync(host), sync(joiner)]
        parts = [x.split(" : ") for x in states]
        if all(len(x) == 3 for x in parts) and parts[0][1] == parts[1][1] and parts[0][1] != "0":
            break
        host.wait_for_timeout(20)
    check(states[0] == states[1] and states[0] != "", f"both browsers show the same match {states}")
    statuses = [host.inner_text(s["status"]), joiner.inner_text(s["status"])]
    check(STRINGS["online.desync"] not in statuses, f"no browser is out of sync {statuses}")
    check(STRINGS["online.garbled"] not in statuses, f"no browser reports a garbled message {statuses}")
    check(not seen["errors"], f"no page or console error {seen['errors'][:3]}")
    check(not seen["offsite"], f"no request off the origin {seen['offsite'][:3]}")
    check(not seen["sockets"], f"no WebSocket, so no relay was contacted {seen['sockets'][:3]}")
    browser.close()


def main():
    if not (DIST / "index.html").exists():
        sys.exit("dist/ is missing: run scripts/build.sh first")
    names = sys.argv[1:] or ["chromium"]
    server = serve()
    origin = "http://127.0.0.1:%d" % server.server_address[1]
    with sync_playwright() as p:
        for engine in names:
            run(p, engine, origin)
    server.shutdown()
    if failures:
        print(f"\nONLINE CHECK FAILED: {len(failures)} problem(s)")
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print(f"\nONLINE CHECK PASSED in {', '.join(names)}")


if __name__ == "__main__":
    main()
