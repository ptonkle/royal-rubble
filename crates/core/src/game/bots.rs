//! Bot decisions for the economy phases: upgrade picks and XP. Segments and
//! producers are the castle AI's (`ai::build::bot_turn`, R1.1451), which
//! places them where they go. The match calls `SimpleBot` for bot seats;
//! any `BotPolicy` can be run with `Match::run_policy`.

use super::flow::{Difficulty, Player};
use super::ledger::Piece;
use super::tune;
use super::upgrades::{Effect, Offer, UPGRADES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotAction {
    BuyXp,
    Place(Piece),
    /// Place a building at a terrain column.
    PlaceAt(Piece, usize),
    BuyShot,
    Reroll,
    Pick(usize),
    Ready,
}

pub trait BotPolicy {
    fn upgrade(&self, me: &Player, offer: &Offer, round: u32) -> Vec<BotAction>;
    fn build(&self, me: &Player, round: u32) -> Vec<BotAction>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimpleBot {
    pub difficulty: Difficulty,
}

impl SimpleBot {
    /// Gold the bot keeps banked for interest.
    fn reserve(&self) -> u32 {
        match self.difficulty {
            Difficulty::Easy => 0,
            Difficulty::Normal => 2 * tune::INTEREST_STEP,
            Difficulty::Hard => 3 * tune::INTEREST_STEP,
        }
    }

    fn score(effect: Effect, round: u32) -> u32 {
        let early = round <= 6;
        match effect {
            Effect::Gold(_)
            | Effect::InterestCap(_)
            | Effect::InterestStep(_)
            | Effect::GoldPerRound(_)
            | Effect::XpPerRound(_)
            | Effect::StockPerRound(..)
            | Effect::ProducerOutput(_)
            | Effect::Stock(..)
            | Effect::Discount(..) => {
                if early {
                    3
                } else {
                    1
                }
            }
            Effect::Strength(..) => {
                if early {
                    1
                } else {
                    3
                }
            }
            Effect::ShotsPerRound(_) | Effect::Ammo(..) => 2,
            _ => 1,
        }
    }

    fn best(offer: &Offer, round: u32) -> (usize, u32) {
        offer
            .cards
            .iter()
            .enumerate()
            .map(|(slot, i)| {
                let s = UPGRADES[*i].effects.iter().map(|e| Self::score(*e, round)).max();
                (slot, s.unwrap_or(0))
            })
            .max_by_key(|(slot, s)| (*s, usize::MAX - slot))
            .unwrap_or((0, 0))
    }
}

impl BotPolicy for SimpleBot {
    fn upgrade(&self, _me: &Player, offer: &Offer, round: u32) -> Vec<BotAction> {
        let mut out = Vec::new();
        // A hard bot spends its free reroll on a weak hand. After a reroll
        // the cards are unknown here, so it takes the first.
        if self.difficulty == Difficulty::Hard && offer.rerolls == 0 && Self::best(offer, round).1 < 2 {
            out.push(BotAction::Reroll);
            out.push(BotAction::Pick(0));
        } else {
            out.push(BotAction::Pick(Self::best(offer, round).0));
        }
        out.push(BotAction::Ready);
        out
    }

    fn build(&self, me: &Player, round: u32) -> Vec<BotAction> {
        let mut out = Vec::new();
        let gold = me.purse.gold;
        let reserve = self.reserve();
        let level = me.purse.level;
        if round >= 3 && level < tune::MAX_LEVEL && gold >= reserve + tune::XP_BUY_COST + 4 {
            out.push(BotAction::BuyXp);
        }
        out.push(BotAction::Ready);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::flow::{Match, Seat, Settings};

    #[test]
    fn a_normal_bot_keeps_twenty_gold_banked_and_is_ready() {
        let mut m = Match::new(Settings {
            seats: vec![Seat::Human; 2],
            wind: false,
            seed: 1,
        })
        .unwrap();
        m.start().unwrap();
        m.players[0].purse.gold = 26;
        let bot = SimpleBot {
            difficulty: Difficulty::Normal,
        };
        m.run_policy(0, &bot);
        let p = &m.players[0];
        assert!(p.purse.gold >= 20, "gold {}", p.purse.gold);
        assert!(p.ready);
    }
}
