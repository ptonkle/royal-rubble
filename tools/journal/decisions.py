#!/usr/bin/env python3
"""The journal's questions, kept as data and drawn as answer chips.

A question for the person lives in the journal's decisions.json, not in the
page's HTML. This script draws each one as a card the person can answer with
one tap: a letter per option, the default marked, a box for a note. The page
saves answers in the person's browser, and, where the journal is published to
a host that gives the page a store (a claude.ai artifact's `db`), in that
store as well. Its "Copy answers" button gives one line to paste to any agent:

    XX-Q1: A; XX-Q2: B (a note); XX-Q3: -

    python3 tools/journal/decisions.py [JOURNAL]              draw the cards into index.html
    python3 tools/journal/decisions.py [JOURNAL] --check      exit 1 if the cards are out of date
    python3 tools/journal/decisions.py [JOURNAL] --answers "XX-Q1: A; XX-Q2: B"
    python3 tools/journal/decisions.py [JOURNAL] --answers-json answers.json
    python3 tools/journal/decisions.py [JOURNAL] --defaults   every unanswered question takes its default

JOURNAL defaults to ../<project>-journal/. --answers and --answers-json record
what came back in the journal's answers.json and print a table: the answer,
whether it is the default, and what to do with it (an agreed entry cites every
answer; an answer that differs from the default also becomes a TRIAGE.md row
citing the question). scripts/journal_check.py checks both.

decisions.json:

    {"prefix": "XX",
     "questions": [
       {"id": "XX-Q1", "page": "reading", "question": "...", "context": "...",
        "blocks": "what waits on the answer",
        "options": [{"key": "A", "text": "...", "default": true},
                    {"key": "B", "text": "..."}]}]}

Cards for a page go between <!--DECIDE page=NAME--> and <!--/DECIDE--> in
index.html. Edit decisions.json, run this script, never edit the cards by hand.
"""

import html
import json
import re
import sys
from pathlib import Path

MARK = re.compile(r"(<!--DECIDE page=([\w-]+)-->)(.*?)(<!--/DECIDE-->)", re.S)
ID = re.compile(r"^([A-Z]{2,4})-Q([1-9]\d*(?:\.\d+)?)$")
LINE = re.compile(r"([A-Z]{2,4}-Q[0-9.]+)\s*:\s*([A-Za-z-])\s*(?:\((.*?)\))?\s*(?:;|$)")


def problems_in(data):
    """Everything wrong with decisions.json, as sentences."""
    out = []
    prefix = data.get("prefix", "")
    seen = set()
    for q in data.get("questions", []):
        qid = q.get("id", "")
        m = ID.match(qid)
        if not m:
            out.append(f"question id {qid!r} is not of the form {prefix or 'XX'}-Qn")
        elif prefix and m.group(1) != prefix:
            out.append(f"{qid} does not use this project's prefix {prefix}")
        if qid in seen:
            out.append(f"{qid} appears twice")
        seen.add(qid)
        opts = q.get("options", [])
        if len(opts) < 2:
            out.append(f"{qid} has fewer than two options")
        defaults = [o for o in opts if o.get("default")]
        if len(defaults) != 1:
            out.append(f"{qid} has {len(defaults)} defaults; mark exactly one option as the default the agent builds on")
        keys = [o.get("key", "") for o in opts]
        if len(set(keys)) != len(keys) or not all(re.fullmatch(r"[A-Z]", k) for k in keys):
            out.append(f"{qid}: option keys must be distinct single capital letters")
        if not q.get("blocks", "").strip():
            out.append(f"{qid} has no 'blocks' line saying what waits on the answer")
        if not q.get("page"):
            out.append(f"{qid} names no page")
    return out


def card(q):
    e = html.escape
    qid = e(q["id"])
    parts = [f'<div class="decide" id="dec-{qid}"><p class="dq"><span class="qid">{qid}</span> {e(q["question"])}</p>']
    ctx = e(q.get("context", ""))
    parts.append(f'<p class="dx">{ctx} <span class="blocks">blocks: {e(q["blocks"])}</span></p>')
    for o in q["options"]:
        d = " data-default" if o.get("default") else ""
        tag = ' <em class="dflt">default</em>' if o.get("default") else ""
        parts.append(f'<button class="opt" type="button" data-q="{qid}" data-a="{e(o["key"])}"{d} aria-pressed="false">'
                     f'<b>{e(o["key"])}</b><span>{e(o["text"])}{tag}</span></button>')
    parts.append(f'<details class="note"><summary>Add a note</summary><textarea id="note-{qid}" data-note="{qid}" rows="2" '
                 f'placeholder="Anything else on this one"></textarea></details><p class="saved" data-saved="{qid}"></p></div>')
    return "".join(parts)


def render(page, data):
    by_page = {}
    for q in data.get("questions", []):
        by_page.setdefault(q["page"], []).append(card(q))

    def fill(m):
        cards = by_page.get(m.group(2), [])
        body = "\n" + "\n".join(cards) + "\n" if cards else "\n"
        return m.group(1) + body + m.group(4)
    return MARK.sub(fill, page)


def unplaced(page, data):
    pages = {m.group(2) for m in MARK.finditer(page)}
    return sorted({q["page"] for q in data.get("questions", [])} - pages)


def parse_line(text):
    found = {}
    for qid, key, note in LINE.findall(text):
        found[qid] = {"choice": "" if key == "-" else key.upper(), "note": (note or "").strip()}
    return found


def default_of(q):
    return next(o["key"] for o in q["options"] if o.get("default"))


def journal_dir(arg):
    if arg:
        return Path(arg).resolve()
    root = Path(__file__).resolve().parents[2]
    return root.parent / f"{root.name}-journal"


def main(argv):
    flags = [a for a in argv if a.startswith("--")]
    args = [a for a in argv if not a.startswith("--")]
    value = None
    if "--answers" in argv or "--answers-json" in argv:
        i = argv.index("--answers" if "--answers" in argv else "--answers-json")
        if i + 1 >= len(argv):
            print("give the answers after the flag", file=sys.stderr)
            return 2
        value = argv[i + 1]
        args = [a for a in args if a != value]
    jd = journal_dir(args[0] if args else None)
    dpath, index = jd / "decisions.json", jd / "index.html"
    if not dpath.exists():
        print(f"no decisions.json in {jd}")
        return 0 if "--check" in flags else 1
    data = json.loads(dpath.read_text())
    bad = problems_in(data)
    if bad:
        for b in bad:
            print(f"FAIL decisions.json: {b}")
        return 1
    page = index.read_text()
    missing = unplaced(page, data)
    if missing:
        print(f"FAIL decisions.json names page(s) {', '.join(missing)} with no <!--DECIDE page=...--> marker in index.html")
        return 1
    drawn = render(page, data)

    if "--check" in flags:
        if drawn != page:
            print("FAIL the answer chips in index.html are out of date with decisions.json: "
                  "run python3 tools/journal/decisions.py")
            return 1
        print(f"decisions: {len(data.get('questions', []))} questions, chips match decisions.json")
        return 0

    qs = {q["id"]: q for q in data.get("questions", [])}
    apath = jd / "answers.json"
    answers = json.loads(apath.read_text()) if apath.exists() else {}
    if "--answers" in flags:
        answers.update(parse_line(value))
    elif "--answers-json" in flags:
        raw = json.loads(Path(value).read_text())
        # an ArtifactData listing ([{"id", "data"}]) or a plain {id: {choice, note}}
        items = raw if isinstance(raw, dict) else {d["id"]: d.get("data", {}) for d in raw}
        for k, v in items.items():
            answers[k] = {"choice": v.get("choice", ""), "note": v.get("note", ""), "byDefault": bool(v.get("byDefault"))}
    elif "--defaults" in flags:
        for qid, q in qs.items():
            if not answers.get(qid, {}).get("choice"):
                answers[qid] = {**answers.get(qid, {}), "choice": default_of(q), "byDefault": True}
    else:
        if drawn != page:
            index.write_text(drawn)
            print(f"drew {len(qs)} question cards into {index}")
        else:
            print("chips already match decisions.json")
        return 0

    unknown = sorted(set(answers) - set(qs))
    if unknown:
        print(f"FAIL answers for questions decisions.json does not hold: {', '.join(unknown)}")
        return 1
    apath.write_text(json.dumps(answers, indent=2, sort_keys=True) + "\n")
    print(f"{'Question':<10} {'Answer':<7} {'Default':<8} Do")
    for qid, q in qs.items():
        a = answers.get(qid, {})
        choice = a.get("choice", "")
        if not choice:
            print(f"{qid:<10} {'-':<7} {default_of(q):<8} open: the default stands until answered")
            continue
        same = choice == default_of(q)
        do = "cite in the agreed entry" if same else "cite in the agreed entry; add a TRIAGE.md row citing " + qid
        note = f"  note: {a['note']}" if a.get("note") else ""
        print(f"{qid:<10} {choice:<7} {default_of(q):<8} {do}{note}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
