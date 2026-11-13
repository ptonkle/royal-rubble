#!/usr/bin/env python3
"""Test tags: which cargo tests a change needs (tests/tags.toml). After Ape Race's
and Chromatic's.

    scripts/tags.py guard    fail if a tag is unknown or a cargo test has no tags

scripts/quick_check.py uses `pick`. A [[rule]] in tests/tags.toml says which
check steps a file needs; a rule may also name `tags`, and then a change to
that file runs only the cargo tests whose [tests] tags meet them. A file no
tagged rule covers runs every test, so a rule with no tags is the safe default.

A changed file is placed line by line where it can be (file_plan): a test file
runs its own tests only; a line in a `#[cfg(test)] mod` runs its test (or the
module's tests, for a helper); a line in a function named in [functions] takes
that function's tags. Any other line takes the file's tags. A change to
comments and blank lines only runs nothing.
"""
import json
import pathlib
import re
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
TAGS = ROOT / "tests" / "tags.toml"
ALL = "all"


def load():
    return tomllib.loads(TAGS.read_text())


def glob_re(glob):
    out = ""
    i = 0
    while i < len(glob):
        if glob.startswith("**/", i):
            out += "(?:.*/)?"
            i += 3
        elif glob.startswith("**", i):
            out += ".*"
            i += 2
        elif glob[i] == "*":
            out += "[^/]*"
            i += 1
        elif glob[i] == "?":
            out += "[^/]"
            i += 1
        else:
            out += re.escape(glob[i])
            i += 1
    return re.compile(out + r"\Z")


def file_tags(cfg, path):
    """The tags of one changed file: those of every rule that covers it and
    names tags. None when none does, which runs every test."""
    hit = None
    for rule in cfg.get("rule", []):
        if "tags" in rule and any(glob_re(g).match(path) for g in rule["files"]):
            hit = (hit or set()) | set(rule["tags"])
    return hit


def sh(cmd, **kw):
    return subprocess.run(cmd, cwd=ROOT, text=True, check=True, capture_output=True, **kw).stdout


# --------------------------------------------------------------- cargo tests


def cargo_tests():
    """Every cargo test as an id: `lib:<path>` for the core's own tests,
    `shim:<path>` for the shim's, `test:<file>:<name>` for integration tests."""
    out = subprocess.run(
        ["cargo", "test", "--workspace", "--no-run", "--message-format=json", "--quiet"],
        cwd=ROOT, text=True, check=True, stdout=subprocess.PIPE,
    ).stdout
    ids = []
    for line in out.splitlines():
        if not line.startswith("{"):
            continue
        m = json.loads(line)
        if m.get("reason") != "compiler-artifact" or not m.get("executable") or not m["profile"]["test"]:
            continue
        t = m["target"]
        if "test" in t["kind"]:
            prefix = f"test:{t['name']}:"
        elif t["name"] == "tool_core":
            prefix = "lib:"
        else:
            prefix = "shim:"
        listed = subprocess.run(
            [m["executable"], "--list", "--format", "terse"], cwd=ROOT, text=True, check=True, capture_output=True
        ).stdout
        for row in listed.splitlines():
            if row.endswith(": test"):
                ids.append(prefix + row[: -len(": test")])
    return ids


def key_prefix(key):
    # `test:pipe` covers the file's tests, not `test:pipe_perf`
    return key + ":" if key.startswith("test:") and key.count(":") == 1 else key


def test_tags(cfg, tid):
    best = None
    for key, tags in cfg["tests"].items():
        p = key_prefix(key)
        if tid.startswith(p) and (best is None or len(p) > len(best[0])):
            best = (p, tags)
    return None if best is None else set(best[1])


def implied(cfg, hit):
    """The hit tags with everything [implies] adds, followed transitively."""
    out, todo = set(hit), list(hit)
    while todo:
        for t in cfg.get("implies", {}).get(todo.pop(), []):
            if t not in out:
                out.add(t)
                todo.append(t)
    return out


def wanted(tags, hit):
    return ALL in hit or bool(tags & hit)


# --------------------------------------------------------------------- guard


def guard():
    cfg = load()
    known = set(cfg["tags"])
    problems = []

    def check_names(where, tags):
        for t in tags:
            if t not in known:
                problems.append(f"{where}: unknown tag {t!r}")

    for rule in cfg.get("rule", []):
        check_names(f"rule {rule.get('name')!r}", rule.get("tags", []))
    for k, tags in cfg["tests"].items():
        check_names(f"[tests] {k}", tags)
        if not tags:
            problems.append(f"[tests] {k} has an empty tag list")
    check_names("[always]", cfg["always"]["tags"])
    for k, tags in cfg.get("implies", {}).items():
        check_names(f"[implies] {k}", [k, *tags])
    for path, funcs in cfg.get("functions", {}).items():
        for pat, tags in funcs.items():
            check_names(f"[functions] {path} {pat}", tags)
            try:
                re.compile(pat)
            except re.error as e:
                problems.append(f"[functions] {path}: bad pattern {pat!r}: {e}")

    ids = cargo_tests()
    used = set()
    for tid in ids:
        if not test_tags(cfg, tid):
            problems.append(f"test {tid} has no tags in [tests]")
        for key in cfg["tests"]:
            if tid.startswith(key_prefix(key)):
                used.add(key)
    # not a failure: the table names the contract's modules before they land
    waiting = sorted(k for k in cfg["tests"] if k not in used)
    print(f"tags: {len(ids)} cargo tests, {len(cfg['tests'])} rows")
    if waiting:
        print(f"rows matching no test yet: {', '.join(waiting)}")
    if problems:
        print("TAG GUARD FAILED (classify them in tests/tags.toml):")
        for p in problems:
            print("  - " + p)
        return 1
    print("every test has tags")
    return 0


# ---------------------------------------------------------- placing a change


ITEM = re.compile(
    r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe|extern\s+"[^"]*")\s+)*(fn|mod|struct|enum)\s+(\w+)')
LEAD = re.compile(r"^\s*(//|#\[|#!\[)")


def rust_items(text):
    """The fn, mod, struct and enum items of a Rust file as (kind, name,
    first, last, attrs): 1-based lines, `first` taking in the doc comments
    and attributes above the item. Braces are counted outside strings, chars
    and comments."""
    lines = text.split("\n")
    items, stack, pending = [], [], None
    depth, state, hashes, block = 0, None, 0, 0
    for no, line in enumerate(lines, 1):
        if state is None:
            m = ITEM.match(line)
            if m:
                first = no
                while first > 1 and LEAD.match(lines[first - 2]):
                    first -= 1
                attrs = " ".join(lines[first - 1:no - 1])
                pending = (m.group(1), m.group(2), first, attrs, depth)
        i, n = 0, len(line)
        while i < n:
            c = line[i]
            if state == "block":
                if line.startswith("*/", i):
                    block -= 1
                    i += 2
                    if not block:
                        state = None
                    continue
                if line.startswith("/*", i):
                    block += 1
                    i += 2
                    continue
                i += 1
                continue
            if state == "str":
                if c == "\\":
                    i += 2
                    continue
                if c == '"':
                    state = None
                i += 1
                continue
            if state == "raw":
                if line.startswith('"' + "#" * hashes, i):
                    state = None
                    i += 1 + hashes
                    continue
                i += 1
                continue
            if line.startswith("//", i):
                break
            if line.startswith("/*", i):
                state, block = "block", 1
                i += 2
                continue
            m = re.match(r'b?r(#*)"', line[i:])
            if m and (i == 0 or not (line[i - 1].isalnum() or line[i - 1] == "_")):
                state, hashes = "raw", len(m.group(1))
                i += len(m.group(0))
                continue
            if c == '"':
                state = "str"
            elif c == "'":
                if line.startswith("\\", i + 1):
                    j = line.find("'", i + 2)
                    i = (j if j > 0 else i) + 1
                    continue
                if i + 2 < n and line[i + 2] == "'":
                    i += 3
                    continue
            elif c == ";" and pending and pending[4] == depth:
                pending = None
            elif c == "{":
                if pending and pending[4] == depth:
                    stack.append(pending)
                    pending = None
                depth += 1
            elif c == "}":
                depth -= 1
                if stack and stack[-1][4] == depth:
                    kind, name, first, attrs, _ = stack.pop()
                    items.append((kind, name, first, no, attrs))
            i += 1
    return items


def enclosing(items, line):
    """The items holding a line, outermost first."""
    return sorted((it for it in items if it[2] <= line <= it[3]), key=lambda it: it[2])


def hunks(base, rng, path):
    """The changed lines of one file's diff as (side, line number, text):
    side '-' numbers the old file's lines, '+' the new file's."""
    out = sh(["git", "diff", "-U0", "--no-color", rng or base, "--", path])
    rows, old, new = [], 0, 0
    for ln in out.splitlines():
        m = re.match(r"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@", ln)
        if m:
            old, new = int(m.group(1)), int(m.group(2))
        elif ln.startswith(("+++ ", "--- ")):
            continue
        elif ln[:1] == "-":
            rows.append(("-", old, ln[1:]))
            old += 1
        elif ln[:1] == "+":
            rows.append(("+", new, ln[1:]))
            new += 1
    return rows


def side_text(base, rng, path, side):
    """A file's text before ('-') or after ('+') the change."""
    if rng:
        a, b = rng.split("..", 1)
        rev = a if side == "-" else b
    elif side == "-":
        rev = base
    else:
        p = ROOT / path
        return p.read_text(errors="replace") if p.exists() else ""
    r = subprocess.run(["git", "show", f"{rev}:{path}"], cwd=ROOT, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""


def module_path(path):
    """`crates/core/src/online/tests.rs` -> `online::tests`."""
    rel = path[len("crates/core/src/"):-len(".rs")]
    rel = re.sub(r"(^|/)(lib|mod)$", "", rel)
    return rel.replace("/", "::")


def is_test_mod(item):
    return item[0] == "mod" and ("cfg(test)" in item[4] or item[1] == "tests" or item[1].endswith("_tests"))


def test_file_prefix(path):
    """The test id prefix of a file that holds only tests, or None."""
    m = re.fullmatch(r"crates/core/tests/(\w+)\.rs", path)
    if m:
        return f"test:{m.group(1)}:"
    if path.startswith("crates/core/src/") and re.search(r"(^|/|_)tests\.rs$", path):
        return f"lib:{module_path(path)}::"
    return None


def picked_by(picks, tid):
    """Whether a test id is one of the picks: an exact id, or a prefix
    ending `:` or `::` (a test file's or a test module's tests)."""
    return any(tid == p or (p.endswith(":") and tid.startswith(p)) for p in picks)


def test_pick(path, chain, ids):
    """The tests a line in a `#[cfg(test)] mod` needs: the test it sits in,
    or the module's tests (a helper, a `use`), or None outside one."""
    mods = []
    for i, it in enumerate(chain):
        if it[0] != "mod":
            return None
        mods.append(it[1])
        if is_test_mod(it):
            mp = "::".join(x for x in [module_path(path), *mods] if x)
            inner = chain[i + 1] if i + 1 < len(chain) else None
            if inner and inner[0] == "fn" and f"lib:{mp}::{inner[1]}" in ids():
                return f"lib:{mp}::{inner[1]}"
            prefix = f"lib:{mp}::"
            return prefix if any(t.startswith(prefix) for t in ids()) else None
    return None


COMMENT = {".rs": "//", ".js": "//", ".py": "#", ".sh": "#", ".toml": "#"}


def comment_only(base, rng, path):
    """Every changed line of a file is blank or a whole-line comment."""
    lead = COMMENT.get(pathlib.Path(path).suffix)
    if not lead:
        return False
    rows = hunks(base, rng, path)
    return bool(rows) and all(not t.strip() or t.strip().startswith(lead) for _, _, t in rows)


def file_plan(cfg, path, base, rng, ids):
    """One changed file's (tags, cargo test picks)."""
    if comment_only(base, rng, path):
        return set(), set()
    fallback = file_tags(cfg, path)
    fallback = {ALL} if fallback is None else fallback
    prefix = test_file_prefix(path)
    if prefix:
        if any(t.startswith(prefix) for t in ids()):
            return set(), {prefix}
        if not (ROOT / path).exists() and not rng:
            return set(), set()
        return fallback, set()
    funcs = cfg.get("functions", {}).get(path)
    rust_core = path.startswith("crates/core/src/") and path.endswith(".rs")
    if not (funcs or rust_core):
        return fallback, set()
    fpats = [(re.compile(k), set(v)) for k, v in (funcs or {}).items()]
    items = {}
    tags, picks = set(), set()
    rows = hunks(base, rng, path)
    if not rows:  # an untracked file, a binary one: no lines to place
        return fallback, set()
    for side, no, text in rows:
        if not text.strip():  # a blank line breaks nothing
            continue
        if side not in items:
            items[side] = rust_items(side_text(base, rng, path, side))
        chain = enclosing(items[side], no)
        pick = test_pick(path, chain, ids) if rust_core else None
        if pick:
            picks.add(pick)
            continue
        fn = next((it[1] for it in chain if it[0] == "fn"), None)
        hit = set().union(*(t for p, t in fpats if fn and p.search(fn))) if fn else set()
        tags |= hit or fallback
    return tags, picks


def plan(cfg, files, base, rng, ids):
    hit = set(cfg["always"]["tags"])
    rows, picks = [], set()
    for f in files:
        tags, p = file_plan(cfg, f, base, rng, ids)
        rows.append((f, tags, p))
        hit |= tags
        picks |= p
    return rows, implied(cfg, hit), picks


def pick(cfg, base, files):
    """The cargo tests these changed files need, as exact names, or None for
    all of them. A test with no tags runs: it cannot say it is safe to skip."""
    listed = []

    def ids():
        if not listed:
            listed.append(set(cargo_tests()))
        return listed[0]

    rows, hit, picks = plan(cfg, files, base, None, ids)
    for f, tags, p in rows:
        own = [", ".join(sorted(tags))] if tags else []
        if p:
            own.append(f"its tests {', '.join(sorted(p))}" if len(p) < 4 else f"{len(p)} of its tests")
        print(f"  {f} -> {'; '.join(own) or '(nothing)'}")
    print(f"tags hit: {', '.join(sorted(hit))}")
    if ALL in hit:
        print("cargo tests: all")
        return None
    every = sorted(ids())
    chosen = [t for t in every if test_tags(cfg, t) is None or wanted(test_tags(cfg, t), hit) or picked_by(picks, t)]
    print(f"cargo tests: {len(chosen)} of {len(every)}")
    # one run by exact name: each test binary runs the names it has (lib names
    # carry a module path; integration names are bare)
    return sorted({t.split(":", 2)[-1] if t.startswith("test:") else t.split(":", 1)[1] for t in chosen})


def main():
    if sys.argv[1:] == ["guard"]:
        sys.exit(guard())
    sys.exit(__doc__)


if __name__ == "__main__":
    main()
