# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | | | | | | |
| R1.300 | divergence | The brief gives no units for the map | Terrain columns are one grid cell wide; heights and ruin nodes use fixed-point units, 1024 to a cell (`terrain::CELL`, `COL_W`) | The physics may rescale; it maps cells to its own grid | `crates/core/src/terrain/mod.rs` | open |
| R1.301 | finding | Plot growth could be centered or anchored | Anchored at the front edge by the gap: level 1 is 40 columns, each level adds 7, level 9 is 96 (the whole generated plot). Build height 12 cells at level 1, +2 a level | Spots carry the lowest level that opens them; the first two spots after the watchtower always open at level 1 | `terrain::tune`, test `terrain_buildable_span_grows_with_level` | done |
| R1.302 | finding | The gap could be one kind | Three kinds by seed: moat (narrow, steep banks), river (wider, gentle), valley (wide, deep, a thin stream). Its floor sits below the lowest plot ground, so no plot column is ever under water | `Plot::in_water` says when a shot is spent | test `terrain_hundred_seeds_all_playable` | done |
| R1.303 | finding | Pointed hills left no room for the watchtower; seeds with a hill often had none | Hills now have a flat top 5 columns wide, and every map with a hill (2 in 3) has a watchtower on it | Seen in `notes/terrain/seeds.png` | `tune::HILL_TOP_HALF_W` | done |
| R1.304 | finding | Ruins need the physics' segment types | Ruins are one of four stone shapes (broken wall, arch, tower stub, rubble stubs) as node pairs with a `RuinMaterial` of their own; feet sit on the ground under them, segments at most about 5 cells long | The physics agent maps `RuinMaterial::Stone` and checks the lengths against its stone maximum | `terrain/spots.rs` `RUINS` | open |
| R1.305 | finding | The debug picture needed an SVG or PNG writer; no converter is installed | An ignored integration test draws 12 seeds from core output into `notes/terrain/seeds.png` with its own small PNG encoder (it lives in `crates/core/tests/`, outside terrain/, because src may not touch files) | Run `cargo test -p tool_core --test terrain_pictures -- --ignored` | `crates/core/tests/terrain_pictures.rs` | done |
| R1.306 | finding | Narrow tags for the terrain tests | `tests/tags.toml` merges every matching rule, and `crates/**` already pulls in browser steps, so a narrower terrain rule would not shorten the quick check | No tags change; the terrain tests ride on the core rule | | open |
| R1.307 | worklist | No terrain run in the browser yet | Build, wasm and browser steps were not run (agent brief: no browser checks) | The coordinator runs them at merge | | open |
| R1.200 | finding | Match rules would need physics internals | They need only a `SiegeResult`: per castle, owner, attacker, ghost flag, royals killed, producer ids still standing | `crates/core/src/game/` (flow, pairing, economy, ledger, royals, upgrades, bots, dice, tune); every balance number in `game/tune.rs` | Balance numbers live in `game/tune.rs` only | done |
| R1.201 | finding | Royal damage curve to end 8-player games in 15–18 rounds | Measured with 8 rule-only bots, 12 seeds, attackers killing a random share of the defender's royals: average kill share 27% ends in round 15–17 (first out 13–15); 37% ends 14–17; 52% ends 13–15. The King (round 13) is the big jump, so the multiplier stays at 200% there and climbs from 15 on to force an end | `DAMAGE_CURVE` = 100% to round 8, 150% from 9, 200% from 12, 250% from 15, 350% 17, 500% 19, 800% 21, 1200% 23. Retune once real siege kill rates are known; first-outs bunch here only because the stand-in gives every defender the same odds | `game/tune.rs` DAMAGE_CURVE; `royal_damage_scales_with_the_round` | open |
| R1.202 | divergence | TFT's recent-opponent rule for any N | TFT's exact numbers are not public; used a window by alive count: 0 for 2, 1 for 3–4, 2 for 5–6, 3 for 7–8. The search narrows the window when it cannot be kept, never below 1 for 3+ players (tested over 30 rounds × 20 seeds × 3–8 players). A ghost counts as meeting its owner. Two players always face each other (nothing else is possible). The ghost goes to the player with fewest ghosts, never twice running when avoidable | `REMATCH_WINDOW` in `game/tune.rs`; `game/pairing.rs` | `no_one_meets_the_same_opponent_two_rounds_running`, `two_players_always_meet_each_other` | done |
| R1.203 | finding | XP curve "TFT-like" | Used TFT's XP per level: 2, 2, 6, 10, 20, 36, 48, 72. The +2 free XP lands at each build start, so round 1 opens at level 2 | `XP_TO_NEXT` in `game/tune.rs` | | done |
| R1.204 | divergence | Rope "cheap" | Gold is whole numbers and wood already costs 1, so rope costs 1 too and unlocks at level 2 | `ROPE_COST`, `ROPE_LEVEL` in `game/tune.rs` | | open |
| R1.205 | divergence | Who attacks the player sent against a ghost? | Pairs attack each other (TFT), so the ghost's attacker has no real attacker. Chose TFT's answer: a bot trebuchet aimed as the copied player attacks the ghost-assigned player's real castle, and those kills do cost that player health but pay no kill gold (kill gold goes only to an attacker whose pairing target matches the castle) | `Pairing::Ghost` doc in `game/pairing.rs`; `apply_siege` in `game/flow.rs`. The siege layer must run that bot trebuchet | `ghost_kills_pay_the_attacker_but_spare_the_owner` | open |
| R1.206 | worklist | Grid and segment length are physics's to decide | Assumed 4 grid cells per segment length (spacing 16 cells), plots 48×32 cells at level 1 to 112×64 at level 9, 60 rule ticks a second. Physics may differ | `GRID_PER_SEGMENT`, `LEVELS`, `TICKS_PER_SECOND` in `game/tune.rs`; reconcile at merge with physics | | open |
| R1.207 | finding | How rules and physics refer to one piece | `Match::place` returns a ledger id; physics keys the segment or building by it and reports surviving producer ids. Where royals may stand comes from the build layer via `set_standing_spots`; empty means the plot floor | `game/ledger.rs`, `game/flow.rs` | | done |
| R1.208 | finding | Upgrade list | 12 placeholder rows (economy, defense, extra shots, ammo, air) keyed by `key`; names and card text go in data/ by key. Economy and defense apply fully; ammo and air only add an entry to `Modifiers` for the siege to read | `UPGRADES` in `game/upgrades.rs`; adding one is one row | | open |
| R1.209 | divergence | Selling a piece paid from the stockpile, later | Pays half its gold value in gold, rounded down, so a stockpile can turn into gold slowly. Interest is counted on the bank at build start, after last siege's kill gold | `Ledger::sell`, `Purse::collect_income` | `selling_a_later_phase_refunds_half_rounded_down` | open |
| R1.210 | finding | When the siege ends | Timer or everyone ready moves Siege to SiegeOver; the match waits for `apply_siege`, which may also come early. Bots are not marked ready in a siege; the shooting AI decides that | `Match::tick`, `Match::apply_siege` | | done |
| R1.211 | worklist | Map spots (deposits, gold vein, holy well, armory ruin) | Not in the rules yet: they need the map's plot spots | Add as yields keyed on the spot a producer sits on | | open |
| R1.212 | finding | Level and building numbers | Normal shots 3,3,4,4,4,5,5,5,6; power 100–140%; Carpenter level 2 (3 gold, 3 wood), Quarry 4 (6, 1 stone), Armory 5 (6, +1 shot), Mine 7 (10, 1 metal); extra shots 2 gold, 3 a round | `LEVELS`, `BUILDINGS`, `SHOT_COST` in `game/tune.rs` | | open |
| R1.213 | finding | Timer runs out mid-choice | An unpicked upgrade takes the first card; unplaced royals go to a random valid spot, or the spot farthest from the others when the plot is too crowded to keep the spacing | `end_upgrade`, `royals::auto_place` | `unplaced_royals_are_auto_placed_at_valid_spots`, `a_crowded_plot_still_places_every_royal` | done |
| R1.214 | finding | Knocked out in the same siege | Ranked by health left (lower is worse), then seat; if no one is left, the highest health wins | `Match::apply_siege`, `standings` | | done |
| R1.215 | finding | Tagging new tests narrowly | tests/tags.toml tags by file glob, and `crates/**` already covers `game/`; no narrower rule possible without the script unioning it | none added | | done |
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
| R1.500 | divergence | Fleeced compiles sound recipes in the core (`sound.rs`). | Royal Rubble's core is busy with physics; sound needs no game rule. | Recipes compile in a build step, `tools/sound/compile.py`, to `data/sound-programs.json`; the page still holds no numbers. | `notes/sound.md` | done |
| R1.501 | finding | Sound variants would be picked at random. | The boundary test bans `Math` in page JS. | Groups (`impact_wood` and the like) play their variants in turn. | `web/sound/sound.js` | done |
| R1.502 | finding | The noise buffer would sit in `web/sound/`. | The page scan reads every file under `web/` as text and would fail on a WAV. | It lives at `data/sound-noise.wav`. | | done |
| R1.503 | divergence | The volume curve belongs in the core (Fleeced's `master_gain`). | No core settings exist yet. | `sound.js` squares the slider share and applies `master_scale` 0.6 from data; move to the core when settings land. | `crates/core/tests/sound.rs` caps `master_scale` at 0.6 | open |
| R1.504 | finding | Compiled music would be small. | Every note is compiled out: `data/sound-programs.json` is about 1.3 MB. | Left as is; compile the score at load if the size matters. | | open |
| R1.505 | worklist | The sounds are tuned by ear from renders only, not heard in a browser. | Only the person can judge them. | Listen on `notes/sound/preview.html`; build music, siege music and the king's sting most of all. | | person |

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
