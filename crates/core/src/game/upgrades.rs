//! Upgrade cards: tiers, dealing, rerolls and effects. Adding an upgrade is
//! one row in UPGRADES; a new kind of effect is one Effect variant plus one
//! arm in `apply`. Names and card text live in data/, keyed by `key`.

use super::dice::Dice;
use super::economy::Purse;
use super::ledger::{Material, Stock};
use super::tune;
use super::Refusal;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Silver,
    Gold,
    Prismatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AmmoKind {
    Exploding,
    Fire,
    Heavy,
    Spread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AirKind {
    Drop,
    Reverse,
    Phase,
    Split,
    Dive,
    Boost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Gold now.
    Gold(u32),
    /// Stockpile units now.
    Stock(Material, u32),
    /// Raise the interest cap.
    InterestCap(u32),
    /// Gold off each segment of a material (a segment never costs below 1).
    Discount(Material, u32),
    /// Segment strength bonus in percent, read by physics.
    Strength(Material, u32),
    /// Extra normal shots every round.
    ShotsPerRound(u32),
    /// Special shots every round.
    Ammo(AmmoKind, u32),
    /// An air effect to trigger in flight.
    Air(AirKind),
}

#[derive(Clone, Copy, Debug)]
pub struct UpgradeRow {
    pub key: &'static str,
    pub tier: Tier,
    pub effect: Effect,
}

/// Placeholder rows until the full list lands (R1.208).
pub const UPGRADES: &[UpgradeRow] = &[
    UpgradeRow {
        key: "small_purse",
        tier: Tier::Silver,
        effect: Effect::Gold(6),
    },
    UpgradeRow {
        key: "woodpile",
        tier: Tier::Silver,
        effect: Effect::Stock(Material::Wood, 8),
    },
    UpgradeRow {
        key: "tough_timber",
        tier: Tier::Silver,
        effect: Effect::Strength(Material::Wood, 20),
    },
    UpgradeRow {
        key: "spare_stones",
        tier: Tier::Silver,
        effect: Effect::ShotsPerRound(1),
    },
    UpgradeRow {
        key: "fire_pots",
        tier: Tier::Silver,
        effect: Effect::Ammo(AmmoKind::Fire, 1),
    },
    UpgradeRow {
        key: "deep_vault",
        tier: Tier::Gold,
        effect: Effect::InterestCap(2),
    },
    UpgradeRow {
        key: "mason_guild",
        tier: Tier::Gold,
        effect: Effect::Discount(Material::Stone, 1),
    },
    UpgradeRow {
        key: "stone_skin",
        tier: Tier::Gold,
        effect: Effect::Strength(Material::Stone, 20),
    },
    UpgradeRow {
        key: "plummet",
        tier: Tier::Gold,
        effect: Effect::Air(AirKind::Drop),
    },
    UpgradeRow {
        key: "royal_treasury",
        tier: Tier::Prismatic,
        effect: Effect::Gold(20),
    },
    UpgradeRow {
        key: "powder_kegs",
        tier: Tier::Prismatic,
        effect: Effect::Ammo(AmmoKind::Exploding, 2),
    },
    UpgradeRow {
        key: "iron_walls",
        tier: Tier::Prismatic,
        effect: Effect::Strength(Material::Metal, 35),
    },
];

/// What a player's upgrades add up to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub discounts: Vec<(Material, u32)>,
    pub strength_pct: Vec<(Material, u32)>,
    pub extra_shots: u32,
    pub ammo: Vec<(AmmoKind, u32)>,
    pub air: Vec<AirKind>,
}

fn sum_for(list: &[(Material, u32)], m: Material) -> u32 {
    list.iter().filter(|(k, _)| *k == m).map(|(_, n)| *n).sum()
}

impl Modifiers {
    pub fn discount(&self, m: Material) -> u32 {
        sum_for(&self.discounts, m)
    }
    pub fn strength(&self, m: Material) -> u32 {
        sum_for(&self.strength_pct, m)
    }
}

/// Apply one upgrade's effect to a player.
pub fn apply(effect: Effect, purse: &mut Purse, stock: &mut Stock, mods: &mut Modifiers) {
    match effect {
        Effect::Gold(n) => purse.gold += n,
        Effect::Stock(m, n) => *stock.get_mut(m) += n,
        Effect::InterestCap(n) => purse.interest_cap_bonus += n,
        Effect::Discount(m, n) => mods.discounts.push((m, n)),
        Effect::Strength(m, n) => mods.strength_pct.push((m, n)),
        Effect::ShotsPerRound(n) => mods.extra_shots += n,
        Effect::Ammo(k, n) => mods.ammo.push((k, n)),
        Effect::Air(a) => mods.air.push(a),
    }
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

/// Deal up to CARDS_OFFERED distinct cards of `tier` the player does not own.
pub fn deal(tier: Tier, owned: &[usize], dice: &mut Dice) -> Vec<usize> {
    let mut pool: Vec<usize> = (0..UPGRADES.len())
        .filter(|i| UPGRADES[*i].tier == tier && !owned.contains(i))
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
    fn economy_upgrades_change_gold_interest_and_prices() {
        let mut purse = Purse::default();
        let mut stock = Stock::default();
        let mut mods = Modifiers::default();
        apply(Effect::Gold(6), &mut purse, &mut stock, &mut mods);
        apply(Effect::InterestCap(2), &mut purse, &mut stock, &mut mods);
        apply(
            Effect::Discount(Material::Stone, 1),
            &mut purse,
            &mut stock,
            &mut mods,
        );
        apply(
            Effect::Stock(Material::Wood, 8),
            &mut purse,
            &mut stock,
            &mut mods,
        );
        assert_eq!((purse.gold, purse.interest_cap(), stock.wood), (6, 7, 8));
        use super::super::ledger::{price, Piece};
        assert_eq!(price(Piece::Segment(Material::Stone), &mods), 2);
        assert_eq!(price(Piece::Segment(Material::Wood), &mods), 1);
    }

    #[test]
    fn defense_upgrades_add_up_per_material() {
        let mut mods = Modifiers::default();
        let (mut p, mut s) = (Purse::default(), Stock::default());
        apply(Effect::Strength(Material::Wood, 20), &mut p, &mut s, &mut mods);
        apply(Effect::Strength(Material::Wood, 15), &mut p, &mut s, &mut mods);
        assert_eq!(
            (mods.strength(Material::Wood), mods.strength(Material::Stone)),
            (35, 0)
        );
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
