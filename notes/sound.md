# Sound: how it is made and how the page calls it

Everything is synthesized; nothing is recorded or downloaded. The machinery is ported from Fleeced (its `web/audio.js` synth and `tools/sound/render.py`), not its sounds.

## Pipeline

1. `tools/sound/author.py` writes `data/sounds.json`: every recipe (layers of waves, noise, filters, envelopes, strikes) and the two music tracks (instruments plus a score: mode, tempo, sections, parts). Edit recipes there.
2. `tools/sound/compile.py` compiles it to `data/sound-programs.json` (Web Audio schedules, every value final) and `data/sound-noise.wav` (one second of seeded noise, the synth's only buffer).
3. `web/synth.js` plays the programs (no numbers in it). `web/sound.js` is the API below.
4. `tools/sound/render.py` renders every sound to `notes/sound/wav/` (git-ignored; about 20 MB). `tools/sound/preview.py` writes `notes/sound/preview.html`, a one-file listening page that plays everything live through the game's own synth.

Rerun after any recipe change:

    python3 tools/sound/author.py && python3 tools/sound/compile.py && python3 tools/sound/preview.py

## Integration (for the shell and match screens)

```js
import { initSound, play, startLoop, stopLoop, music, urgent,
         setVolume, setMusicVolume, setEffectsVolume } from "./sound.js";

await initSound();          // once at start; sound wakes on the first press or key
setVolume(slider / max);    // main slider, 0 to 1; on a square, times master_scale (0.6)
setMusicVolume(share);      // hook for a separate Music slider (0 to 1); default 1
setEffectsVolume(share);    // hook for an Effects slider; default 1
```

The volume fractions should come from the core or settings, not be computed in page JS. The square law and the 0.6 cap are applied inside `sound.js` from `data/sounds.json` (`setup.master_scale`, `music_gain`).

| When | Call |
| --- | --- |
| Build or upgrade phase starts | `play("horn_build"); music("music_build")` |
| Siege phase starts | `play("horn_siege"); music("music_siege")` |
| Last 10 seconds of a phase | The core cues the war drum: `drum_count` each new second from 10 to 4, `drum_close` (harder, with a second beat close behind) from 3 to 1 (R1.3800, `drum_beat` in `app/arena.rs`). `urgent(true)` is not yet wired |
| Trebuchet swing starts / release | `play("trebuchet_creak")` / `play("trebuchet_release")` |
| Shot in the air / lands | `startLoop("shot_whoosh", shotId)` / `stopLoop(shotId)` |
| Hit by material | `play("impact_wood")`, `play("impact_stone")`, `play("impact_metal")` (groups rotate variants); `opts.gain` 0 to 1 for a light hit |
| Segment overstressed | `play("segment_snap")` |
| Collapse | `play("collapse", { count: segmentsFallen })` (small from 1, medium from 5, big from 12) |
| Debris | `play("debris")` |
| Moat | `play("splash")` |
| Exploding ammo | `play("explosion")` |
| Fire | `play("fire_ignite")`, then `startLoop("fire_crackle", "fire")` while anything burns, `stopLoop("fire")` |
| Air effects | `phase_shimmer`, `split_pop`, `dive_whistle`, `air_drop`, `air_reverse` |
| Royal killed | The core cues `royal_` + rank (squire, knight, princess, prince, queen, king), a short cry of death, for each royal killed in the castle you attack and in your own (R1.3801, `cry_kills` in `app/arena.rs`) |
| Build actions | `place_segment`, `sell_segment`, `coin` (gold gained), `buy_xp`, `level_up` |
| Upgrades | `cards_reveal`, `card_pick`, `reroll` |
| End of game | `elimination` (a player is out), `victory` |
| Menus | `ui_click` on press, `ui_hover` on hover (the same sound never stacks within `repeat_gap`) |
| Leaving a match | `music(null)` fades out |

The page marks `body[data-audio]` (`waiting`, `started`), `data-last-sound` and `data-music` for browser checks.

## Music

- **Build and upgrade** (`music_build`, 80 s loop): D Dorian at 96 bpm. A plucked lute arpeggio with a bass note on each bar, a recorder melody, a tabor and a soft D–A drone. Four sections (A, B, a quieter C with long recorder notes, A with a varied arpeggio) so it doesn't grate. Original melody.
- **Siege** (`music_siege`, 96 s loop): A minor at 120 bpm. Timpani and a low drone from the start; war drum and a string ostinato join, then snare and a shawm call; the last two sections repeat at full strength.
- **Last seconds** (`music_siege_urgent`, 4 s loop): woodblock sixteenths and a high string figure, layered over the siege music by `urgent(true)`.
- Tracks crossfade over about a second (`setup.crossfade` is the time constant, 0.33 s). Music sits at `music_gain` 0.32 under the effects.

Loops have no seam: drones and beds run on from one start, and the notes are scheduled half a second ahead, rep after rep.
