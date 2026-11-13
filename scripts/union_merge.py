#!/usr/bin/env python3
"""Resolve append-only merge conflicts by keeping both sides.

Agents add rows to the same shared files, so a merge conflicts only because
both sides appended. Run this during a conflicted merge; it refuses any file
outside the list, so a real code conflict is left for a person to read.

    python3 scripts/union_merge.py
"""

import json
import re
import subprocess
import tomllib

BLOCK = re.compile(r"^<<<<<<< [^\n]*\n(.*?)^=======\n(.*?)^>>>>>>> [^\n]*\n", re.S | re.M)
UNION = ("SECOND-ORDER.md", "HANDOFF.md", "LICENSES.md", "MEASUREMENTS.md", "GLOSSARY.md",
         "tests/tags.toml", "crates/core/src/lib.rs", "crates/shim/src/lib.rs")


def sorted_modules(text):
    """Both sides of a lib.rs conflict list modules: keep one of each, in order."""
    lines = text.split("\n")
    mods = sorted({line for line in lines if line.startswith("pub mod ")})
    first = next(i for i, line in enumerate(lines) if line.startswith("pub mod "))
    rest = [line for line in lines[first:] if not line.startswith("pub mod ")]
    return "\n".join(lines[:first] + mods + rest)


def drop_repeated_rows(text):
    """Two sides can carry the same row: keep its first copy, once."""
    seen = set()
    kept = []
    for line in text.split("\n"):
        if line.startswith("| R") and line in seen:
            continue
        seen.add(line)
        kept.append(line)
    return "\n".join(kept)


def split_rules(text):
    """Git shares one `[[rule]]` line between two appended rules: give each its own."""
    out, named = [], False
    for line in text.split("\n"):
        if line.startswith("["):
            named = False
        elif line.startswith("name = "):
            if named:
                out += ["", "[[rule]]"]
            named = True
        out.append(line)
    return "\n".join(out)


def main():
    names = subprocess.run(["git", "diff", "--name-only", "--diff-filter=U"],
                           capture_output=True, text=True, check=True).stdout.split()
    left = []
    for name in names:
        text = open(name).read()
        if name == "tests/breaks.json":
            text = BLOCK.sub(lambda m: m.group(1) + "  },\n  {\n" + m.group(2), text)
            json.loads(text)
        elif name in UNION:
            text = BLOCK.sub(lambda m: m.group(1) + m.group(2), text)
            if name.endswith("lib.rs"):
                text = sorted_modules(text)
            if name == "SECOND-ORDER.md":
                text = drop_repeated_rows(text)
            if name == "tests/tags.toml":
                text = split_rules(text)
                tomllib.loads(text)
        else:
            left.append(name)
            continue
        open(name, "w").write(text)
        subprocess.run(["git", "add", name], check=True)
        print("kept both sides:", name)
    for name in left:
        print("left for you:", name)
    return 1 if left else 0


if __name__ == "__main__":
    raise SystemExit(main())
