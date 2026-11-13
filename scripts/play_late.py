#!/usr/bin/env python3
"""Plays a ?fast match with 8 seats (the human and 7 bots) in the built
page and screenshots rounds 5-8 at 1920x1080: the build view, each siege
shot in flight and after impact (the corner inset shows then), the health
list and the results.

    $BUILDER_PYTHON scripts/play_late.py [OUT_DIR]   (default notes/shots)

The human builds a small castle each round: two posts, a beam across them
and a royal between. Chromium runs through tests/browser/engines.py, on the
GPU in an invisible Xvfb window. Any page or console error fails the run.
"""
import functools
import http.server
import pathlib
import sys
import threading
import time

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "browser"))
sys.path.insert(0, str(ROOT / "scripts"))
import engines  # noqa: E402
from play_match import press  # noqa: E402
from shot import Handler, by_id  # noqa: E402

SIZE = {"width": 1920, "height": 1080}
MATCH_LIMIT_S = 600
ROUNDS = range(5, 9)
# after release (?fast runs 8x, so a 2 s flight takes 250 ms): in flight,
# then just after impact and while debris settles
SIEGE_AT_MS = [40, 90, 150, 300]


def drag(page, a, b):
    page.mouse.move(*a)
    page.mouse.down()
    page.mouse.move(*b, steps=4)
    page.mouse.up()


def build_castle(page, rnd):
    """Two posts, a beam across their tops and a royal between them,
    shifted each round so the castle grows."""
    zone = page.evaluate("document.getElementById('field').dataset.zone")
    if not zone:
        return
    left, top, right, bottom = (int(v) for v in zone.split(","))
    cell = (right - left) // 40
    x0 = left + cell * (6 + (rnd % 4) * 6)
    press(page, "match.tool.wood")
    drag(page, (x0, bottom), (x0, bottom - cell * 3))
    drag(page, (x0 + cell * 4, bottom), (x0 + cell * 4, bottom - cell * 3))
    drag(page, (x0, bottom - cell * 3), (x0 + cell * 4, bottom - cell * 3))
    if page.locator(by_id("match.tool.royal")).count():
        press(page, "match.tool.royal")
        page.mouse.click(x0 + cell * 2, bottom - 2)


def round_no(page):
    loc = page.locator(by_id("match.phase"))
    if not loc.count():
        return 0
    digits = "".join(c if c.isdigit() else " " for c in loc.inner_text().split("\n")[0]).split()
    return int(digits[0]) if digits else 0


def main():
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "notes" / "shots"
    out.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(Handler, directory=str(ROOT / "dist"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f"http://127.0.0.1:{server.server_address[1]}"
    problems = []
    taken = set()

    with sync_playwright() as p:
        browser = engines.launch(p, "chromium")
        page = browser.new_page(viewport=SIZE)
        page.set_default_timeout(15000)
        page.on("pageerror", lambda e: problems.append(f"page error: {e}"))
        page.on("console", lambda m: problems.append(f"console: {m.text}") if m.type == "error" else None)

        def shot(name):
            if name in taken:
                return
            taken.add(name)
            page.screenshot(path=str(out / f"late-{name}.png"))
            print("shot", name, flush=True)

        page.goto(f"{origin}/index.html?fast&net=off")
        page.wait_for_selector("body[data-ready='true']", state="attached")
        page.click(by_id("menu.solo"))
        page.wait_for_selector("body[data-screen='lobby']", state="attached")
        page.evaluate(
            """() => { const s = document.getElementById('lobby.players');
            s.value = s.max; s.dispatchEvent(new Event('change', {bubbles: true})); }"""
        )
        page.wait_for_timeout(200)
        shot("lobby")
        page.click(by_id("lobby.start"))
        page.wait_for_selector("body[data-screen='match']", state="attached")
        end = time.time() + MATCH_LIMIT_S
        built = set()
        shots_in = {}
        while time.time() < end:
            rnd = round_no(page) or max(built, default=0)
            if page.locator(by_id("match.results")).count():
                page.wait_for_timeout(300)
                shot("results")
                break
            if page.locator(by_id("match.skip")).count():
                if rnd in ROUNDS:
                    shot(f"r{rnd}-replay")
                press(page, "match.skip")
                continue
            if page.locator(".card").count():
                page.wait_for_timeout(100)
                if rnd in ROUNDS:
                    shot(f"r{rnd}-upgrade")
                if page.locator(by_id("match.card.0")).is_enabled():
                    press(page, "match.card.0")
                elif page.locator(by_id("match.ready")).is_enabled():
                    press(page, "match.ready")
                page.wait_for_timeout(100)
                continue
            if page.locator(by_id("match.ammo")).count():
                left = page.locator(by_id("match.ammo.0")).inner_text()
                if left.strip().endswith(" 0") and not page.locator(by_id("match.ammo.1")).count():
                    if page.locator(by_id("match.ready")).is_enabled():
                        press(page, "match.ready")
                    page.wait_for_timeout(100)
                    continue
                n = shots_in.get(rnd, 0)
                shots_in[rnd] = n + 1
                if rnd in ROUNDS and n == 0:
                    shot(f"r{rnd}-siege-aim")
                page.keyboard.press("Space")
                page.wait_for_timeout(85 + 20 * (n % 3))
                page.keyboard.press("Space")
                waited = 0
                for at in SIEGE_AT_MS:
                    page.wait_for_timeout(at - waited)
                    waited = at
                    if rnd in ROUNDS and n < 2:
                        shot(f"r{rnd}-siege{n}-{at}ms")
                continue
            if page.locator(by_id("match.tool")).count():
                if rnd not in built:
                    built.add(rnd)
                    build_castle(page, rnd)
                    page.mouse.move(SIZE["width"] // 2, SIZE["height"] // 2)
                    if rnd in ROUNDS:
                        page.wait_for_timeout(200)
                        shot(f"r{rnd}-build")
                press(page, "match.ready")
                page.wait_for_timeout(100)
                continue
            page.wait_for_timeout(100)
        else:
            problems.append("no results screen within the time limit")
        browser.close()
    server.shutdown()
    if problems:
        print("LATE RUN FAILED:", *problems[:20], sep="\n  ")
        sys.exit(1)
    print("late match reached the results")


if __name__ == "__main__":
    main()
