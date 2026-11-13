# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | | |
| Core tests | `./scripts/count-tests.sh` | | |
| Full check | `./scripts/check.sh` | | |
| WebAssembly size | printed by `./scripts/build.sh` | | |
| Breaks caught | `python3 scripts/break_tests.py` | | |
| Physics step time (201-segment castle, 5 shots, 600 ticks, release) | `cargo test --release --test physics_probe probe_step_time -- --ignored --nocapture` | 174 µs mean step, 594 µs worst step (15 segments broken, 30 debris); static test of the castle: 0 breaks | 2026-11-12, WSL2, 16 threads |
| Physics step time with swept shots (R1.1455), same probe | same | 180 µs mean step, 383 µs worst (13 broken, 26 debris) | 2026-11-12, WSL2 |
| Cantilever strain (wood, off a braced wood tower) | `cargo test --release --test physics_probe probe_cantilever_strain -- --ignored --nocapture` | root strain 12% (1 unit), 71% (2), 147% breaks (3), 426% breaks (4) | 2026-11-12 |
| Physics breaks caught | `python3 scripts/break_tests.py physics` | 22 of 22 | 2026-11-12 |
| AI aiming decision time (201-segment castle, 3 royals, release) | `cargo test --release --test ai_probe -- --ignored --nocapture --test-threads=1` | one shot, plain: easy 10.1 ms, normal 9.9, hard 21.1 mean (26.1 worst); with drop: 11.0, 12.2, 23.5 (23.9); with drop and split: 13.8, 18.4, 45.1 (45.5). Whole siege plan of 5 shots: 100, 98, 149 ms; with drop 104, 108, 164 ms | 2026-11-12, WSL2 |
| Siege tick time (2 castles of 148 segments, 2 trebuchets, every shot, release) | `cargo test --release --test play_probe -- --ignored --nocapture` | 293 µs mean tick, 500 µs worst over 991 ticks; strain static test of one castle 34 ms; both castles built by input 22 ms; siege start 0.2 ms | 2026-11-12, WSL2 |
| Match glue breaks caught | `python3 scripts/break_tests.py play:` | 11 of 11 | 2026-11-12 |
| Trebuchet landing by release tick (R1.1101), normal shot, pad 30 cells below a plot from 120 cells out | one-off probe with `land_x` in play/tests.rs (flat pad to col 120, ramp to +30) | level 1 (power 0), release:cells out: 34:13 35:35 36:57 37:79 38:100 39:118 40:130 41:146 42:162 43:176 44:188 45:199 46:209 47:217 48:222 49:226 50:227 51:227 52:224 53:219 54:212 55:203 56:190 57:174 58:153 59:125 (60-66 land on the ramp near 120) 67:92 68:78 69:65. Level 9 (power 40): 34:30 36:116 38:193 40:267 42:332 44:387 46:430 48:460 50:474 51:475 52:473 54:456 56:423 58:372 60:303 61:259 62:200 (63-68 ramp) 69:95. Forward-and-up window 34-69 (36 ticks) | 2026-11-12 |
| Trebuchet landing by release tick (R1.1300), normal shot, terrain seed 1 (trebuchet at column 14; plot front 135 cells out; level-1 plot to 175, level 5 to 203, level 9 to 231) | one-off probe with `land_x` in play/tests.rs; cells out from the trebuchet | level 1 (100%): 20:39 22:68 24:81 26:88 28:95 30:102 32:110 34:120 36:126 38:131 **40:137 41:140 42:143 43:147 44:150 45:153 46:156 47:159 48:162 49:164 50:167 51:170 52:172 53:174** 54:177 56:183 58:195 60:203 62:216 64:230 66:239 68:243 70:242 74:235 78:228 82:210 86:204 90:202 94:198 98:183 100:179 102:176 **103:174 104:172 105:171 106:169 107:167 108:165 109:163 110:161 111:159 112:156 113:154 114:151 115:148 116:145 117:139** 118:42 (hill) to 158. Level 5 (106%): 30:119 **35:136 ... 55:202** (21 ticks) 60:231 68:275 (top) 80:252 94:208 **99:202 ... 119:152** (21 ticks) 120:41 (hill). Level 9 (115%): 26:121 **30:138 ... 52:229** (23 ticks) 57:251 64-79:301 (world edge) 90:275 **99:245 ... 120:168** (22 ticks) 121:41 (hill). Shortest back-to-back run on the plot over seeds 1-4: level 1 12 ticks, level 5 20, level 9 22 (all seed 4). Latest release on a plot: tick 139 (level 1), 148 (level 9); the arm stops at 205 | 2026-11-12 |
| Wiring breaks caught | `python3 scripts/break_tests.py "wire:"` | 11 of 11 | 2026-11-12 |
| Field draw time, 150-segment castle, 1920x1080, headless Chromium (baked textures) | `.venv/bin/python -I notes/look/stitch/render.py notes/look/stitch/perf.html --json` | first frame 152 ms (picture decode and patterns); still view 3.92 ms a frame; moving view (background redrawn each frame) 12.78 ms a frame | 2026-11-12 |

## Not measured

Devices, browsers and conditions no measurement here covers.
