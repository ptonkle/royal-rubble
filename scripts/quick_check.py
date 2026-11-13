#!/usr/bin/env python3
"""Run only the check steps the changed files need, and log how long checks take.

    ./scripts/quick-check.sh              the steps the changes need, since the newer of the
                                          last full pass and the merge base with main
    ./scripts/quick-check.sh BASE         ... the changes since commit BASE
    ./scripts/quick-check.sh --dry-run    print the plan and run nothing
    ./scripts/quick-check.sh --full       run ./scripts/check.sh and log its time
    python3 scripts/quick_check.py --guard    fail if a file is covered by no rule

tests/tags.toml maps files to the steps of ./scripts/check.sh. The changed
files are those that differ from BASE (by default the newer of the newest
passing full check in notes/check-times.md and the merge base with main, so a
quick check only ever skips relative to a full pass or to what main already
holds), plus uncommitted and untracked files. A changed file no rule covers
runs the whole check. Every run adds a row to notes/check-times.md; a failing
run names the step that failed.

The core step runs only the cargo tests the changes need (scripts/tags.py), and
the browser step runs in Chromium alone; ./scripts/check.sh runs everything.

What a pass establishes: the steps that ran passed. It says nothing about the
steps it skipped. ./scripts/check.sh stays the check before a handoff, a
deploy or a publish. The idea and its rules are Peter's (Ape Race, Honey Hole,
Crabble); this is written fresh for the template.
"""

import fnmatch
import importlib.util
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


def tag_tool():
    spec = importlib.util.spec_from_file_location("tags", ROOT / "scripts" / "tags.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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


def is_ancestor(a, b):
    return git("merge-base", "--is-ancestor", a, b).returncode == 0


def default_base():
    """The newer of the last full pass and the merge base with main."""
    last = last_full_pass()
    found = [last] if last and is_ancestor(last, "HEAD") else []
    merge_base = git("merge-base", "HEAD", "main").stdout.strip()
    if merge_base:
        found.append(merge_base)
    if len(found) == 2 and is_ancestor(found[0], found[1]):
        return found[1]
    return found[0] if found else None


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


def core_files(files, conf):
    """The changed files whose rules reach the core tests."""
    def reaches(f):
        found = rules_for(f, conf)
        return not found or any({"core", "all"} & set(r["steps"]) for r in found)

    return [f for f in files if not ignored(f, conf) and reaches(f)]


def picked_tests(base, files, conf):
    """The cargo tests the change needs, by exact name, or None for all."""
    try:
        return tag_tool().pick(conf, base or "HEAD", core_files(files, conf))
    except subprocess.CalledProcessError:
        print("the tests do not compile: running them all, so the error shows")
        return None


def run(kind, steps, names=None):
    times = {}
    env = {**os.environ, "BUILDER_ENGINES": os.environ.get("BUILDER_ENGINES", "chromium")}
    for s in steps:
        print(f"\n== quick: {s}", flush=True)
        t0 = time.monotonic()
        command = COMMANDS[s]()
        if s == "core" and names is not None:
            command = ["cargo", "test", "--workspace", "--quiet", "--lib", "--tests", "--", "--exact", *names]
        code = subprocess.run(command, cwd=ROOT, env=env).returncode
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
    base = args[0] if args else default_base()
    files = changed(base)
    steps, why = plan(files, conf)
    print(f"base: {base or 'HEAD (no passing full check logged yet)'}")
    for w in why:
        print(f"  {w}")
    print(f"steps: {', '.join(steps)}")
    whole = files is None or steps == ORDER
    names = None if whole or "core" not in steps else picked_tests(base, files, conf)
    if "--dry-run" in argv:
        return 0
    return run("quick", steps, names)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
