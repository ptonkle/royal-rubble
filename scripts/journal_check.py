#!/usr/bin/env python3
"""Check that the design journal exists and can be read the way the procedure reads it.

The journal is where the person reads the agent's work: an HTML page of dated
entries, kept beside the project and never pushed. It is written for a person,
so this checks its form and its links to the repository's records, and nothing
about whether an entry is right:

- once BRIEF.md holds the person's description, the journal named in the
  brief's Journal section exists (`default` is ../<this folder>-journal/), unless
  that section says `none`;
- it holds an opening entry (data-kind="reading"), and the entry is no longer
  the template's blank one (no data-blank attribute). The opening entry is the
  agent reading Builder and the brief back to the person before it plans;
- every entry has an id and a kind, so it can be cited and its removal seen;
- every SECOND-ORDER.md row with status *person* is named in the journal, so
  an item the person must do reaches the page they read;
- every notebook row (R2.3) and triage row (T4) the journal cites exists, so
  the journal cannot report a record that is not there;
- no entry present in the last commit of the journal's repository is gone.
  Entries are history: they are added and corrected, never removed;
- decisions.json is well formed (one default per question, a blocks line)
  and the answer chips in the page match it (tools/journal/decisions.py);
- every answer in answers.json is cited by an *agreed* entry, and an answer
  that differs from the default is also cited by a TRIAGE.md row;
- once a *live* entry is dated on or after a *mockups* entry, an *against*
  entry (data-against="<mockups id>") sets that mockup beside the build;
- the Live now page's data-commit is the newest *live* entry's data-commit;
- a *research* entry links at least one source, and an *intake* entry quotes
  the input it reads (a blockquote);
- every project named under BRIEF.md's References is named in the opening entry.

On CI the journal is usually absent, since it is never pushed; a missing
journal is then reported and not failed. Everywhere else it fails.

    python3 scripts/journal_check.py
"""

import html
import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from notebook_check import cells, rows  # noqa: E402

ENTRY = re.compile(r"<article\b[^>]*>", re.I)
BLOCK = re.compile(r"(<article\b[^>]*>)(.*?)</article>", re.I | re.S)
DATE = re.compile(r'<time[^>]*datetime="(\d{4}-\d{2}-\d{2})"')
ATTR = re.compile(r'([\w-]+)(?:\s*=\s*"([^"]*)")?')
NOTE_ID = re.compile(r"(?<![\w.-])R([1-9]\d*\.[1-9]\d*)(?![\w]|\.\d)")
TRIAGE_ID = re.compile(r"(?<![\w.-])T([1-9]\d*)(?![\w])")


def section(text, heading):
    """The lines under '## heading' up to the next '## ' heading, or None."""
    out, inside = [], False
    for line in text.splitlines():
        if line.startswith("## "):
            if inside:
                break
            inside = line[3:].strip().lower().startswith(heading)
            continue
        if inside:
            out.append(line)
    return out if inside else None


def described(brief):
    """True once the brief's word-for-word section quotes the person."""
    for line in brief.splitlines():
        if line.startswith("## ") and "word for word" in line.lower():
            body = section(brief, line[3:].strip().lower()) or []
            return any(l.startswith(">") and l.lstrip("> ").strip() for l in body)
    return False


def setting(root, brief):
    """('none', None), or ('path', Path) for the journal the brief names."""
    body = section(brief, "journal")
    value = "default"
    if body:
        m = re.search(r"`([^`]+)`", "\n".join(body))
        if m:
            value = m.group(1).strip()
    if value.lower() == "none":
        return "none", None
    if value.lower() == "default":
        return "path", root.parent / f"{root.name}-journal"
    return "path", (root / value).resolve()


def entries(page):
    """Yield a dict of attributes for each <article class="entry">."""
    for m in ENTRY.finditer(page):
        attrs = {k.lower(): (v if v is not None else "") for k, v in ATTR.findall(m.group(0)[8:-1])}
        if "entry" in attrs.get("class", "").split():
            yield attrs


def text_of(page):
    page = re.sub(r"<(script|style)\b.*?</\1>", " ", page, flags=re.S | re.I)
    return html.unescape(re.sub(r"<[^>]+>", " ", page))


def blocks(page):
    """(attrs, inner html, date) for each entry, in page order."""
    out = []
    for m in BLOCK.finditer(page):
        attrs = {k.lower(): (v if v is not None else "") for k, v in ATTR.findall(m.group(1)[8:-1])}
        if "entry" in attrs.get("class", "").split():
            d = DATE.search(m.group(2))
            out.append((attrs, m.group(2), d.group(1) if d else ""))
    return out


def chips_problems(where, page):
    """decisions.json against the page, and answers.json against the entries and TRIAGE.md."""
    dpath = where / "decisions.json"
    if not dpath.exists():
        return [], {}, {}
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools" / "journal"))
    import decisions  # noqa: E402
    data = json.loads(dpath.read_text())
    out = [f"decisions.json: {b}" for b in decisions.problems_in(data)]
    if out:
        return out, {}, {}
    missing = decisions.unplaced(page, data)
    if missing:
        out.append(f"decisions.json puts questions on page(s) {', '.join(missing)}, which have no <!--DECIDE page=...--> marker")
    elif decisions.render(page, data) != page:
        out.append("the answer chips are out of date with decisions.json: run python3 tools/journal/decisions.py")
    apath = where / "answers.json"
    answers = json.loads(apath.read_text()) if apath.exists() else {}
    qs = {q["id"]: q for q in data.get("questions", [])}
    return out, qs, answers


def mockup_and_live_problems(found_blocks, page):
    out = []
    mocks = [(a, d) for a, _, d in found_blocks if a.get("data-kind") == "mockups"]
    lives = [(a, d) for a, _, d in found_blocks if a.get("data-kind") == "live"]
    against = set()
    for a, _, _ in found_blocks:
        if a.get("data-kind") == "against":
            against.update(a.get("data-against", "").split())
    for m, d in mocks:
        if any(ld >= d for _, ld in lives) and m.get("id") not in against:
            out.append(f"mockups entry {m.get('id')} was followed by a deploy with no 'against' entry setting it beside "
                       f'the build: add <article class="entry" data-kind="against" data-against="{m.get("id")}">')
    if lives:
        newest = max(lives, key=lambda x: x[1])[0]
        now = re.search(r'class="live-now"[^>]*data-commit="([^"]*)"', page) or re.search(r'data-commit="([^"]*)"[^>]*class="live-now"', page)
        if not newest.get("data-commit"):
            out.append(f"live entry {newest.get('id')} has no data-commit naming the deployed commit")
        elif not now:
            out.append("the journal has live entries and no Live now page (a .live-now block with data-commit)")
        elif now.group(1) != newest["data-commit"]:
            out.append(f"Live now says {now.group(1) or 'nothing'} is deployed, and the newest live entry "
                       f"({newest.get('id')}) says {newest['data-commit']}: rewrite Live now on every deploy")
    return out


def references(brief):
    """Names in bold under BRIEF.md's References section."""
    return [n.strip() for n in re.findall(r"^\s*[-*]\s+\*\*(.+?)\*\*", "\n".join(section(brief, "references") or []), re.M)]


def triage_numbers(root):
    path = root / "TRIAGE.md"
    found, header = set(), None
    if not path.exists():
        return found
    for line in path.read_text().splitlines():
        if not line.lstrip().startswith("|"):
            header = None
            continue
        c = cells(line)
        if header is None:
            header = c
            continue
        if set("".join(c)) <= set("-: ") or not any(c[1:]):
            continue
        m = re.fullmatch(r"T?([1-9]\d*)", c[0])
        if m:
            found.add(m.group(1))
    return found


def committed_ids(index):
    """Entry ids in the last commit of the repository holding the journal, or None."""
    def git(*args):
        return subprocess.run(["git", "-C", str(index.parent), *args], capture_output=True, text=True)
    top = git("rev-parse", "--show-toplevel")
    if top.returncode != 0:
        return None
    rel = index.resolve().relative_to(Path(top.stdout.strip()).resolve()).as_posix()
    old = git("show", f"HEAD:{rel}")
    if old.returncode != 0:
        return None
    return {e["id"] for e in entries(old.stdout) if e.get("id")}


def main(root):
    root = Path(root).resolve()
    brief = (root / "BRIEF.md").read_text()
    kind, where = setting(root, brief)
    if kind == "none":
        print("journal: turned off in BRIEF.md's Journal section")
        return 0
    if not described(brief):
        print("journal: not started yet (BRIEF.md has no description word for word)")
        return 0
    index = where / "index.html"
    if not index.exists():
        if os.environ.get("CI"):
            print(f"journal: {where.name}/ is not on this machine (CI); it is checked where it is kept")
            return 0
        print(f"FAIL the journal is missing: BRIEF.md names {where}, and it has no index.html. "
              "Start it with python3 tools/journal/new_journal.py, or write `none` under ## Journal in BRIEF.md")
        return 1

    page = index.read_text()
    found = list(entries(page))
    problems = []
    readings = [e for e in found if e.get("data-kind") == "reading"]
    if not readings:
        problems.append('the journal has no opening entry: add an <article class="entry" data-kind="reading"> '
                        "reading Builder and the brief back to the person before the plan")
    elif all("data-blank" in e for e in readings):
        problems.append("the opening entry is still the template's blank one (data-blank): fill it in, then remove data-blank")
    for e in found:
        if not e.get("id"):
            problems.append(f"an entry of kind {e.get('data-kind', '?')!r} has no id; give every entry an id so it can be cited")
        if not e.get("data-kind"):
            problems.append(f"entry {e.get('id', '?')} has no data-kind")

    text = text_of(page)
    notebook = root / "SECOND-ORDER.md"
    numbers = set()
    if notebook.exists():
        for number, _, status in rows(notebook.read_text()):
            numbers.add(number)
            if status == "person" and not re.search(rf"(?<![\w.]){re.escape(number)}(?![\w]|\.\d)", text):
                problems.append(f"SECOND-ORDER.md: {number} is the person's to do and the journal does not name it")
    for n in sorted(set(NOTE_ID.findall(text))):
        if f"R{n}" not in numbers:
            problems.append(f"the journal cites R{n}, and SECOND-ORDER.md has no such row")
    triage = triage_numbers(root)
    for n in sorted(set(TRIAGE_ID.findall(text)), key=int):
        if n not in triage:
            problems.append(f"the journal cites T{n}, and TRIAGE.md has no such row")

    found_blocks = blocks(page)
    chip_problems, qs, answers = chips_problems(where, page)
    problems += chip_problems
    agreed = " ".join(text_of(b) for a, b, _ in found_blocks if a.get("data-kind") == "agreed")
    triage_text = (root / "TRIAGE.md").read_text() if (root / "TRIAGE.md").exists() else ""
    for qid, a in sorted(answers.items()):
        choice = a.get("choice", "")
        if not choice or qid not in qs:
            continue
        if not re.search(rf"(?<![\w-]){re.escape(qid)}(?![\w.])", agreed):
            problems.append(f"{qid} was answered ({choice}) and no agreed entry cites it")
        default = next(o["key"] for o in qs[qid]["options"] if o.get("default"))
        if choice != default and not re.search(rf"(?<![\w-]){re.escape(qid)}(?![\w.])", triage_text):
            problems.append(f"{qid} was answered {choice}, not the default {default}, and no TRIAGE.md row cites it")
    problems += mockup_and_live_problems(found_blocks, page)
    for a, body, _ in found_blocks:
        if a.get("data-kind") == "research" and not re.search(r'href="https?://', body):
            problems.append(f"research entry {a.get('id')} links no source")
        if a.get("data-kind") == "intake" and "<blockquote" not in body:
            problems.append(f"intake entry {a.get('id')} does not quote the input it reads (a blockquote, word for word)")
    opening = " ".join(text_of(b) for a, b, _ in found_blocks if a.get("data-kind") == "reading").lower()
    for name in references(brief):
        if name.lower() not in opening:
            problems.append(f"BRIEF.md names {name} under References, and the opening entry does not say what it took from it")

    before = committed_ids(index)
    if before:
        now = {e["id"] for e in found if e.get("id")}
        for gone in sorted(before - now):
            problems.append(f"entry {gone} was in the journal's last commit and is gone: entries are corrected, never removed")

    for p in problems:
        print(f"FAIL {p}")
    if problems:
        return 1
    print(f"journal: {len(found)} entries at {where.name}/, an opening entry, "
          "person rows named, cited rows exist" + ("" if before is None else ", none removed since the last commit"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent))
