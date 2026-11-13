//! Gold, interest, kill gold, XP and levels.

use super::royals::RoyalKind;
use super::tune::{self, LevelRow};
use super::Refusal;

/// Interest on banked gold: 1 per INTEREST_STEP, up to `cap`.
pub fn interest(banked: u32, cap: u32) -> u32 {
    interest_per(banked, tune::INTEREST_STEP, cap)
}

/// Interest at 1 per `step` gold banked, up to `cap`.
pub fn interest_per(banked: u32, step: u32, cap: u32) -> u32 {
    (banked / step.max(1)).min(cap)
}

/// Gold for the royals an attacker killed.
pub fn kill_gold(kills: &[RoyalKind]) -> u32 {
    kills
        .iter()
        .map(|k| {
            if *k == RoyalKind::King {
                tune::KING_KILL_GOLD
            } else {
                tune::KILL_GOLD
            }
        })
        .sum()
}

pub fn level_row(level: u8) -> &'static LevelRow {
    let i = (level.clamp(1, tune::MAX_LEVEL) - 1) as usize;
    &tune::LEVELS[i]
}

/// One player's gold, XP and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purse {
    pub gold: u32,
    pub level: u8,
    /// XP toward the next level.
    pub xp: u32,
    /// Extra interest cap from upgrades.
    pub interest_cap_bonus: u32,
    /// Gold banked per 1 interest (INTEREST_STEP unless an upgrade lowered it).
    pub interest_step: u32,
}

impl Default for Purse {
    fn default() -> Self {
        Purse {
            gold: tune::START_GOLD,
            level: 1,
            xp: 0,
            interest_cap_bonus: 0,
            interest_step: tune::INTEREST_STEP,
        }
    }
}

impl Purse {
    pub fn interest_cap(&self) -> u32 {
        tune::INTEREST_CAP + self.interest_cap_bonus
    }

    /// Round income: base plus interest on what is banked now. Returns it.
    pub fn collect_income(&mut self) -> u32 {
        let got = tune::BASE_INCOME + interest_per(self.gold, self.interest_step, self.interest_cap());
        self.gold += got;
        got
    }

    pub fn spend(&mut self, amount: u32) -> Result<(), Refusal> {
        if self.gold < amount {
            return Err(Refusal::NotEnoughGold);
        }
        self.gold -= amount;
        Ok(())
    }

    /// XP needed from the current level to the next; None at the top.
    pub fn xp_needed(&self) -> Option<u32> {
        if self.level >= tune::MAX_LEVEL {
            None
        } else {
            Some(tune::XP_TO_NEXT[(self.level - 1) as usize])
        }
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp += amount;
        while let Some(need) = self.xp_needed() {
            if self.xp < need {
                return;
            }
            self.xp -= need;
            self.level += 1;
        }
        self.xp = 0;
    }

    pub fn buy_xp(&mut self) -> Result<(), Refusal> {
        if self.level >= tune::MAX_LEVEL {
            return Err(Refusal::MaxLevel);
        }
        self.spend(tune::XP_BUY_COST)?;
        self.add_xp(tune::XP_BUY_AMOUNT);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_is_one_per_ten_and_stops_at_five() {
        assert_eq!(interest(9, 5), 0);
        assert_eq!(interest(10, 5), 1);
        assert_eq!(interest(39, 5), 3);
        assert_eq!(interest(50, 5), 5);
        assert_eq!(interest(90, 5), 5);
    }

    #[test]
    fn income_is_five_plus_interest_on_the_bank() {
        let mut p = Purse {
            gold: 23,
            ..Purse::default()
        };
        assert_eq!(p.collect_income(), 7);
        assert_eq!(p.gold, 30);
        let mut rich = Purse {
            gold: 80,
            interest_cap_bonus: 2,
            ..Purse::default()
        };
        assert_eq!(rich.collect_income(), 12);
    }

    #[test]
    fn kill_gold_is_one_per_royal_and_three_for_the_king() {
        assert_eq!(kill_gold(&[]), 0);
        assert_eq!(kill_gold(&[RoyalKind::Squire, RoyalKind::Queen]), 2);
        assert_eq!(kill_gold(&[RoyalKind::King, RoyalKind::Knight]), 4);
    }

    #[test]
    fn buying_xp_costs_four_and_levels_up_on_the_curve() {
        let mut p = Purse {
            gold: 10,
            ..Purse::default()
        };
        p.buy_xp().unwrap();
        // 4 XP: level 1 -> 2 takes 2, level 2 -> 3 takes 2.
        assert_eq!((p.gold, p.level, p.xp), (6, 3, 0));
        p.buy_xp().unwrap();
        assert_eq!((p.level, p.xp), (3, 4));
        assert_eq!(p.buy_xp(), Err(Refusal::NotEnoughGold));
    }

    #[test]
    fn xp_stops_at_level_nine() {
        let mut p = Purse {
            gold: 100,
            ..Purse::default()
        };
        p.add_xp(10_000);
        assert_eq!((p.level, p.xp), (9, 0));
        assert_eq!(p.buy_xp(), Err(Refusal::MaxLevel));
        assert_eq!(p.gold, 100);
    }
}
