# play/ API (for the playfield page)

`crates/core/src/play/` joins the rules (`game`), physics and terrain into one match. Everything moves by inputs and ticks: feed the same inputs on every machine and `checksum()` agrees.

## Units (R1.700)

- **Cell**: one terrain column = one physics unit (`Fx::ONE`) = `terrain::CELL` (1024) terrain units. About one floor height.
- **Grid step**: a quarter cell, the physics snap grid. Every point a player gives is a `GridPt { x, y }` in grid steps, absolute (x from the terrain's left edge, y up from 0). `GridPt::to_fx()` gives the physics position, `GridPt::col()` the terrain column.
- One segment length (wood's longest) is 2 cells; royals stand 8 cells apart (4 segment lengths).
- The plot's size is the terrain's only: `Plot::buildable(level)` columns, `Plot::build_height(level)` above the plot's front ground. `game::tune::LEVELS` no longer has plot sizes.

## The match

```rust
let mut play = Play::new(Settings { seats, wind, seed })?;  // lobby
play.start()?;                                             // round 1
let refusals = play.step(&[(player, input), ...]);        // inputs this tick, then one tick
play.apply(player, input)?;                                // one input now (no tick)
play.tick();                                               // one tick, no inputs
play.checksum() -> u64                                     // whole-match fingerprint
```

State to draw: `play.rules` (the `game::Match`: phase, round, ticks_left, players, pairings, wind), `play.castles[p]` (geometry by ledger id), `play.siege` (fields while a siege runs), `play.last_result` (the last siege's outcome, for the replay).

## Input (one enum for every phase)

| Phase | Input |
| --- | --- |
| Upgrade | `Pick(slot)`, `Reroll` |
| Build | `PlaceSegment { piece, a, b }` (piece: `Piece::Segment(Material)` or `Piece::Rope`), `PlaceBuilding { kind, at }`, `Sell(ledger_id)`, `PlaceRoyal { index, at }`, `BuyXp`, `BuyShot` |
| Siege | `SelectAmmo(slot)` (0 = normal), `Swing`, `Release`, `Air(slot)` (Q/W/E/R = slots 0..3 of the shooter's `air`) |
| Any timed phase | `Ready` |

Each input returns `Result<(), Refusal>` (`game::Refusal`); the page words the refusal from data/.

Build rules checked: both ends in the buildable span and between the ground and the build height, length within the material's longest (`physics::TUNE`), no duplicate, cost and unlock through the ledger. Buildings and royals must stand on a floor (a non-rope segment more across than up) or the ground, within 1/8 cell.

## Build-phase queries

- `play.strain(p) -> Vec<(ledger_id, permille)>`: each segment's highest strain under its own weight, 0..=1000 (1000 = breaks). Runs a static test (about 180 physics ticks), so call it when the castle changes, not every frame.
- `play.castle_world(p) -> Option<physics::World>`: the castle with its royals and buildings, to draw or to step for the Test button.

## Siege

`play.siege: Option<Siege>` with `fields: Vec<Field>`, one per castle under attack:

- `owner`, `attacker`, `ghost`; `castle.world` (the physics world: segments, projectiles, debris, bodies), `castle.seg_ids[i]` (ledger id of physics segment i), `castle.bodies[i]` (`BodyOwner::Royal(index)` or `Building(id)`).
- `shooter`: `ammo: Vec<(Ammo, left)>` (slot order for keys 1–5; `Ammo::kind()` is the `physics::ShotKind`), `air` with `air_uses` (uses left this round per slot), `selected`, `swing_from`, `live` (projectiles of the last shot), `air_used` (one air effect per shot), `mods` (the shooter's `ShotMods`).
- `fuses` (exploding shots in the air), `lives` (shot hits each body still survives), `killed` (royal bodies in the order they died).
- `base`: where the trebuchet stands (left edge of the terrain's launch column, on the ground); it throws to the right.
- The field a player drives is the one where `attacker == p && !shooter.bot`. Their own castle is the field whose `owner == p`.

A ghost pairing (R1.205) makes two fields: the attacker against a copy of `copy_of`'s castle (`ghost: true`), and a stand-in bot trebuchet as `copy_of` against the attacker's real castle.

Bot trebuchets follow a plan: `Play::set_shot_planner(fn(&SiegeView) -> Vec<PlannedShot>)`; the default `siege::simple_planner` fires every shot at one arc. The aiming AI plugs in there.

When the siege timer ends (or everyone is ready), shots in the air finish (at most 4 s), then royals killed and buildings standing go to `Match::apply_siege` and the next round begins.
