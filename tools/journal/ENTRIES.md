# Journal entries

The design journal is where the person reads the build. Each entry reports one thing that happened, cites the rows that record it, and says what is still the person's to judge. [`docs/journal.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/journal.md) says why the journal exists and what it cannot show.

## Rules

1. The journal starts with the opening entry (kind *reading*), written before `PLAN.md`. Every project has one unless `BRIEF.md`'s Journal section says `none`.
2. Every decision put to the person, every visible change, every divergence from the plan and every deploy gets an entry, written when it happens.
3. Entries are newest first on their page, and never removed. A correction is a new entry, or a note added to the old one ("replaced by …, T7").
4. An entry cites the rows it reports: `T4` for a triage row, `R2.3` for a notebook row, `XX-Q5` for a question. Every *person* row in `SECOND-ORDER.md` is named somewhere in the journal.
5. Pictures are of the running build, or drawn from the same numbers the code uses. A picture made separately from the build can disagree with it.
6. Each entry says what still looks wrong and what has not been done ("Nothing is pushed").
7. An entry written after the fact has the kind *retrospective* and names its sources.
8. Questions for the person are answer chips, never a list to answer in chat. Each one goes in the journal's `decisions.json` with exactly one default, and `python3 tools/journal/decisions.py` draws it. When the person says the answers are in the journal, read them (from the page's store where it has one, or from the line the person pastes from "Copy answers"), record them with `decisions.py --answers`, and write an *agreed* entry citing each. An answer that differs from the default also becomes a `TRIAGE.md` row citing the question. "Go with the defaults" is an answer: record it with `decisions.py --defaults`.
9. Each planning round gets its own page and a link in the nav, so the person can always find the round they are answering. The home page's card for that page says it holds open questions.
10. A *mockups* entry is followed, once the thing is built, by an *against* entry that sets each mockup beside a screenshot of the build at the same view, before the next deploy.
11. On every deploy, rewrite the Live now page and set its `data-commit` to the deployed commit, and add a *live* entry with the same `data-commit`.
12. A request with more than a few points, or a pasted transcript, chat or a co-designer's notes, gets an *intake* entry and a stop before any code. A small fix goes straight to `TRIAGE.md`.
13. When the journal has a published address, republish it after every entry. The person reads the published page, and a stale one ("it still says the town is not on main") costs a round of asking.

`scripts/journal_check.py` checks the form of rules 1, 3, 4, 8, 10 and 11, and that a *research* entry links a source and an *intake* entry quotes its input. Whether an entry is right is the person's reading.

## Kinds

The kind goes in the stamp and in `data-kind`. The label is the AI-literacy step the entry asks the reader to take, the same labels `docs/procedure.md` gives the stages.

| Kind | Stage | Label | Holds |
| --- | --- | --- | --- |
| reading | 0 | B1, C3 | what Builder asks of the agent, in its words; what the project adds or changes; facts measured before any edit; numbered rules, each with what holds it; the ID prefix; where the agent stops; questions, each with a recommendation and what it blocks |
| agreed | 0 | C5 | each question, the person's answer, and what it changes; which rules moved into `CLAUDE.md` and which became tests |
| brief | 1 | C1, A1 | the description word for word; a table from each requirement to where it lands, with question numbers |
| research | 1–2 | B1, C3 | before a design that leans on other work (a genre, a game, a paper, an interface): each source with a link, what it shows, and what this project takes from it and leaves |
| intake | 1, 9 | C1, A1 | a large input (a transcript, a co-designer's notes, a long request) word for word in a blockquote, then one row per point: how the agent reads it, and the T row or `XX-Q` it becomes; mockups where a point changes the look; then a stop |
| plan | 2 | B2 | the build order; diagrams drawn from the numbers the code will use; files touched and why |
| mockups | 2 | B2 | what the tool will look like, drawn before it is built |
| for you | 3 | A0, B2 | each decision as `XX-Qn` answer chips drawn from `decisions.json`: the default the agent builds on, the alternatives, and what waits on the answer (`blocks:`). An answer that differs becomes a TRIAGE row citing the question |
| measured | 4 | B2 | the baseline check, step by step |
| screenshots | 5 | C4 | pictures of the running build with one-line captions, and the weak spots |
| divergences | 6 | A3, B3 | the plan, what happened, what changed, citing notebook rows |
| against | 6–7 | C4, A3 | each mockup (`data-against` names the *mockups* entry) beside a screenshot of the build at the same view; one row per difference: kept, fixed (its T row), or dropped (why) |
| ready for you | 7 | C4 | how to open the build, what to try first, rough edges, checks with counts |
| your fixes | 7 | C4, B4 | the triage row, what the person said, what changed, with pictures |
| live | 8 | C4 | where it was published, the commit (`data-commit`), the check against the live address, what is left and whose it is; the Live now page is rewritten to match |
| review | any | A1–A6 | the person or a second model checks an entry against the build and says what it could not verify |
| retrospective | any | C5 | an entry written after the fact, with its sources |

## Markup

```html
<article class="entry" id="2026-10-19-plan-route" data-kind="plan">
  <div class="stamp"><time datetime="2026-10-19">2026-10-19</time><span>plan</span></div>
  <div class="entry-body">
    <h3>A plain sentence about what happened</h3>
    <p>The lede: what changed, what still looks wrong.</p>
    <div class="grid">
      <figure><button aria-label="Open full-window"><img src="img/route/plan.svg" alt="The route over the floor plan"></button>
        <figcaption><b>The route.</b> Drawn from the numbers in layout.rs.</figcaption></figure>
    </div>
  </div>
</article>
```

A *for you* entry's questions are data. In the journal's `decisions.json`:

```json
{"prefix": "XX", "questions": [
  {"id": "XX-Q3", "page": "round2", "question": "The question.",
   "context": "Why it matters, in a sentence.", "blocks": "what waits on the answer",
   "options": [{"key": "A", "text": "What I build on, and why", "default": true},
               {"key": "B", "text": "The other choice, and what it costs"}]}]}
```

and in the entry, where the chips go:

```html
<div class="decide-bar"><button type="button" class="act" data-defaults>Go with the defaults for the rest</button><button type="button" class="act" data-copy>Copy answers</button><span class="where" data-where></span></div>
<textarea class="copyout" data-copyout rows="2" readonly hidden aria-label="Your answers as text"></textarea>
<div class="decisions"><!--DECIDE page=round2-->
<!--/DECIDE--></div>
```

Then run `python3 tools/journal/decisions.py`. The page saves answers in the person's browser. Where the journal is published as a claude.ai artifact with the `db` capability, they are also saved to the artifact's `answers` collection, and the agent reads them from there.

An *against* entry and a *live* entry carry their links as attributes:

```html
<article class="entry" id="2026-11-02-against-lobby" data-kind="against" data-against="2026-10-30-mockups-lobby">
<article class="entry" id="2026-11-02-live" data-kind="live" data-commit="3f2a9c1">
```

Give each entry an `id` that will not change (the date and a word or two), add its row to the home page's timeline, and update the page's card. For a decision that waits on the person, use `<span class="you">for you</span>` in the stamp.

## Pictures

Capture the running build headless (the browser check's Playwright works) at the size the person will view it, frame each shot on what changed, look at every shot before using it, and retake any that reads badly. Save them under the journal's `img/<page>/<topic>/`. A game with its own camera tooling uses it; `kits/canvas/` has a tile script. Never open a visible window on the person's screen.

## Publishing

The journal is a static page that works opened from disk. An agent that can publish it to a private page (a claude.ai artifact, for example) does so and keeps one address for its life, unless the brief says the content is confidential, and republishes after every entry. On claude.ai, publish with the `db` capability so the answer chips save to the page. Commit in the journal's own repository after every change.
