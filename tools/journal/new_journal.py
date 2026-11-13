#!/usr/bin/env python3
"""Start this project's design journal where BRIEF.md says it lives.

Copies tools/journal/index.html to the journal folder (by default
../<this folder>-journal/), puts the project's name in its title, makes img/
and an empty decisions.json (the questions, drawn by decisions.py),
and makes the folder its own git repository with a first commit, unless it
already sits inside one. The journal is never pushed: it holds draft pictures and the person's reference material,
which stay out of a public repository. It refuses to touch a folder that
already has an index.html.

    python3 tools/journal/new_journal.py ["Display name"]
"""

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from journal_check import setting  # noqa: E402


def main(title=None):
    kind, where = setting(ROOT, (ROOT / "BRIEF.md").read_text())
    if kind == "none":
        print("BRIEF.md's Journal section says none; no journal to start")
        return 1
    index = where / "index.html"
    if index.exists():
        print(f"{index} already exists; the journal is started")
        return 1
    title = title or ROOT.name.replace("-", " ").replace("_", " ").title()
    (where / "img").mkdir(parents=True, exist_ok=True)
    shell = (Path(__file__).parent / "index.html").read_text()
    index.write_text(shell.replace("{{TITLE}}", title))
    # questions for the person live here as data; tools/journal/decisions.py draws them as answer chips
    (where / "decisions.json").write_text('{\n  "prefix": "XX",\n  "questions": []\n}\n')
    (where / "README.md").write_text(
        f"# {title} journal\n\nThe design journal for `{ROOT.name}`: open `index.html`. "
        "Kept as its own repository and never pushed. Entries are added and corrected, never removed; "
        f"`{ROOT.name}/scripts/journal_check.py` checks its form.\n"
    )
    inside = shutil.which("git") and subprocess.run(
        ["git", "-C", str(where), "rev-parse", "--show-toplevel"], capture_output=True).returncode == 0
    if inside:
        print("the journal folder is inside another git repository; commit it there")
    elif shutil.which("git"):
        subprocess.run(["git", "init", "-q"], cwd=where, check=True)
        subprocess.run(["git", "add", "-A"], cwd=where, check=True)
        subprocess.run(["git", "commit", "-q", "-m", "Start the journal"], cwd=where, check=False)
    print(f"journal started at {where}; fill in its opening entry before writing PLAN.md")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else None))
