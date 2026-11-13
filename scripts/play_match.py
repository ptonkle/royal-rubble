#!/usr/bin/env python3
"""Plays a whole ?fast match vs one bot in the built page, with scripted
inputs, until someone is knocked out; screenshots build, siege, the upgrade
cards and the results at 1920x1080.

    $BUILDER_PYTHON scripts/play_match.py [OUT_DIR]   (default notes/shots)

Chromium runs through tests/browser/engines.py: on the GPU in an invisible
Xvfb window, never a window on the person's screen. Any page or console
error fails the run.
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
from shot import Handler, by_id  # noqa: E402

SIZE = {"width": 1920, "height": 1080}
MATCH_LIMIT_S = 240


def press(page, name):
    """A click by event: the HUD redraws every frame, so it is never still."""
    loc = page.locator(by_id(name))
    if loc.count() and loc.is_enabled():
        loc.dispatch_event("click")


def build_some(page):
    """A post from the ground at the zone's left edge, a beam off its top,
    and a royal on the ground when one waits."""
    zone = page.evaluate("document.getElementById('field').dataset.zone")
    if not zone:
        return
    left, top, right, bottom = (int(v) for v in zone.split(","))
    cell = (right - left) // 40
    x0 = left + cell * 4
    press(page, "match.tool.wood")
    # two clicks a segment, the next starting where the last ended (R1.3300)
    page.mouse.click(left, bottom)
    page.mouse.click(left, bottom - cell * 2)
    page.mouse.click(left + cell * 2, bottom - cell * 2)
    page.keyboard.press("Escape")
    if page.locator(by_id("match.tool.royal")).count():
        press(page, "match.tool.royal")
        page.mouse.click(x0 + cell * 6, bottom - 2)


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
            page.screenshot(path=str(out / f"match-{name}.png"))
            print("shot", name)

        page.goto(f"{origin}/index.html?fast&net=off")
        page.wait_for_selector("body[data-ready='true']", state="attached")
        page.click(by_id("menu.solo"))
        page.wait_for_selector("body[data-screen='lobby']", state="attached")
        page.click(by_id("lobby.start"))
        page.wait_for_selector("body[data-screen='match']", state="attached")
        end = time.time() + MATCH_LIMIT_S
        built_rounds = set()
        while time.time() < end:
            if page.locator(by_id("match.results")).count():
                page.wait_for_timeout(200)
                shot("results")
                break
            if page.locator(by_id("match.skip")).count():
                shot("replay")
                press(page, "match.skip")
                continue
            if page.locator(".card").count():
                page.wait_for_timeout(100)
                shot("upgrade")
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
                    continue
                page.keyboard.press("Space")
                page.wait_for_timeout(85)
                page.keyboard.press("Space")
                page.wait_for_timeout(250)
                shot("siege")
                page.wait_for_timeout(400)
                continue
            if page.locator(by_id("match.tool")).count():
                rnd = page.locator(by_id("match.phase")).inner_text().split("\n")[0]
                if rnd not in built_rounds:
                    built_rounds.add(rnd)
                    build_some(page)
                    page.mouse.move(SIZE["width"] // 2, SIZE["height"] // 2)
                    if len(built_rounds) == 1:
                        press(page, "match.test")
                        page.wait_for_timeout(200)
                        shot("build")
                        press(page, "match.test")
                    if len(built_rounds) == 2:
                        press(page, "match.scout")
                        page.wait_for_timeout(200)
                        shot("scout")
                        press(page, "match.scout")
                press(page, "match.ready")
                continue
            page.wait_for_timeout(100)
        else:
            problems.append("no results screen within the time limit")
        browser.close()
    server.shutdown()
    if problems:
        print("MATCH RUN FAILED:", *problems, sep="\n  ")
        sys.exit(1)
    print("match reached the results")


if __name__ == "__main__":
    main()
