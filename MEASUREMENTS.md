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
| Cantilever strain (wood, off a braced wood tower) | `cargo test --release --test physics_probe probe_cantilever_strain -- --ignored --nocapture` | root strain 12% (1 unit), 71% (2), 147% breaks (3), 426% breaks (4) | 2026-11-12 |
| Physics breaks caught | `python3 scripts/break_tests.py physics` | 22 of 22 | 2026-11-12 |
| Siege tick time (2 castles of 148 segments, 2 trebuchets, every shot, release) | `cargo test --release --test play_probe -- --ignored --nocapture` | 293 µs mean tick, 500 µs worst over 991 ticks; strain static test of one castle 34 ms; both castles built by input 22 ms; siege start 0.2 ms | 2026-11-12, WSL2 |
| Match glue breaks caught | `python3 scripts/break_tests.py play:` | 11 of 11 | 2026-11-12 |
| Trebuchet landing by release tick (R1.1101), normal shot, pad 30 cells below a plot from 120 cells out | one-off probe with `land_x` in play/tests.rs (flat pad to col 120, ramp to +30) | level 1 (power 0), release:cells out: 34:13 35:35 36:57 37:79 38:100 39:118 40:130 41:146 42:162 43:176 44:188 45:199 46:209 47:217 48:222 49:226 50:227 51:227 52:224 53:219 54:212 55:203 56:190 57:174 58:153 59:125 (60-66 land on the ramp near 120) 67:92 68:78 69:65. Level 9 (power 40): 34:30 36:116 38:193 40:267 42:332 44:387 46:430 48:460 50:474 51:475 52:473 54:456 56:423 58:372 60:303 61:259 62:200 (63-68 ramp) 69:95. Forward-and-up window 34-69 (36 ticks) | 2026-11-12 |
| Wiring breaks caught | `python3 scripts/break_tests.py "wire:"` | 11 of 11 | 2026-11-12 |

## Not measured

Devices, browsers and conditions no measurement here covers.
