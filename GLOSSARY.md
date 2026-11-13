# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Ghost

A copy of another player's castle, attacked when the player count is odd. Royals killed in a ghost cost its owner no health.

Not: clone, mirror

### Interest

Extra gold each round for gold you have banked.

### Royal

A noble you place in your castle. Each royal killed in a siege costs you health.

Not: VIP, unit

### Stockpile

Wood, stone and metal you have banked. Building spends it before gold.

Not: inventory, storage

### Upgrade

A card you pick in the upgrade phase. It lasts for the rest of the match.

Not: augment, perk
