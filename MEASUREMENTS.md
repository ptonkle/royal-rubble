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

## Not measured

Devices, browsers and conditions no measurement here covers.
