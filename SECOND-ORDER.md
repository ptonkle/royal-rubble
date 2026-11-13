# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | | | | | | |
| R1.400 | finding | The page shell could be ported from Fleeced file by file. | app.rs, api.rs, view.rs, strings.rs, save.rs, patch_notes.rs, rng.rs, ticks.rs, online.rs, online/relays.rs and the page's app.js, online.js, net/, save/store.js came over nearly whole; lobby.rs, room.rs and app/lobby_view.rs were cut down to this game's seats and options. None of Fleeced's look, palette or game words came over. | The shell lives in `crates/core/src/{app,lobby,online}`; Fleeced's teams, roles, map size and HUD are gone. |  | done |
| R1.401 | divergence | The tab icon would match the chosen look. | No look is chosen yet. | `icon.rs` draws a plain tower in neutral gray and cream; recolor it when the style lands. |  | open |
| R1.402 | divergence | The menus would take the chosen style. | The person is still choosing between three mockups. | Every color, font and size sits in `web/theme.css` as custom properties; `web/style.css` holds structure only and reads those names. The chosen style replaces theme.css. | theme.css is the only file a style changes | open |
| R1.403 | divergence | Start would open a match. | The rules and physics are other agents' work. | Start runs the online kit's toy `World` through the inputs-only session (D23), so the lobby, the start message and the lockstep inputs are real end to end; the screen reads "Match placeholder". Swap `World` for the match once it implements `Rollback`. |  | open |
| R1.404 | finding | Every open seat could be empty, as in Fleeced. | BRIEF: bots fill empty seats. | An open seat always holds a person or a bot (Normal by default); the host sets Players 2 to 8 and each bot's difficulty. A joiner takes the first bot seat; a leaver's seat gets a bot back. | `lobby.rs` tests, `tests/breaks.json` shell rows | done |
| R1.405 | finding | The quick check and its test tags came with the template. | The template had the rules but not Fleeced's test tags. | Ported `scripts/tags.py`, the tagged `quick_check.py`, `check.sh`'s tags step, `land.sh`, `union_merge.py`, `bump.sh`, `shot.py` and Ape Race's invisible Xvfb Chromium (`tests/browser/engines.py`). Tags: boundary, foundation, online, page, saves, notes, copy. Agents in physics/game/terrain add their own tag rows. | `tests/tags.toml` | done |
| R1.406 | finding | Full volume would be Fleeced's. | The person's rule: full slider is 60% of Fleeced's level before its T23. | `save::master_gain` is the slider squared scaled to 600 thousandths at 100. A separate Music slider (coordinator's ask) gives `music_gain`, a multiplier under the main gain. Both reach the page as decimal strings in the view model and through `web/settings.js` (`gains()`, `onGains()`, `<html data-master-gain data-music-gain>`) for the sound module. | `save.rs` tests, break rows | done |
| R1.407 | worklist | The browser check would run from this repo's .venv. | ctc has no .venv yet, and installs were out of scope. | The check and the shots ran with Fleeced's `.venv/bin/python` via BUILDER_PYTHON. Run `scripts/setup.sh` once (network) to give ctc its own. |  | person |
| R1.408 | finding | The online status lines would need new words. | Fleeced's online.* and save.* lines are plain functional messages with no game flavor. | They were kept word for word; every other string is new. Listed below for the person to read. |  | done |
| R1.409 | divergence | Version 0.1.0 would carry "First playable build." | The game is not playable yet, and the core refuses a version with no line. | 0.1.0's one line is "Menus, settings and the online lobby."; reword it at the first playable deploy. |  | open |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
| title | Royal Rubble |
| menu.solo | Play vs bots |
| menu.host | Host online |
| menu.join | Join with code |
| menu.settings | Settings |
| menu.back | Back |
| settings.heading | Settings |
| settings.sound | Sound |
| settings.music | Music |
| notes.heading | What's New |
| online.heading | Join with code |
| lobby.heading | Lobby |
| lobby.solo_heading | Play vs bots |
| lobby.seats | Players |
| lobby.code | Room code |
| lobby.copy_code | Copy code |
| lobby.copy_link | Copy link |
| lobby.name | Name |
| lobby.name_hint | Your name |
| lobby.ready | Ready |
| lobby.not_ready | Not ready |
| lobby.start | Start |
| lobby.leave | Leave |
| lobby.need_ready | Waiting for every player to be ready. |
| lobby.match | Match |
| lobby.players | Players |
| lobby.wind | Wind |
| seat.default_name | Player {n} |
| seat.bot | Bot |
| seat.host | Host |
| seat.you | You |
| seat.is_ready | Ready |
| bot.easy | Easy |
| bot.normal | Normal |
| bot.hard | Hard |
| play.placeholder | Match placeholder |
| play.starting | Starting in {n} |
| play.leave | Leave |
| data/patch-notes.json 0.1.0 | Menus, settings and the online lobby. |
