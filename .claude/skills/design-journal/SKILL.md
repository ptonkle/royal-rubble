---
name: design-journal
description: Keep this Builder project's design journal, the page where the person reads the build - start it, write its opening entry before the plan, add an entry for every decision put to the person, visible change, divergence and deploy, and keep it in step with SECOND-ORDER.md and TRIAGE.md. Use when starting a project, before writing PLAN.md, when asking the person a question, after any visible change, when the plan changes, after a triage round, and before a handoff.
---

# The design journal

The person reviews this project from its journal. Pictures you open yourself and decisions you make alone are invisible to them until they are in it.

1. **Start it.** `python3 tools/journal/new_journal.py` makes the folder `BRIEF.md`'s Journal section names (by default `../<this folder>-journal/`) as its own git repository. It is never pushed.
2. **Open with the reading entry, before `PLAN.md`.** Read Builder and the brief back to the person: what Builder asks of you in your own words, what this project changes, facts measured before any edit, numbered rules with what holds each one, the question prefix, where you stop, and what you took from each project `BRIEF.md` lists under References. Put each question in the journal's `decisions.json` with exactly one default and what it blocks, and run `python3 tools/journal/decisions.py` to draw it as answer chips; never ask a list of questions in chat. Remove `data-blank` when it is filled.
3. **Carry the answers forward.** When the person says the answers are in the journal, read them: on claude.ai from the artifact's `answers` collection, otherwise from the line "Copy answers" gives them to paste. Record them with `decisions.py --answers "XX-Q1: A; …"` (or `--defaults` for "go with the defaults"), write an *agreed* entry citing each, and add a `TRIAGE.md` row citing any that differs from its default. Each rule that applies to every later session goes under "Rules agreed for this project" in `CLAUDE.md` and `AGENTS.md`, with its journal ID; each rule the core can enforce becomes a test with a row in `tests/breaks.json`.
4. **Then one entry per event**, of the kinds in `tools/journal/ENTRIES.md`: the brief, research, intake, the plan and mockups, decisions as chips, the baseline, screenshots, divergences, against the mockup, ready for you, your fixes, live. Give each planning round its own page. On every deploy rewrite Live now with the deployed commit. Cite the rows each one reports.
5. **Never remove an entry.** Correct it with a new one or a note.
6. **Check and commit.** `python3 scripts/journal_check.py` (also a step of `./scripts/check.sh`), then commit in the journal's repository. Publish it to a private page if you can and the brief does not forbid it (on claude.ai, with the `db` capability, so the chips save to the page), republish after every entry, and give the person the link with a few plain lines on what is new and what still looks wrong.

The form comes from the design journals of Ape Race (Peter, ptonkle) and the games built under Builder after it.
