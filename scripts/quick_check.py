#!/usr/bin/env python3
"""Run only the check steps the changed files need, and log how long checks take.

    ./scripts/quick-check.sh              the steps the changes since the last full pass need
    ./scripts/quick-check.sh BASE         ... the changes since commit BASE
    ./scripts/quick-check.sh --dry-run    print the plan and run nothing
    ./scripts/quick-check.sh --full       run ./scripts/check.sh and log its time
    python3 scripts/quick_check.py --guard    fail if a file is covered by no rule

tests/tags.toml maps files to the steps of ./scripts/check.sh. The changed
files are those that differ from BASE (by default the commit of the newest
passing full check in notes/check-times.md, so a quick check only ever skips
relative to a full pass), plus uncommitted and untracked files. A changed file
no rule covers runs the whole check. Every run adds a row to
notes/check-times.md; a failing run names the step that failed.

What a pass establishes: the steps that ran passed. It says nothing about the
steps it skipped. ./scripts/check.sh stays the check before a handoff, a
deploy or a publish. The idea and its rules are Peter's (Ape Race, Honey Hole,
Crabble); this is written fresh for the template.
"""

import fnmatch
import os
import subprocess
import sys
import time
import tomllib
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORDER = ["fmt", "clippy", "core", "build", "wasm", "art", "notebook", "journal", "browser"]


def py():
    env = os.environ.get("BUILDER_PYTHON")
    if env:
        return env
    venv = ROOT / ".venv" / "bin" / "python"
    return str(venv) if venv.exists() else sys.executable


COMMANDS = {
    "fmt": lambda: ["cargo", "fmt", "--all", "--", "--check"],
    "clippy": lambda: ["cargo", "clippy", "--workspace", "--all-targets", "--quiet", "--", "-D", "warnings"],
    "core": lambda: ["cargo", "test", "--workspace", "--quiet"],
    "build": lambda: ["./scripts/build.sh"],
    "wasm": lambda: [py(), "scripts/wasm_imports.py", "dist"],
    "art": lambda: [py(), "tests/art_check.py"] if (ROOT / "tests/art_check.py").exists() else ["true"],
    "notebook": lambda: [py(), "scripts/notebook_check.py"],
    "journal": lambda: [py(), "scripts/journal_check.py"],
    "browser": lambda: [py(), "tests/browser/check.py"],
}


def tags():
    return tomllib.loads((ROOT / "tests" / "tags.toml").read_text())


def matches(path, pattern):
    if pattern.endswith("/**"):
        return path == pattern[:-3] or path.startswith(pattern[:-2])
    if "**" in pattern:
        return fnmatch.fnmatch(path, pattern.replace("**/", "*").replace("**", "*"))
    return fnmatch.fnmatch(path, pattern) and path.count("/") == pattern.count("/")


def rules_for(path, conf):
    return [r for r in conf.get("rule", []) if any(matches(path, g) for g in r["files"])]


def ignored(path, conf):
    return any(matches(path, g) for g in conf.get("ignore", []))


def git(*args):
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True)


def project_files():
    out = git("ls-files", "--cached", "--others", "--exclude-standard")
    if out.returncode == 0:
        return [l for l in out.stdout.splitlines() if l and (ROOT / l).exists()]
    skip = {"target", "dist", ".venv", ".git", "node_modules"}
    return sorted(p.relative_to(ROOT).as_posix() for p in ROOT.rglob("*")
                  if p.is_file() and not skip & set(p.relative_to(ROOT).parts))


def last_full_pass():
    log = ROOT / "notes" / "check-times.md"
    if not log.exists():
        return None
    for line in reversed(log.read_text().splitlines()):
        c = [x.strip() for x in line.strip().strip("|").split("|")]
        if len(c) >= 6 and c[2] == "check" and c[-1] == "pass":
            return c[1].rstrip("+")
    return None


def changed(base):
    if git("rev-parse", "--git-dir").returncode != 0:
        return None
    files = set()
    if base:
        d = git("diff", "--name-only", "--relative", base)
        if d.returncode != 0:
            return None
        files |= set(d.stdout.split())
    files |= set(git("diff", "--name-only", "--relative", "HEAD").stdout.split())
    files |= set(git("ls-files", "--others", "--exclude-standard").stdout.split())
    return sorted(files)


def plan(files, conf):
    """(steps in order, why) for these changed files."""
    if files is None:
        return list(ORDER), ["not a git repository, or the base is unknown: the whole check"]
    steps, why = {"fmt"}, []
    for f in files:
        if ignored(f, conf):
            continue
        found = rules_for(f, conf)
        if not found:
            why.append(f"{f}: no rule in tests/tags.toml covers it, so the whole check runs")
            steps.add("all")
            continue
        for r in found:
            steps.update(r["steps"])
            why.append(f"{f}: {r['name']}")
    if "all" in steps:
        return list(ORDER), why
    if "browser" in steps:
        steps.update({"build"})
    if "wasm" in steps:
        steps.add("build")
    return [s for s in ORDER if s in steps], why


def guard(conf):
    names = [r.get("name", "?") for r in conf.get("rule", [])]
    problems = []
    for r in conf.get("rule", []):
        for s in r.get("steps", []):
            if s not in ORDER and s != "all":
                problems.append(f"rule {r.get('name')!r} names an unknown step {s!r}")
    for f in project_files():
        if not ignored(f, conf) and not rules_for(f, conf):
            problems.append(f"no rule in tests/tags.toml covers {f}: add it to a rule, or to ignore")
    for p in problems:
        print(f"FAIL {p}")
    if not problems:
        print(f"test tags: every file is covered by one of {len(names)} rules")
    return 1 if problems else 0


def log(kind, times, result):
    path = ROOT / "notes" / "check-times.md"
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("# Check times\n\nOne row per run of ./scripts/quick-check.sh, newest last. "
                        "A quick check skips relative to the newest passing `check` row.\n\n"
                        "| When | Commit | Check | Steps (s) | Total (s) | Result |\n| --- | --- | --- | --- | --- | --- |\n")
    head = git("rev-parse", "--short", "HEAD").stdout.strip() or "-"
    dirty = "+" if git("status", "--porcelain").stdout.strip() else ""
    steps = " ".join(f"{k}={v:.1f}" for k, v in times.items()) or "-"
    with path.open("a") as f:
        f.write(f"| {datetime.now():%Y-%m-%d %H:%M} | {head}{dirty} | {kind} | {steps} | {sum(times.values()):.1f} | {result} |\n")


def run(kind, steps):
    times = {}
    for s in steps:
        print(f"\n== quick: {s}", flush=True)
        t0 = time.monotonic()
        code = subprocess.run(COMMANDS[s](), cwd=ROOT).returncode
        times[s] = time.monotonic() - t0
        if code != 0:
            log(kind, times, f"FAIL at {s}")
            print(f"\nQUICK CHECK FAILED at {s}")
            return 1
    log(kind, times, "pass")
    print(f"\nQUICK CHECK PASSED ({', '.join(steps)}); ./scripts/check.sh is still the check before a handoff or deploy")
    return 0


def main(argv):
    conf = tags()
    if "--guard" in argv:
        return guard(conf)
    if "--full" in argv:
        t0 = time.monotonic()
        code = subprocess.run(["./scripts/check.sh"], cwd=ROOT).returncode
        log("check", {"all": time.monotonic() - t0}, "pass" if code == 0 else "FAIL")
        return code
    args = [a for a in argv if not a.startswith("--")]
    base = args[0] if args else last_full_pass()
    files = changed(base)
    steps, why = plan(files, conf)
    print(f"base: {base or 'HEAD (no passing full check logged yet)'}")
    for w in why:
        print(f"  {w}")
    print(f"steps: {', '.join(steps)}")
    if "--dry-run" in argv:
        return 0
    return run("quick", steps)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
