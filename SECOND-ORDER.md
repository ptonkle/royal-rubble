# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | | | | | | |
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
