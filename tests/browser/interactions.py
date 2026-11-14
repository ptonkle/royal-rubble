"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. Drive the tool here the way a person would, and assert what the page
shows with `check(condition, "what this shows")`. Assert the effect a person
would see (the text drawn, the file saved), not the markup that should cause it.

Expected values in these checks are worked out by hand from the brief (C4),
never copied from the tool's own output. Wait on a selector or a data-*
attribute the page sets, never on a fixed time. Ported from Fleeced's.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())
NOTES = json.loads((ROOT / "data" / "patch-notes.json").read_text())


def on_screen(page, name):
    page.wait_for_selector(f"body[data-screen='{name}']", state="attached", timeout=10000)


def run(page, check, engine):
    title(page, check)
    settings(page, check)
    whats_new(page, check)
    solo_lobby(page, check)
    online_lobby(page, check)
    invite_link(page, check)


def by_id(name):
    return f"[id='{name}']"


def offline(page, part=""):
    """The page again with no transport: check.py fails on any WebSocket."""
    url = page.url.split("?")[0].split("#")[0] + "?fast&net=off" + part
    page.goto("about:blank")
    page.goto(url)
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)


def title(page, check):
    on_screen(page, "title")
    check(page.text_content("h1") == STRINGS["title"], "the Title shows the game's name")
    check(page.title() == STRINGS["title"], "the tab shows the game's name")
    for key in ("menu.solo", "menu.host", "menu.join", "menu.settings"):
        check(page.locator(by_id(key)).text_content() == STRINGS[key], f"the Title offers {STRINGS[key]}")
    check(page.inner_text("#message") == "", "no error message is shown")
    icon = page.get_attribute("#tab-icon", "href")
    check(icon.startswith("data:image/svg+xml,"), "the tab icon is the core's SVG data URL")


def settings(page, check):
    page.click(by_id("menu.settings"))
    on_screen(page, "settings")
    # the sliders' values are the core's: move them, and read back what is drawn
    page.locator(by_id("settings.volume")).fill("100")
    page.wait_for_selector("[id='settings.volume.text']:has-text('100%')", timeout=5000)
    page.locator(by_id("settings.music")).fill("30")
    page.wait_for_selector("[id='settings.music.text']:has-text('30%')", timeout=5000)
    # by hand: a full slider is 60% of the old full gain; music 30% is 0.3 squared
    gains = page.evaluate("() => [document.documentElement.dataset.masterGain, document.documentElement.dataset.musicGain]")
    check(gains == ["0.600", "0.090"], f"the sound module can read the gains {gains}")
    page.reload()
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.click(by_id("menu.settings"))
    on_screen(page, "settings")
    check(page.input_value(by_id("settings.volume")) == "100", "the sound slider survives a reload")
    check(page.input_value(by_id("settings.music")) == "30", "the music slider survives a reload")
    page.click(by_id("menu.back"))
    on_screen(page, "title")


def whats_new(page, check):
    version = NOTES[0]["version"]
    check(page.inner_text("#version") == version, f"the version button says {version}")
    page.click("#version")
    on_screen(page, "notes")
    check(page.inner_text(".note-version") == version, "What's New opens on the newest version")
    check(NOTES[0]["notes"][0] in page.inner_text(".notes"), "What's New shows the version's lines")
    page.click(by_id("menu.back"))
    on_screen(page, "title")


def solo_lobby(page, check):
    page.click(by_id("menu.solo"))
    on_screen(page, "lobby")
    page.fill(by_id("lobby.players"), "4")
    page.locator(by_id("lobby.players")).blur()
    page.wait_for_selector(by_id("seat.3"), timeout=5000)
    check(page.locator(".seat").count() == 4, "four players show four seats")
    page.select_option(by_id("seat.3.level"), "hard")
    check(page.input_value(by_id("seat.3.level")) == "hard", "a bot's difficulty can be set")
    page.click(by_id("lobby.start"))
    on_screen(page, "match")
    check(page.locator(by_id("match.tool")).count() == 1, "a match opens on the build phase's tools")
    page.locator(by_id("play.leave")).dispatch_event("click")
    on_screen(page, "title")
    fast_match(page, check)


def press(page, name):
    """A click by event: the match's HUD redraws every frame, so it is never still."""
    loc = page.locator(by_id(name))
    if loc.count() and loc.is_enabled():
        loc.dispatch_event("click")


def fast_match(page, check):
    """With ?fast, a match vs one bot where you only press Ready (and take
    the first card) runs to the results: the bot knocks you out."""
    page.click(by_id("menu.solo"))
    on_screen(page, "lobby")
    page.click(by_id("lobby.start"))
    on_screen(page, "match")
    for _ in range(8000):
        if page.locator(by_id("match.results")).count():
            break
        press(page, "match.skip")
        press(page, "match.card.0")
        press(page, "match.ready")
        page.wait_for_timeout(50)
    lines = page.inner_text(by_id("match.results")) if page.locator(by_id("match.results")).count() else ""
    you = STRINGS["seat.you"]
    # who wins an idle match turns on the bots' aim and castles; the check is
    # that it ends with you in the standings
    check(
        lines.startswith("1. ") and f". {you}" in lines,
        f"a fast match reaches the results with you placed: {lines!r}",
    )
    press(page, "play.leave")
    on_screen(page, "title")


def online_lobby(page, check):
    offline(page)
    page.click(by_id("menu.join"))
    on_screen(page, "online")
    page.fill(by_id("online.code"), "nope")
    page.locator(by_id("online.code")).blur()
    page.click(by_id("online.join"))
    page.wait_for_selector(f"#message:has-text('{STRINGS['online.bad_code']}')", timeout=5000)
    page.click(by_id("menu.back"))
    on_screen(page, "title")
    page.click(by_id("menu.host"))
    on_screen(page, "lobby")
    check("#room=" in page.url, "the address carries the room code")
    code = page.url.split("#room=")[1]
    check(code in page.inner_text(by_id("lobby.copy_code.code")), "the lobby shows the room code")
    page.click(by_id("lobby.copy_link"))
    copied = page.evaluate("() => document.body.dataset.copied")
    check(copied.endswith("#room=" + code), f"Copy link copies the invite link {copied}")
    check(page.is_checked(by_id("lobby.wind")), "wind starts on")
    page.click(by_id("lobby.leave"))
    on_screen(page, "title")
    check("#room=" not in page.url, "leaving clears the room from the address")


def invite_link(page, check):
    offline(page, "#room=ABCDEF")
    on_screen(page, "lobby")
    check(page.inner_text("#message") == STRINGS["online.finding"], "an invite link goes straight to the room")
