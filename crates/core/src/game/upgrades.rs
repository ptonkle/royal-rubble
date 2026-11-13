//! Upgrade cards: tiers, dealing, rerolls and effects. Adding an upgrade is
//! one row in UPGRADES; a new kind of effect is one Effect variant plus one
//! arm in `apply`. Names and card text live in data/upgrades.json, keyed by
//! `key`. A row with `dealable: false` is in the table but never offered,
//! because some layer does not act on its effect yet (R1.951).

use super::dice::Dice;
use super::economy::Purse;
use super::ledger::{Material, Stock};
use super::royals::RoyalKind;
use super::tune;
use super::Refusal;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Silver,
    Gold,
    Prismatic,
}

/// Special ammo. The play layer maps each to `physics::ShotKind` of the
/// same name, whose numbers are in `physics::ammo::SHOTS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AmmoKind {
    Exploding,
    Fire,
    Heavy,
    Spread,
    Chain,
    Meteor,
}

/// Air effects. The play layer maps each to `physics::AirEffect` of the
/// same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AirKind {
    Drop,
    Reverse,
    Phase,
    Split,
    Dive,
    Boost,
    Bounce,
}

/// Buildings a card gives for free (none is dealt yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GiftBuilding {
    CountingHouse,
    Watchtower,
    Carpenter,
    Chapel,
    Mint,
    Cathedral,
}

/// One effect of a card. Percents are bonuses unless the name says a cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Gold now.
    Gold(u32),
    /// Stockpile units now.
    Stock(Material, u32),
    GoldPerRound(u32),
    XpPerRound(u32),
    StockPerRound(Material, u32),
    /// Raise the interest cap.
    InterestCap(u32),
    /// Interest is 1 per this much gold banked instead of INTEREST_STEP.
    InterestStep(u32),
    /// Kill gold bonus in percent.
    KillGold(u32),
    /// Gold off each segment of a material (a segment never costs below 1).
    Discount(Material, u32),
    /// Segment strength bonus in percent, read by physics.
    Strength(Material, u32),
    JointStrength(u32),
    GroundedStrength(u32),
    RopeStrength(u32),
    RopesUncuttable,
    /// Your wood burns this much slower, percent.
    SlowBurn(u32),
    /// The first n segments hit each round take no damage.
    HitImmunity(u32),
    /// Extra normal shots every round.
    ShotsPerRound(u32),
    /// Gold off each bought shot (never below 1).
    ShotDiscount(u32),
    LuckyShots {
        chance_pct: u32,
        shots: u32,
    },
    ShotsPerKill {
        each: u32,
        max: u32,
    },
    ShotsPerLoss {
        each: u32,
        max: u32,
    },
    /// The first shot each round fires this many times.
    FirstShotCopies(u32),
    ArrowStorm {
        volleys: u32,
        arrows: u32,
    },
    RainOfRuin {
        volleys: u32,
        boulders: u32,
    },
    /// Special shots every round.
    Ammo(AmmoKind, u32),
    AllShotsIgnite,
    FireSpread(u32),
    /// Blast size bonus in percent.
    Blast(u32),
    /// An air effect and its uses per round; takes the next key slot.
    Air(AirKind, u32),
    /// Extra uses per round for every air effect.
    AirUses(u32),
    GainBuilding(GiftBuilding),
    ProducerOutput(u32),
    /// Shots each surviving armory gives, instead of its row's.
    ArmoryShots(u32),
    /// Hits a royal survives each round; None means every royal.
    RoyalSave(Option<RoyalKind>, u32),
    FallLimit(u32),
    Decoys(u32),
    RoyalSpacing(u32),
    /// Royal deaths spared each round, first killed first.
    FirstDeathSpared(u32),
    /// Damage from royal deaths cut by this percent.
    RoyalDamageCut(u32),
    ShotPower(u32),
    GhostPaths(u32),
    WindCut(u32),
    AimPreview(u32),
    ShotWeight(u32),
    ForwardCamp(u32),
    ShotDamage(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct UpgradeRow {
    pub key: &'static str,
    pub tier: Tier,
    pub effects: &'static [Effect],
    /// Offered in the upgrade phase. False while an effect is not built.
    pub dealable: bool,
}

/// Every upgrade, in data/upgrades.draft.json's order (notes/upgrades.md).
pub const UPGRADES: &[UpgradeRow] = &[
    UpgradeRow {
        key: "firebrand",
        tier: Tier::Silver,
        effects: &[Effect::Ammo(AmmoKind::Fire, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "black_powder",
        tier: Tier::Silver,
        effects: &[Effect::Ammo(AmmoKind::Exploding, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "millstone",
        tier: Tier::Silver,
        effects: &[Effect::Ammo(AmmoKind::Heavy, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "grapeshot",
        tier: Tier::Silver,
        effects: &[Effect::Ammo(AmmoKind::Spread, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "chain_shot",
        tier: Tier::Gold,
        effects: &[Effect::Ammo(AmmoKind::Chain, 2)],
        dealable: false,
    },
    UpgradeRow {
        key: "greek_fire",
        tier: Tier::Gold,
        effects: &[Effect::Ammo(AmmoKind::Fire, 2), Effect::FireSpread(50)],
        dealable: false,
    },
    UpgradeRow {
        key: "powder_keg",
        tier: Tier::Gold,
        effects: &[Effect::Ammo(AmmoKind::Exploding, 2), Effect::Blast(25)],
        dealable: true,
    },
    UpgradeRow {
        key: "dragons_breath",
        tier: Tier::Prismatic,
        effects: &[Effect::AllShotsIgnite],
        dealable: true,
    },
    UpgradeRow {
        key: "meteor",
        tier: Tier::Prismatic,
        effects: &[Effect::Ammo(AmmoKind::Meteor, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "plummet",
        tier: Tier::Silver,
        effects: &[Effect::Air(AirKind::Drop, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "turnabout",
        tier: Tier::Silver,
        effects: &[Effect::Air(AirKind::Reverse, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "spur",
        tier: Tier::Silver,
        effects: &[Effect::Air(AirKind::Boost, 2)],
        dealable: false,
    },
    UpgradeRow {
        key: "skipping_stone",
        tier: Tier::Silver,
        effects: &[Effect::Air(AirKind::Bounce, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "ghost_stone",
        tier: Tier::Gold,
        effects: &[Effect::Air(AirKind::Phase, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "hawk_dive",
        tier: Tier::Gold,
        effects: &[Effect::Air(AirKind::Dive, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "splinter",
        tier: Tier::Gold,
        effects: &[Effect::Air(AirKind::Split, 2)],
        dealable: true,
    },
    UpgradeRow {
        key: "wild_hunt",
        tier: Tier::Prismatic,
        effects: &[Effect::AirUses(2)],
        dealable: true,
    },
    UpgradeRow {
        key: "spare_stones",
        tier: Tier::Silver,
        effects: &[Effect::ShotsPerRound(1)],
        dealable: true,
    },
    UpgradeRow {
        key: "quartermaster",
        tier: Tier::Silver,
        effects: &[Effect::ShotDiscount(1)],
        dealable: true,
    },
    UpgradeRow {
        key: "lucky_stone",
        tier: Tier::Silver,
        effects: &[Effect::LuckyShots {
            chance_pct: 25,
            shots: 2,
        }],
        dealable: true,
    },
    UpgradeRow {
        key: "scavenger",
        tier: Tier::Silver,
        effects: &[Effect::ShotsPerKill { each: 1, max: 3 }],
        dealable: true,
    },
    UpgradeRow {
        key: "second_volley",
        tier: Tier::Gold,
        effects: &[Effect::FirstShotCopies(2)],
        dealable: true,
    },
    UpgradeRow {
        key: "arrow_storm",
        tier: Tier::Gold,
        effects: &[Effect::ArrowStorm {
            volleys: 1,
            arrows: 8,
        }],
        dealable: false,
    },
    UpgradeRow {
        key: "vengeance",
        tier: Tier::Gold,
        effects: &[Effect::ShotsPerLoss { each: 1, max: 3 }],
        dealable: true,
    },
    UpgradeRow {
        key: "siege_train",
        tier: Tier::Prismatic,
        effects: &[Effect::ShotsPerRound(3)],
        dealable: true,
    },
    UpgradeRow {
        key: "rain_of_ruin",
        tier: Tier::Prismatic,
        effects: &[Effect::RainOfRuin {
            volleys: 1,
            boulders: 3,
        }],
        dealable: false,
    },
    UpgradeRow {
        key: "seasoned_oak",
        tier: Tier::Silver,
        effects: &[Effect::Strength(Material::Wood, 25)],
        dealable: true,
    },
    UpgradeRow {
        key: "dressed_stone",
        tier: Tier::Silver,
        effects: &[Effect::Strength(Material::Stone, 20)],
        dealable: true,
    },
    UpgradeRow {
        key: "tempered_iron",
        tier: Tier::Silver,
        effects: &[Effect::Strength(Material::Metal, 20)],
        dealable: true,
    },
    UpgradeRow {
        key: "damp_timber",
        tier: Tier::Silver,
        effects: &[Effect::SlowBurn(50)],
        dealable: false,
    },
    UpgradeRow {
        key: "mortar",
        tier: Tier::Gold,
        effects: &[Effect::JointStrength(30)],
        dealable: false,
    },
    UpgradeRow {
        key: "deep_footings",
        tier: Tier::Gold,
        effects: &[Effect::GroundedStrength(50)],
        dealable: false,
    },
    UpgradeRow {
        key: "bracing",
        tier: Tier::Gold,
        effects: &[Effect::RopeStrength(100), Effect::RopesUncuttable],
        dealable: false,
    },
    UpgradeRow {
        key: "citadel",
        tier: Tier::Prismatic,
        effects: &[
            Effect::Strength(Material::Wood, 30),
            Effect::Strength(Material::Stone, 30),
            Effect::Strength(Material::Metal, 30),
        ],
        dealable: true,
    },
    UpgradeRow {
        key: "hallowed_walls",
        tier: Tier::Prismatic,
        effects: &[Effect::HitImmunity(4)],
        dealable: false,
    },
    UpgradeRow {
        key: "purse",
        tier: Tier::Silver,
        effects: &[Effect::Gold(8)],
        dealable: true,
    },
    UpgradeRow {
        key: "tithe",
        tier: Tier::Silver,
        effects: &[Effect::GoldPerRound(1)],
        dealable: true,
    },
    UpgradeRow {
        key: "scholar",
        tier: Tier::Silver,
        effects: &[Effect::XpPerRound(2)],
        dealable: true,
    },
    UpgradeRow {
        key: "lumber_deal",
        tier: Tier::Silver,
        effects: &[Effect::StockPerRound(Material::Wood, 6)],
        dealable: true,
    },
    UpgradeRow {
        key: "strongbox",
        tier: Tier::Gold,
        effects: &[Effect::InterestCap(3)],
        dealable: true,
    },
    UpgradeRow {
        key: "bounty",
        tier: Tier::Gold,
        effects: &[Effect::KillGold(100)],
        dealable: true,
    },
    UpgradeRow {
        key: "masons_discount",
        tier: Tier::Gold,
        effects: &[
            Effect::Discount(Material::Stone, 1),
            Effect::Discount(Material::Metal, 1),
        ],
        dealable: true,
    },
    UpgradeRow {
        key: "royal_treasury",
        tier: Tier::Prismatic,
        effects: &[Effect::Gold(35)],
        dealable: true,
    },
    UpgradeRow {
        key: "usury",
        tier: Tier::Prismatic,
        effects: &[Effect::InterestStep(5)],
        dealable: true,
    },
    UpgradeRow {
        key: "counting_house",
        tier: Tier::Silver,
        effects: &[Effect::GainBuilding(GiftBuilding::CountingHouse)],
        dealable: false,
    },
    UpgradeRow {
        key: "watchtower",
        tier: Tier::Silver,
        effects: &[Effect::GainBuilding(GiftBuilding::Watchtower)],
        dealable: false,
    },
    UpgradeRow {
        key: "carpenters_gift",
        tier: Tier::Silver,
        effects: &[Effect::GainBuilding(GiftBuilding::Carpenter)],
        dealable: false,
    },
    UpgradeRow {
        key: "guild_charter_1",
        tier: Tier::Silver,
        effects: &[Effect::ProducerOutput(1)],
        dealable: true,
    },
    UpgradeRow {
        key: "guild_charter_2",
        tier: Tier::Gold,
        effects: &[Effect::ProducerOutput(2)],
        dealable: true,
    },
    UpgradeRow {
        key: "arsenal",
        tier: Tier::Gold,
        effects: &[Effect::ArmoryShots(2)],
        dealable: true,
    },
    UpgradeRow {
        key: "chapel",
        tier: Tier::Gold,
        effects: &[Effect::GainBuilding(GiftBuilding::Chapel)],
        dealable: false,
    },
    UpgradeRow {
        key: "royal_mint",
        tier: Tier::Prismatic,
        effects: &[Effect::GainBuilding(GiftBuilding::Mint)],
        dealable: false,
    },
    UpgradeRow {
        key: "cathedral",
        tier: Tier::Prismatic,
        effects: &[Effect::GainBuilding(GiftBuilding::Cathedral)],
        dealable: false,
    },
    UpgradeRow {
        key: "chainmail",
        tier: Tier::Silver,
        effects: &[Effect::RoyalSave(Some(RoyalKind::Squire), 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "hardy_stock",
        tier: Tier::Silver,
        effects: &[Effect::FallLimit(5)],
        dealable: false,
    },
    UpgradeRow {
        key: "decoy",
        tier: Tier::Silver,
        effects: &[Effect::Decoys(1)],
        dealable: false,
    },
    UpgradeRow {
        key: "close_quarters",
        tier: Tier::Silver,
        effects: &[Effect::RoyalSpacing(3)],
        dealable: false,
    },
    UpgradeRow {
        key: "plate_armor",
        tier: Tier::Gold,
        effects: &[Effect::RoyalSave(None, 1)],
        dealable: true,
    },
    UpgradeRow {
        key: "secret_passage",
        tier: Tier::Gold,
        effects: &[Effect::FirstDeathSpared(1)],
        dealable: true,
    },
    UpgradeRow {
        key: "decoy_court",
        tier: Tier::Gold,
        effects: &[Effect::Decoys(3)],
        dealable: false,
    },
    UpgradeRow {
        key: "royal_pardon",
        tier: Tier::Prismatic,
        effects: &[Effect::RoyalDamageCut(40)],
        dealable: true,
    },
    UpgradeRow {
        key: "long_arm_1",
        tier: Tier::Silver,
        effects: &[Effect::ShotPower(10)],
        dealable: true,
    },
    UpgradeRow {
        key: "steady_hand",
        tier: Tier::Silver,
        effects: &[Effect::GhostPaths(3)],
        dealable: false,
    },
    UpgradeRow {
        key: "windward",
        tier: Tier::Silver,
        effects: &[Effect::WindCut(50)],
        dealable: false,
    },
    UpgradeRow {
        key: "sighting_peg",
        tier: Tier::Silver,
        effects: &[Effect::AimPreview(25)],
        dealable: false,
    },
    UpgradeRow {
        key: "long_arm_2",
        tier: Tier::Gold,
        effects: &[Effect::ShotPower(20)],
        dealable: true,
    },
    UpgradeRow {
        key: "heavy_sling",
        tier: Tier::Gold,
        effects: &[Effect::ShotWeight(25)],
        dealable: true,
    },
    UpgradeRow {
        key: "forward_camp",
        tier: Tier::Gold,
        effects: &[Effect::ForwardCamp(6)],
        dealable: false,
    },
    UpgradeRow {
        key: "siege_master",
        tier: Tier::Prismatic,
        effects: &[Effect::ShotDamage(40)],
        dealable: false,
    },
];

/// What a player's upgrades add up to. The match rules read the economy and
/// shot fields; the play layer reads the rest and hands them to physics:
/// `strength`/`rope_pct` to `World::strengthen`, `weight_pct`, `blast_pct`
/// and `all_ignite` to `physics::ShotMods`, `air_slots` to the Q/W/E/R keys,
/// `extra_lives` when a shot hits a royal, `first_shot_copies` on the first
/// shot of a siege.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub discounts: Vec<(Material, u32)>,
    pub strength_pct: Vec<(Material, u32)>,
    pub extra_shots: u32,
    pub ammo: Vec<(AmmoKind, u32)>,
    /// Air effects in key order (Q, W, E, R).
    pub air: Vec<AirKind>,
    /// Uses per round of each entry in `air`, before `air_bonus_uses`.
    pub air_uses: Vec<u32>,
    pub air_bonus_uses: u32,
    pub gold_per_round: u32,
    pub xp_per_round: u32,
    pub stock_per_round: Vec<(Material, u32)>,
    pub kill_gold_pct: u32,
    pub joint_pct: u32,
    pub grounded_pct: u32,
    pub rope_pct: u32,
    pub ropes_uncuttable: bool,
    pub slow_burn_pct: u32,
    pub hit_immunity: u32,
    pub shot_discount: u32,
    /// (chance percent, shots) rolled each round.
    pub lucky: Vec<(u32, u32)>,
    /// (shots each, most) per royal you killed last round.
    pub per_kill: Vec<(u32, u32)>,
    /// (shots each, most) per royal you lost last round.
    pub per_loss: Vec<(u32, u32)>,
    /// The first shot each round fires this many times (0 or 1: once).
    pub first_shot_copies: u32,
    pub arrow_storms: Vec<(u32, u32)>,
    pub rains_of_ruin: Vec<(u32, u32)>,
    pub all_ignite: bool,
    pub fire_spread_pct: u32,
    pub blast_pct: u32,
    pub gifts: Vec<GiftBuilding>,
    pub producer_bonus: u32,
    /// Shots each surviving armory gives, when an upgrade changed it.
    pub armory_shots: Option<u32>,
    pub royal_saves: Vec<(Option<RoyalKind>, u32)>,
    pub fall_limit: Option<u32>,
    pub decoys: u32,
    pub royal_spacing: Option<u32>,
    pub deaths_spared: u32,
    pub royal_damage_cut_pct: u32,
    pub power_pct: u32,
    pub ghost_paths: u32,
    pub wind_cut_pct: u32,
    pub aim_preview_pct: u32,
    pub weight_pct: u32,
    pub forward_camp: u32,
    pub damage_pct: u32,
}

fn sum_for(list: &[(Material, u32)], m: Material) -> u32 {
    list.iter().filter(|(k, _)| *k == m).map(|(_, n)| *n).sum()
}

fn per_count(list: &[(u32, u32)], count: u32) -> u32 {
    list.iter().map(|(each, max)| (each * count).min(*max)).sum()
}

impl Modifiers {
    pub fn discount(&self, m: Material) -> u32 {
        sum_for(&self.discounts, m)
    }
    pub fn strength(&self, m: Material) -> u32 {
        sum_for(&self.strength_pct, m)
    }
    pub fn stock_per_round(&self, m: Material) -> u32 {
        sum_for(&self.stock_per_round, m)
    }
    /// Hits a royal of `kind` survives each round.
    pub fn extra_lives(&self, kind: RoyalKind) -> u32 {
        self.royal_saves
            .iter()
            .filter(|(who, _)| who.is_none_or(|k| k == kind))
            .map(|(_, n)| *n)
            .sum()
    }
    /// Each owned air effect with its uses this round, in key order.
    pub fn air_slots(&self) -> Vec<(AirKind, u32)> {
        self.air
            .iter()
            .zip(&self.air_uses)
            .map(|(a, n)| (*a, n + self.air_bonus_uses))
            .collect()
    }
    /// Special shots of `kind` each round.
    pub fn ammo_of(&self, kind: AmmoKind) -> u32 {
        self.ammo
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, n)| *n)
            .sum()
    }
    /// Gold for one bought shot.
    pub fn shot_price(&self) -> u32 {
        tune::SHOT_COST
            .saturating_sub(self.shot_discount)
            .max(tune::MIN_SHOT_COST)
    }
    /// Kill gold after the bonus.
    pub fn kill_gold(&self, base: u32) -> u32 {
        base * (100 + self.kill_gold_pct) / 100
    }
    /// Health lost after the cut, rounded down.
    pub fn royal_damage(&self, base: u32) -> u32 {
        base * (100 - self.royal_damage_cut_pct.min(100)) / 100
    }
    /// Extra normal shots this round from last round's kills and losses.
    pub fn revenge_shots(&self, kills: u32, losses: u32) -> u32 {
        per_count(&self.per_kill, kills) + per_count(&self.per_loss, losses)
    }
    /// Extra normal shots from lucky rolls this round.
    pub fn lucky_shots(&self, dice: &mut Dice) -> u32 {
        self.lucky
            .iter()
            .map(|(chance, n)| if dice.below(100) < *chance { *n } else { 0 })
            .sum()
    }
}

/// Apply one effect to a player.
pub fn apply(effect: Effect, purse: &mut Purse, stock: &mut Stock, mods: &mut Modifiers) {
    match effect {
        Effect::Gold(n) => purse.gold += n,
        Effect::Stock(m, n) => *stock.get_mut(m) += n,
        Effect::GoldPerRound(n) => mods.gold_per_round += n,
        Effect::XpPerRound(n) => mods.xp_per_round += n,
        Effect::StockPerRound(m, n) => mods.stock_per_round.push((m, n)),
        Effect::InterestCap(n) => purse.interest_cap_bonus += n,
        Effect::InterestStep(n) => purse.interest_step = purse.interest_step.min(n.max(1)),
        Effect::KillGold(n) => mods.kill_gold_pct += n,
        Effect::Discount(m, n) => mods.discounts.push((m, n)),
        Effect::Strength(m, n) => mods.strength_pct.push((m, n)),
        Effect::JointStrength(n) => mods.joint_pct += n,
        Effect::GroundedStrength(n) => mods.grounded_pct += n,
        Effect::RopeStrength(n) => mods.rope_pct += n,
        Effect::RopesUncuttable => mods.ropes_uncuttable = true,
        Effect::SlowBurn(n) => mods.slow_burn_pct += n,
        Effect::HitImmunity(n) => mods.hit_immunity += n,
        Effect::ShotsPerRound(n) => mods.extra_shots += n,
        Effect::ShotDiscount(n) => mods.shot_discount += n,
        Effect::LuckyShots { chance_pct, shots } => mods.lucky.push((chance_pct, shots)),
        Effect::ShotsPerKill { each, max } => mods.per_kill.push((each, max)),
        Effect::ShotsPerLoss { each, max } => mods.per_loss.push((each, max)),
        Effect::FirstShotCopies(n) => mods.first_shot_copies = mods.first_shot_copies.max(n),
        Effect::ArrowStorm { volleys, arrows } => mods.arrow_storms.push((volleys, arrows)),
        Effect::RainOfRuin { volleys, boulders } => mods.rains_of_ruin.push((volleys, boulders)),
        Effect::Ammo(k, n) => mods.ammo.push((k, n)),
        Effect::AllShotsIgnite => mods.all_ignite = true,
        Effect::FireSpread(n) => mods.fire_spread_pct += n,
        Effect::Blast(n) => mods.blast_pct += n,
        Effect::Air(a, n) => {
            if mods.air.len() < tune::MAX_AIR_SLOTS {
                mods.air.push(a);
                mods.air_uses.push(n);
            }
        }
        Effect::AirUses(n) => mods.air_bonus_uses += n,
        Effect::GainBuilding(b) => mods.gifts.push(b),
        Effect::ProducerOutput(n) => mods.producer_bonus += n,
        Effect::ArmoryShots(n) => mods.armory_shots = Some(mods.armory_shots.unwrap_or(0).max(n)),
        Effect::RoyalSave(who, n) => mods.royal_saves.push((who, n)),
        Effect::FallLimit(n) => mods.fall_limit = Some(mods.fall_limit.unwrap_or(0).max(n)),
        Effect::Decoys(n) => mods.decoys += n,
        Effect::RoyalSpacing(n) => mods.royal_spacing = Some(mods.royal_spacing.map_or(n, |s| s.min(n))),
        Effect::FirstDeathSpared(n) => mods.deaths_spared += n,
        Effect::RoyalDamageCut(n) => mods.royal_damage_cut_pct += n,
        Effect::ShotPower(n) => mods.power_pct += n,
        Effect::GhostPaths(n) => mods.ghost_paths = mods.ghost_paths.max(n),
        Effect::WindCut(n) => mods.wind_cut_pct += n,
        Effect::AimPreview(n) => mods.aim_preview_pct = mods.aim_preview_pct.max(n),
        Effect::ShotWeight(n) => mods.weight_pct += n,
        Effect::ForwardCamp(n) => mods.forward_camp += n,
        Effect::ShotDamage(n) => mods.damage_pct += n,
    }
}

/// Apply every effect of UPGRADES[`index`].
pub fn apply_upgrade(index: usize, purse: &mut Purse, stock: &mut Stock, mods: &mut Modifiers) {
    for e in UPGRADES[index].effects {
        apply(*e, purse, stock, mods);
    }
}

/// The UPGRADES index of `key`.
pub fn index_of(key: &str) -> Option<usize> {
    UPGRADES.iter().position(|u| u.key == key)
}

fn gives_air(i: usize) -> bool {
    UPGRADES[i].effects.iter().any(|e| matches!(e, Effect::Air(..)))
}

/// The tier for the `index`-th upgrade round (0 first), the same for everyone.
pub fn roll_tier(index: usize, dice: &mut Dice) -> Tier {
    let (s, g, p) = tune::TIER_WEIGHTS[index.min(tune::TIER_WEIGHTS.len() - 1)];
    let r = dice.below(s + g + p);
    if r < s {
        Tier::Silver
    } else if r < s + g {
        Tier::Gold
    } else {
        Tier::Prismatic
    }
}

/// Cards on offer to one player in one upgrade phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub tier: Tier,
    /// Indexes into UPGRADES.
    pub cards: Vec<usize>,
    pub rerolls: u32,
    pub picked: Option<usize>,
}

/// Deal up to CARDS_OFFERED distinct dealable cards of `tier` the player
/// does not own. Once every air-effect key is taken, no card that gives an
/// air effect is dealt.
pub fn deal(tier: Tier, owned: &[usize], dice: &mut Dice) -> Vec<usize> {
    let air_full = owned.iter().filter(|i| gives_air(**i)).count() >= tune::MAX_AIR_SLOTS;
    let mut pool: Vec<usize> = (0..UPGRADES.len())
        .filter(|i| UPGRADES[*i].dealable && UPGRADES[*i].tier == tier && !owned.contains(i))
        .filter(|i| !(air_full && gives_air(*i)))
        .collect();
    dice.shuffle(&mut pool);
    pool.truncate(tune::CARDS_OFFERED);
    pool
}

/// Gold for the next reroll after `used` rerolls.
pub fn reroll_cost(used: u32) -> u32 {
    if used < tune::FREE_REROLLS {
        0
    } else {
        tune::REROLL_COST
    }
}

impl Offer {
    pub fn new(tier: Tier, owned: &[usize], dice: &mut Dice) -> Self {
        Offer {
            tier,
            cards: deal(tier, owned, dice),
            rerolls: 0,
            picked: None,
        }
    }

    pub fn reroll(&mut self, purse: &mut Purse, owned: &[usize], dice: &mut Dice) -> Result<(), Refusal> {
        if self.picked.is_some() {
            return Err(Refusal::AlreadyPicked);
        }
        purse.spend(reroll_cost(self.rerolls))?;
        self.rerolls += 1;
        self.cards = deal(self.tier, owned, dice);
        Ok(())
    }

    /// Take card `slot`; returns the UPGRADES index.
    pub fn pick(&mut self, slot: usize) -> Result<usize, Refusal> {
        if self.picked.is_some() {
            return Err(Refusal::AlreadyPicked);
        }
        let i = *self.cards.get(slot).ok_or(Refusal::Unknown)?;
        self.picked = Some(i);
        Ok(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take(keys: &[&str]) -> (Purse, Stock, Modifiers) {
        let (mut p, mut s, mut m) = (Purse::default(), Stock::default(), Modifiers::default());
        for k in keys {
            apply_upgrade(index_of(k).expect(k), &mut p, &mut s, &mut m);
        }
        (p, s, m)
    }

    #[test]
    fn the_first_reroll_is_free_then_each_costs_two() {
        let mut purse = Purse {
            gold: 3,
            ..Purse::default()
        };
        let mut d = Dice::new(1);
        let mut o = Offer::new(Tier::Silver, &[], &mut d);
        o.reroll(&mut purse, &[], &mut d).unwrap();
        assert_eq!(purse.gold, 3);
        o.reroll(&mut purse, &[], &mut d).unwrap();
        assert_eq!(purse.gold, 1);
        assert_eq!(o.reroll(&mut purse, &[], &mut d), Err(Refusal::NotEnoughGold));
    }

    #[test]
    fn three_distinct_cards_of_the_tier_and_none_already_owned() {
        let owned = vec![0];
        let cards = deal(Tier::Silver, &owned, &mut Dice::new(5));
        assert_eq!(cards.len(), 3);
        assert!(!cards.contains(&0));
        assert!(cards.iter().all(|i| UPGRADES[*i].tier == Tier::Silver));
        let mut sorted = cards.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn the_table_has_all_69_with_unique_keys() {
        assert_eq!(UPGRADES.len(), 69);
        for (i, u) in UPGRADES.iter().enumerate() {
            assert_eq!(index_of(u.key), Some(i), "{} twice", u.key);
            assert!(!u.effects.is_empty(), "{}", u.key);
        }
    }

    #[test]
    fn a_card_not_yet_built_is_never_dealt() {
        let mortar = index_of("mortar").unwrap();
        assert!(!UPGRADES[mortar].dealable);
        for seed in 0..300 {
            for tier in [Tier::Silver, Tier::Gold, Tier::Prismatic] {
                let cards = deal(tier, &[], &mut Dice::new(seed));
                assert!(cards.iter().all(|i| UPGRADES[*i].dealable));
            }
        }
    }

    #[test]
    fn every_tier_has_enough_dealable_cards_for_a_hand_and_a_reroll() {
        for tier in [Tier::Silver, Tier::Gold, Tier::Prismatic] {
            let n = UPGRADES.iter().filter(|u| u.dealable && u.tier == tier).count();
            assert!(n >= 5, "{tier:?} has {n}");
        }
    }

    #[test]
    fn air_effect_cards_stop_once_four_keys_are_taken() {
        let owned: Vec<usize> = ["plummet", "turnabout", "ghost_stone", "splinter"]
            .iter()
            .map(|k| index_of(k).unwrap())
            .collect();
        for seed in 0..200 {
            let cards = deal(Tier::Gold, &owned, &mut Dice::new(seed));
            assert!(!cards.contains(&index_of("hawk_dive").unwrap()));
        }
        let (_, _, m) = take(&["plummet", "splinter", "wild_hunt"]);
        assert_eq!(m.air_slots(), vec![(AirKind::Drop, 4), (AirKind::Split, 4)]);
    }

    #[test]
    fn economy_upgrades_pay_gold_now_and_raise_interest() {
        let (p, _, m) = take(&["purse", "royal_treasury", "tithe", "strongbox"]);
        assert_eq!(p.gold, 43);
        assert_eq!(p.interest_cap(), 8);
        assert_eq!(m.gold_per_round, 1);
        let (p, _, _) = take(&["usury"]);
        assert_eq!(p.interest_step, 5);
    }

    #[test]
    fn masons_discount_takes_a_gold_off_stone_and_metal() {
        use super::super::ledger::{price, Piece};
        let (_, _, m) = take(&["masons_discount"]);
        assert_eq!(price(Piece::Segment(Material::Stone), &m), 2);
        assert_eq!(price(Piece::Segment(Material::Metal), &m), 4);
        assert_eq!(price(Piece::Segment(Material::Wood), &m), 1);
    }

    #[test]
    fn defense_upgrades_add_up_per_material() {
        let (_, _, m) = take(&["seasoned_oak", "citadel"]);
        assert_eq!(
            (
                m.strength(Material::Wood),
                m.strength(Material::Stone),
                m.strength(Material::Metal)
            ),
            (55, 30, 30)
        );
    }

    #[test]
    fn ammo_upgrades_stack_per_kind() {
        let (_, _, m) = take(&["firebrand", "black_powder", "powder_keg", "dragons_breath"]);
        assert_eq!(
            (m.ammo_of(AmmoKind::Fire), m.ammo_of(AmmoKind::Exploding)),
            (1, 3)
        );
        assert_eq!(m.blast_pct, 25);
        assert!(m.all_ignite);
    }

    #[test]
    fn chainmail_saves_squires_and_plate_armor_everyone() {
        let (_, _, m) = take(&["chainmail"]);
        assert_eq!(
            (m.extra_lives(RoyalKind::Squire), m.extra_lives(RoyalKind::King)),
            (1, 0)
        );
        let (_, _, m) = take(&["chainmail", "plate_armor"]);
        assert_eq!(
            (m.extra_lives(RoyalKind::Squire), m.extra_lives(RoyalKind::King)),
            (2, 1)
        );
    }

    #[test]
    fn shot_cards_change_price_damage_and_revenge() {
        let (_, _, m) = take(&["quartermaster", "royal_pardon", "scavenger", "vengeance"]);
        assert_eq!(m.shot_price(), 1);
        assert_eq!(m.royal_damage(10), 6);
        assert_eq!(m.revenge_shots(5, 1), 4);
        let (_, _, m) = take(&["bounty", "second_volley"]);
        assert_eq!((m.kill_gold(3), m.first_shot_copies), (6, 2));
    }

    /// Numbers a card's text should show for one effect.
    fn shown(e: &Effect) -> Vec<u32> {
        match *e {
            Effect::Gold(n)
            | Effect::Stock(_, n)
            | Effect::GoldPerRound(n)
            | Effect::XpPerRound(n)
            | Effect::StockPerRound(_, n)
            | Effect::Discount(_, n)
            | Effect::Strength(_, n)
            | Effect::JointStrength(n)
            | Effect::GroundedStrength(n)
            | Effect::RopeStrength(n)
            | Effect::SlowBurn(n)
            | Effect::HitImmunity(n)
            | Effect::ShotsPerRound(n)
            | Effect::ShotDiscount(n)
            | Effect::Ammo(_, n)
            | Effect::FireSpread(n)
            | Effect::Blast(n)
            | Effect::Air(_, n)
            | Effect::AirUses(n)
            | Effect::ProducerOutput(n)
            | Effect::ArmoryShots(n)
            | Effect::FallLimit(n)
            | Effect::RoyalSpacing(n)
            | Effect::RoyalDamageCut(n)
            | Effect::ShotPower(n)
            | Effect::GhostPaths(n)
            | Effect::WindCut(n)
            | Effect::ShotWeight(n)
            | Effect::ForwardCamp(n)
            | Effect::ShotDamage(n) => vec![n],
            // "Gain a Decoy royal" for one
            Effect::Decoys(n) => [n].into_iter().filter(|n| *n > 1).collect(),
            Effect::InterestCap(n) => vec![n, tune::INTEREST_CAP + n],
            Effect::InterestStep(n) => vec![1, n, tune::INTEREST_CAP],
            Effect::LuckyShots { chance_pct, shots } => vec![chance_pct, shots],
            Effect::ShotsPerKill { each, max } | Effect::ShotsPerLoss { each, max } => vec![each, max],
            Effect::ArrowStorm { arrows, .. } => vec![arrows],
            Effect::RainOfRuin { boulders, .. } => vec![boulders],
            Effect::RoyalSave(_, _)
            | Effect::FirstDeathSpared(_)
            | Effect::AimPreview(_)
            | Effect::KillGold(_)
            | Effect::FirstShotCopies(_)
            | Effect::AllShotsIgnite
            | Effect::RopesUncuttable
            | Effect::GainBuilding(_) => vec![],
        }
    }

    /// Numbers in a card's text that come from elsewhere than its effects.
    fn shown_elsewhere(key: &str) -> Vec<String> {
        use crate::physics::{ammo::shot_tune, ShotKind, TUNE};
        let phase_tenths = TUNE.phase_ticks * 10 / crate::physics::tune::TICKS_PER_SECOND;
        match key {
            "grapeshot" => vec![shot_tune(ShotKind::Spread).pieces.to_string()],
            "ghost_stone" => vec![format!("{}.{}", phase_tenths / 10, phase_tenths % 10)],
            "close_quarters" => vec![tune::ROYAL_SPACING_SEGMENTS.to_string()],
            "arsenal" => vec![tune::BUILDINGS
                .iter()
                .find(|b| b.kind == super::super::ledger::BuildingKind::Armory)
                .map(|b| match b.yields {
                    tune::Yield::Shots(n) => n.to_string(),
                    tune::Yield::Stock(..) => String::new(),
                })
                .unwrap()],
            // Not built yet (never dealt): their numbers have no table to check.
            "spur" => vec!["50".into()],
            "splinter" => {
                let ball =
                    crate::physics::Ball::new(Default::default(), Default::default(), TUNE.grid, TUNE.grid);
                vec![ball.split(TUNE.split_spread).len().to_string()]
            }
            "counting_house" => vec!["2".into()],
            "chapel" => vec!["1".into()],
            "royal_mint" => vec!["5".into()],
            "cathedral" => vec!["4".into()],
            _ => vec![],
        }
    }

    fn numbers_in(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in text.chars().chain([' ']) {
            if c.is_ascii_digit() || (c == '.' && !cur.is_empty()) {
                cur.push(c);
            } else if !cur.is_empty() {
                out.push(cur.trim_end_matches('.').to_string());
                cur.clear();
            }
        }
        out
    }

    fn texts() -> serde_json::Map<String, serde_json::Value> {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../../../data/upgrades.json")).unwrap();
        v["upgrades"].as_object().unwrap().clone()
    }

    #[test]
    fn every_upgrade_has_a_name_and_text_and_no_text_is_orphaned() {
        let t = texts();
        for u in UPGRADES {
            let row = t.get(u.key).unwrap_or_else(|| panic!("{} has no text", u.key));
            assert!(row["name"].as_str().is_some_and(|s| !s.is_empty()), "{}", u.key);
            assert!(row["text"].as_str().is_some_and(|s| !s.is_empty()), "{}", u.key);
        }
        for k in t.keys() {
            assert!(index_of(k).is_some(), "text for unknown upgrade {k}");
        }
    }

    #[test]
    fn the_numbers_on_each_card_match_its_effects() {
        let t = texts();
        let mut wrong = Vec::new();
        for u in UPGRADES {
            let text = t[u.key]["text"].as_str().unwrap();
            let mut want: Vec<String> = u
                .effects
                .iter()
                .flat_map(shown)
                .map(|n| n.to_string())
                .chain(shown_elsewhere(u.key))
                .collect();
            let mut got = numbers_in(text);
            want.sort();
            want.dedup();
            got.sort();
            got.dedup();
            if got != want {
                wrong.push(format!("{}: {text} shows {got:?}, table {want:?}", u.key));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn guild_charter_and_arsenal_raise_building_yields() {
        use super::super::ledger::{BuildingKind, Ledger, Piece};
        let (_, _, m) = take(&["guild_charter_1", "guild_charter_2", "arsenal"]);
        let mut l = Ledger::default();
        let mut gold = 50;
        let c = l
            .place(Piece::Building(BuildingKind::Carpenter), 1, 9, &mut gold, &m)
            .unwrap();
        let a = l
            .place(Piece::Building(BuildingKind::Armory), 1, 9, &mut gold, &m)
            .unwrap();
        let pay = l.collect_yields_with(&[c, a], &m);
        // carpenter 3 wood + 3; armory 2 shots instead of 1
        assert_eq!((l.stock.wood, pay.shots), (6, 2));
    }

    #[test]
    fn every_tier_can_be_rolled() {
        let mut seen = Vec::new();
        for seed in 0..200 {
            let t = roll_tier(4, &mut Dice::new(seed));
            if !seen.contains(&t) {
                seen.push(t);
            }
        }
        assert_eq!(seen.len(), 3);
    }
}
