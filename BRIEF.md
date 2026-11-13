# Royal Rubble — brief

A medieval castle-building auto-battler in 2D profile view. Build a castle with Poly Bridge-style segments, then hurl shots from a trebuchet at a rival's castle. Inspired by Teamfight Tactics, Angry Birds and Crush the Castle (mechanics only; never its name or look). Folder stays `ctc/`.

Decided 2026-11-12 in a question session (log at the bottom).

## Match
- 2–8 players, TFT-style. Each round you are assigned one castle to attack; with an odd count, someone attacks a copy of a castle.
- Last one standing wins. Everyone starts with 100 health.
- Health is lost **only** when your royals are killed. Royal damage scales up by round so late rounds end the game.
- Target length: ~25–30 min with 8 players (about 15–18 rounds).
- Modes: solo vs AI bots (easy/normal/hard), online with friends (bots fill empty seats). No hot-seat.

## Round structure
1. **Upgrade** (20s): every other round, starting after the first siege (before round 2, 4, 6 …).
2. **Build** (60s). Your target for this round is shown at the start. You can scout any castle and see who is attacking you.
3. **Siege** (60s). Everyone fires at once. Ammo you don't use is wasted. A small corner window shows your own castle.
- A timer ends early once everyone is ready.
- After the siege: castles fully repair, royals revive, and only buildings that **survived** pay out.

## Building
- Segments join nodes that snap to a fine grid; any angle is allowed. Each material has a maximum length.
- **Real load and stress:** segments carry load and break when overstressed. The build phase shows strain colors and has a Test button. Knocking out a support drops whatever it held up.
- Materials: wood (1 gold), stone (3), metal (5). Wood is available from the start, stone unlocks at about level 4, metal at about level 7.
- **Ropes and cables:** tension-only, cheap. Shots pass through them.
- **Producer buildings** sit on floors: Carpenter gives wood, Quarry gives stone, Mine gives metal, each a set amount per surviving round. Resources stockpile forever and are spent like free segments.
- **Armory:** gives +1 shot next round if it survives.
- Selling: a full refund for anything placed this build phase; half value, rounded down, after that.
- No upkeep. The plot is a fixed width that grows (wider and taller) with level.

## Royals
- About 12, each placed once when they arrive. If one isn't placed when the timer runs out, they're placed at random. They stay where they were put but can be moved during build.
- Spacing: about 4 segment lengths. Show a circle while placing.
- Killed by a direct hit, heavy enough debris, or a fall of more than about 3 segment heights. They revive every round.
- Schedule (damage before round scaling): Squires R1, R2, R4, R7 (1 each); Knights R3, R9 (2); Princess R5, R12 (3); Prince R8, R15 (4); Queen R11 (6); King R13 (10).

## Siege and trebuchet
- The trebuchet isn't part of the castle. Tap Space to start the swing and tap again to release; release timing sets the arc. Show a faint ghost of your last shot's path.
- Light follow-cam that stays zoomed out enough to use air effects.
- Wind is the same for everyone, changes each round, and is shown on a flag. There's a lobby option to turn it off.
- No reload cooldown.
- Keys: 1–5 (or clicking the bottom bar) choose the ammo type before launch. Q/W/E/R trigger owned air effects. Only one air effect per shot.
- Start with 3 normal shots. More come from levels (normal shots), upgrades (special ammo every round), buying with gold during build (this round only), and the Armory.
- Fire ignites wood, which burns, weakens and spreads to touching wood. Stone and metal don't burn.

## Economy
- Income per round: 5 gold, plus 1 interest per 10 gold banked (max +5), plus kill gold (+1 per royal killed, +3 for the King). No streaks.
- XP: 4 gold buys 4 XP, plus 2 free XP each round. Levels 1–9. Levels give shot power, normal ammo, plot size, materials and buildings.

## Upgrades (augment-style)
- 3 cards with exact numbers ("+20% smash damage"). Tiers are silver, gold and prismatic; one tier is rolled per upgrade round for everyone. One free reroll, then 2 gold each.
- At least 50 in v1. Categories:
  - **Ammo:** exploding, fire, heavy, spread, …
  - **Air effects:** drop straight down, reverse direction, phase, split, dive, boost, …
  - **Defense:** tougher wood, tougher stone, …
  - **Economy:** lump gold, higher interest cap, gold-making placeables, material discounts.
  - **Extra attacks.**

## Map
- Seeded terrain, the same for everyone; each plot is a copy. New seed every match.
- Spots to defend:
  - stone and ore deposits (double output)
  - gold vein (gold per surviving round)
  - holy well (heals a little each surviving round)
  - high ground, cliffs and moats
  - forest (double wood)
  - old ruins (free stone walls)
  - watchtower hill (see next round's wind)
  - armory ruin (+1 shot)

## Tech
- Builder template, Rust core, **our own deterministic fixed-point physics** (segments, nodes, stress, rigid shots), so online matches play out identically everywhere and the AI can simulate shots. No floats or HashMap in the core; the page JS holds no numbers.
- Menus and lobby ported from **Fleeced and Ape Race**: Copy code and link, invite links, names, bots, version button and What's New, volume (low), settings in browser storage, tab icon.
- Look: make 2–3 mockups (painted storybook, illuminated manuscript, Bayeux tapestry, maybe woodcut) of the same castle scene. The person picks one before any UI. Not Builder's default style.
- Deploy: public `ptonkle/royal-rubble` on Pages once a vs-AI build is playable; ask "deploy?" first.

## First playable
1v1 vs AI with the full loop: build with stress, trebuchet siege, economy, royals, and about 20 upgrades. Then: online, 8 players, all 50+ upgrades.

## Question log (2026-11-12)
- Players: up to 8, TFT-style · Siege: everyone fires at once · Win: last one standing · Damage: royals killed only
- Structure: real stress · Grid: snapped nodes, any angle · Plot: grows with level · Placeables: producers, ropes
- Aim: tap Space twice · Wind (lobby toggle), light follow-cam, no cooldown · Ammo from levels, upgrades, gold, armory · Air effects: drop, reverse, phase, split/dive/boost
- Income: 5 + interest (streaks later dropped for kill gold) · Levels 1–9, 4g/4xp · Stockpile forever · No upkeep
- Anti-stall: royal damage scales by round · Royals placed for the game · ~12 royals · Spacing ~4 segments
- Map spots: all four plus my extras · Physics: own deterministic core
- Name: Royal Rubble (not "Crush the Castle": an existing Armor Games title) · Look: mock up several · Timing 60/60/20 · Modes: AI + online
- Upgrades: tiers + 1 free reroll · Ammo chosen before launch, Q/W/E/R effects, one per shot · Deploy when playable
- Scouting: target known at build start, scout any castle · Full refund same phase · Royal death: hit, crush or fall · Fire spreads through wood
- Keys 1–5 / QWER · ~25–30 min · First build 1v1 vs AI · Menus from Fleeced + Ape Race

## Later answers (2026-11-12)
- Pairings work like TFT: never the same opponent in consecutive rounds (TFT's recent-opponent rule); with an odd count, one player attacks a ghost copy of a castle.
- Eliminated players are removed entirely.
- After each siege, a short replay of the hits on your own castle.
- The upgrade list (50+) goes in the design journal for the person to review whenever they like; build from the draft meanwhile.

## Autonomy
- The person, 2026-11-12: "parallelize as much as you can from the outset." Subagents are approved, in worktrees, merged into main as they land.
- Deploy: build the first playable locally, then ask "deploy?" (public repo ptonkle/royal-rubble, Pages).
- Questions: inline in the chat, a few at a time, recommendation first (the person's rule in ~/.claude/game-lessons.md); record them in the journal afterwards.

## Journal
`../ctc-journal/`, its own local repository, never pushed, published to one artifact URL.
