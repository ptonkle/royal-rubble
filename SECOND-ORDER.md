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

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
