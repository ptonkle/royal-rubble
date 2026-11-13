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
| AI aiming decision time (201-segment castle, 3 royals, release) | `cargo test --release --test ai_probe -- --ignored --nocapture` | plain shots: easy 9.9 ms, normal 9.9, hard 19.1 mean (19.5 worst); with drop: 10.6, 11.3, 22.8 (23.5); with drop and split: 12.7, 16.0, 33.9 (38.7) | 2026-11-12, WSL2 |
| Siege tick time (2 castles of 148 segments, 2 trebuchets, every shot, release) | `cargo test --release --test play_probe -- --ignored --nocapture` | 293 µs mean tick, 500 µs worst over 991 ticks; strain static test of one castle 34 ms; both castles built by input 22 ms; siege start 0.2 ms | 2026-11-12, WSL2 |
| Match glue breaks caught | `python3 scripts/break_tests.py play:` | 11 of 11 | 2026-11-12 |

## Not measured

Devices, browsers and conditions no measurement here covers.
