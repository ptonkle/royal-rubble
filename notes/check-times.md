# Check times

One row per run of ./scripts/quick-check.sh, newest last. A quick check skips relative to the newest passing `check` row.

| When | Commit | Check | Steps (s) | Total (s) | Result |
| --- | --- | --- | --- | --- | --- |
| 2026-11-12 19:58 | bac9a6a+ | quick | fmt=0.1 clippy=1.9 core=1.0 build=1.5 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=4.6 | 9.3 | pass |
| 2026-11-12 20:00 | d968fc0+ | quick | fmt=0.1 clippy=2.2 core=0.5 | 2.8 | FAIL at core |
| 2026-11-12 20:01 | b9f6cae+ | quick | fmt=0.1 clippy=0.1 core=0.3 build=4.5 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=13.0 | 18.2 | pass |
| 2026-11-12 20:03 | 9abce6b+ | quick | fmt=0.1 clippy=0.9 core=0.3 build=1.8 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=12.6 | 16.0 | pass |
