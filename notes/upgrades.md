# Upgrades (draft)

69 upgrades for the first full version: 32 silver, 24 gold, 13 prismatic. 22 are marked for the first playable (11 silver, 6 gold, 5 prismatic). Machine-readable copy: `data/upgrades.draft.json`. The person reviews this whenever; the build uses it meanwhile.

| Category | Silver | Gold | Prismatic | Total |
| --- | --- | --- | --- | --- |
| Ammo | 4 | 3 | 2 | 9 |
| Tricks | 4 | 3 | 1 | 8 |
| Extra attacks | 4 | 3 | 2 | 9 |
| Defense | 4 | 3 | 2 | 9 |
| Economy | 4 | 3 | 2 | 9 |
| Buildings | 4 | 3 | 2 | 9 |
| Royals | 4 | 3 | 1 | 8 |
| Trebuchet | 4 | 3 | 1 | 8 |
| **All** | 32 | 24 | 13 | 69 |

## Design notes

- **Tier value.** Silver is about one round's income (~5 gold) of value or one extra shot a round; gold about twice that or a new trick; prismatic changes how you play (every shot burns, +3 shots, +30% all materials).
- **Deliberate tiered pairs** (same idea, bigger at a higher tier): Long Arm I / Long Arm II; Guild Charter I / Guild Charter II; Chainmail / Plate Armor; Decoy / Decoy Court; Firebrand / Greek Fire; Black Powder / Powder Keg; Spare Stones / Siege Train. Taking both stacks.
- **Tricks** take Q, W, E, R in the order you take them. Four at most: once you own four, air-effect cards stop being offered (Wild Hunt still is). One trick per shot, as the brief says.
- **Special ammo** arrives every round and is lost if unused, like all ammo. Repeats of the same card stack.
- **Buildings** granted by a card are placed like producers and pay only if they survive the siege.
- **Health** is still lost only from royal deaths; Chapel and Royal Pardon soften it, nothing else touches health.
- **First playable set** has at least 5 cards per tier, so three cards plus a reroll never run dry in a 1v1.
- **Not settled:** exact segment strengths behind every percent (they wait on the physics core), arrow and boulder sizes, whether Decoys count toward the 12 royals (draft: they don't), and the shot power each level gives.

## Ammo

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Firebrand | silver | Fire shots: +1 per round | yes |
| Black Powder | silver | Exploding shots: +1 per round | yes |
| Millstone | silver | Heavy shots: +1 per round | yes |
| Grapeshot | silver | Spread shots: +1 per round (5 small stones) | no |
| Chain Shot | gold | Chain shots: +2 per round (cut ropes and wood) | no |
| Greek Fire | gold | Fire shots: +2 per round. Fire spreads 50% faster | no |
| Powder Keg | gold | Exploding shots: +2 per round. +25% blast size | no |
| Dragon's Breath | prismatic | All your shots set wood on fire | yes |
| Meteor | prismatic | Meteor shots: +1 per round (huge, heavy, burning) | no |

## Tricks

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Plummet | silver | Trick: drop straight down. 2 uses per round | yes |
| Turnabout | silver | Trick: reverse direction. 2 uses per round | yes |
| Spur | silver | Trick: +50% speed. 2 uses per round | no |
| Kestrel | silver | Trick: hang still for 1 second. 2 uses per round | no |
| Ghost Stone | gold | Trick: pass through the next 2 segments. 2 uses per round | yes |
| Hawk Dive | gold | Trick: dive at double speed. 2 uses per round | no |
| Splinter | gold | Trick: split into 3. 2 uses per round | yes |
| Wild Hunt | prismatic | Tricks: +2 uses each per round | yes |

## Extra attacks

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Spare Stones | silver | Normal shots: +1 per round | yes |
| Quartermaster | silver | Bought shots cost 1 gold less | no |
| Lucky Stone | silver | 25% chance each round: +2 normal shots | no |
| Scavenger | silver | +1 normal shot per royal you killed last round (max 3) | no |
| Second Volley | gold | Your first shot each round fires twice | yes |
| Arrow Storm | gold | Once per round: 8 arrows at your aim point (royals only) | no |
| Vengeance | gold | +1 normal shot per royal you lost last round (max 3) | no |
| Siege Train | prismatic | Normal shots: +3 per round | no |
| Rain of Ruin | prismatic | Once per round: 3 boulders fall on your target's tallest tower | no |

## Defense

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Seasoned Oak | silver | +25% wood strength | yes |
| Dressed Stone | silver | +20% stone strength | no |
| Tempered Iron | silver | +20% metal strength | no |
| Damp Timber | silver | Your wood burns 50% slower | no |
| Mortar | gold | +30% joint strength | yes |
| Deep Footings | gold | +50% strength for segments on the ground | no |
| Bracing | gold | +100% rope strength. Ropes can't be cut | no |
| Citadel | prismatic | +30% strength for all materials | yes |
| Hallowed Walls | prismatic | The first 4 segments hit each round take no damage | no |

## Economy

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Purse | silver | +8 gold now | yes |
| Tithe | silver | +1 gold per round | yes |
| Scholar | silver | +2 free XP per round | no |
| Lumber Deal | silver | +6 wood per round | no |
| Strongbox | gold | Interest cap: +3 (max +8) | yes |
| Bounty | gold | Kill gold doubled | no |
| Mason's Discount | gold | Stone and metal cost 1 gold less | no |
| Royal Treasury | prismatic | +35 gold now | yes |
| Usury | prismatic | Interest: +1 per 5 gold banked (max +5) | no |

## Buildings

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Counting House | silver | Gain a Counting House: 2 gold per surviving round | no |
| Watchtower | silver | Gain a Watchtower: see next round's wind while it stands | no |
| Carpenter's Gift | silver | Gain a free Carpenter | no |
| Guild Charter I | silver | Producers: +1 output | yes |
| Guild Charter II | gold | Producers: +2 output | no |
| Arsenal | gold | Armories give +2 shots instead of +1 | yes |
| Chapel | gold | Gain a Chapel: +1 health per surviving round | no |
| Royal Mint | prismatic | Gain a Mint: 5 gold per surviving round | no |
| Cathedral | prismatic | Gain a Cathedral: royals within 4 segments survive one hit per round | no |

## Royals

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Chainmail | silver | Squires survive one hit per round | yes |
| Hardy Stock | silver | Royals survive falls of up to 5 segment heights | no |
| Decoy | silver | Gain a Decoy royal (no damage when killed) | no |
| Close Quarters | silver | Royal spacing: 3 segments instead of 4 | no |
| Plate Armor | gold | All royals survive one hit per round | no |
| Secret Passage | gold | Once per round, your first royal killed survives | no |
| Decoy Court | gold | Gain 3 Decoy royals | no |
| Royal Pardon | prismatic | Take 40% less damage from royal deaths | no |

## Trebuchet

| Name | Tier | Effect | First playable |
| --- | --- | --- | --- |
| Long Arm I | silver | +10% shot power | no |
| Steady Hand | silver | Shows the paths of your last 3 shots | no |
| Windward | silver | Wind moves your shots 50% less | no |
| Sighting Peg | silver | Shows the start of your arc while swinging | no |
| Long Arm II | gold | +20% shot power | no |
| Heavy Sling | gold | +25% weight for all your shots | no |
| Forward Camp | gold | Your trebuchet stands 6 segments closer | no |
| Siege Master | prismatic | +40% damage to all materials | yes |
