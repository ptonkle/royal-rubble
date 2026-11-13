# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Air effect

A trick you trigger on a shot in flight, such as dropping it straight down. One per shot.

### Ammo

The shots you take into a siege: normal shots and any special ones. Ammo you don't throw is wasted.

### Armory ruin

A spot on the map. A building standing on it gives you an extra shot each round it survives.

### Forecast

Next round's wind, seen early by whoever holds the watchtower hill.

### Ghost

A copy of another player's castle, attacked when the player count is odd. Royals killed in a ghost cost its owner no health.

Not: clone, mirror

### Gold vein

A spot on the map. A building standing on it pays you gold each round it survives.

### Holy well

A spot on the map. A building standing on it heals you a little each round it survives.

### Interest

Extra gold each round for gold you have banked.

### Old ruins

A spot on the map with stone walls already standing. Everyone starts with them free; they sell for nothing.

### Royal

A noble you place in your castle. Each royal killed in a siege costs you health.

Not: VIP, unit

### Segment

One beam of a castle between two points on the build grid: wood, stone, metal or rope. Each material has a longest length.

Not: piece, block

### Spot

A place on the map worth defending, such as a deposit, forest or holy well. A building counts as on it when it stands inside both the spot and your plot.

Not: tile, node

### Stockpile

Wood, stone and metal you have banked. Building spends it before gold.

Not: inventory, storage

### Strain

How close a segment is to breaking under the castle's weight, shown as a color while you build.

Not: stress

### Upgrade

A card you pick in the upgrade phase. It lasts for the rest of the match.

Not: augment, perk
### Bot

A computer player in an open seat. Bots fill every seat no person takes, at Easy, Normal or Hard.

Not: AI, CPU, computer

### Host

The player who opened the room. The host picks the options and starts the match.

### Invite code

A long code to paste by hand when a room code can't connect. The joiner pastes it and sends back a reply code.

### Lobby

The room before a match: the players, the bots and the match options.

### Room code

The six letters that let friends join a room. Copy link sends the same code as a link.

Not: game code

### Wind

Air that pushes every shot the same way. It changes each round and can be turned off in the lobby.


### Watchtower hill

A spot on the map. While a building on it survives, you see the forecast.
