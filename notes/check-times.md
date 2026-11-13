# Check times

One row per run of ./scripts/quick-check.sh, newest last. A quick check skips relative to the newest passing `check` row.

| When | Commit | Check | Steps (s) | Total (s) | Result |
| --- | --- | --- | --- | --- | --- |
| 2026-11-12 19:58 | bac9a6a+ | quick | fmt=0.1 clippy=1.9 core=1.0 build=1.5 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=4.6 | 9.3 | pass |
| 2026-11-12 20:00 | d968fc0+ | quick | fmt=0.1 clippy=2.2 core=0.5 | 2.8 | FAIL at core |
| 2026-11-12 20:01 | b9f6cae+ | quick | fmt=0.1 clippy=0.1 core=0.3 build=4.5 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=13.0 | 18.2 | pass |
| 2026-11-12 20:03 | 9abce6b+ | quick | fmt=0.1 clippy=0.9 core=0.3 build=1.8 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=12.6 | 16.0 | pass |
| 2026-11-12 20:22 | edb403d | quick | fmt=0.1 clippy=2.4 | 2.5 | FAIL at clippy |
| 2026-11-12 20:25 | 49820f8+ | quick | fmt=0.1 clippy=4.5 core=9.0 | 13.7 | FAIL at core |
| 2026-11-12 20:27 | 1b5e630+ | quick | fmt=0.1 clippy=0.1 core=0.4 build=0.2 wasm=0.1 art=0.1 notebook=0.0 | 0.9 | FAIL at notebook |
| 2026-11-12 20:30 | f84b809 | quick | fmt=0.2 | 0.2 | pass |
| 2026-11-12 20:46 | ddbe8c9 | quick | fmt=0.2 clippy=0.1 core=8.6 | 8.8 | FAIL at core |
| 2026-11-12 23:23 | c48920c+ | quick | fmt=0.2 clippy=2.1 core=7.7 | 10.0 | FAIL at core |
| 2026-11-12 23:23 | c48920c+ | quick | fmt=0.2 clippy=0.7 core=7.7 build=2.8 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=0.1 | 11.7 | FAIL at browser |
| 2026-11-13 00:09 | fc13224+ | quick | fmt=0.2 clippy=2.1 core=66.2 | 68.5 | FAIL at core |
| 2026-11-13 01:20 | 7767851 | quick | fmt=0.2 clippy=0.1 core=25.5 build=5.5 wasm=0.1 notebook=0.0 | 31.4 | FAIL at notebook |
| 2026-11-13 01:22 | 86b645e | quick | fmt=0.2 clippy=0.1 core=25.9 build=0.2 wasm=0.1 notebook=0.0 journal=0.0 browser=41.7 | 68.2 | pass |
| 2026-11-13 01:48 | 1e850c9 | quick | fmt=0.2 clippy=2.5 core=3.8 | 6.5 | FAIL at core |
| 2026-11-13 01:49 | 6c722f0 | quick | fmt=0.2 clippy=0.8 core=4.1 | 5.1 | FAIL at core |
| 2026-11-13 01:50 | 6c722f0+ | quick | fmt=0.2 clippy=0.7 core=4.1 build=6.1 wasm=0.1 notebook=0.0 | 11.2 | FAIL at notebook |
| 2026-11-13 01:51 | 6c722f0+ | quick | fmt=0.2 clippy=0.1 core=4.1 build=0.2 wasm=0.1 notebook=0.0 journal=0.0 browser=48.0 | 52.7 | pass |
| 2026-11-13 02:06 | 74f8e06 | quick | fmt=0.2 clippy=3.1 core=29.2 build=0.2 wasm=0.1 art=0.1 notebook=0.0 | 32.8 | FAIL at notebook |
| 2026-11-13 02:10 | 74f8e06+ | quick | fmt=0.2 clippy=0.1 core=25.8 build=0.2 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=41.1 | 67.6 | pass |
| 2026-11-13 03:43 | 754b255 | quick | fmt=0.2 clippy=3.0 core=20.9 build=0.2 wasm=0.1 art=0.1 notebook=0.0 journal=0.0 browser=55.3 | 79.9 | pass |
| 2026-11-13 15:48 | d9a936b | quick | fmt=0.2 clippy=3.6 core=0.1 build=6.4 wasm=0.1 notebook=0.1 journal=0.0 browser=49.7 | 60.2 | pass |
| 2026-11-13 15:54 | c4dd125 | quick | fmt=0.3 clippy=5.5 core=32.9 | 38.7 | FAIL at core |
