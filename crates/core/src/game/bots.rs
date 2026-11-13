//! Bot decisions for the economy phases: what to buy, build and pick. Where
//! segments go is the castle AI's job; this only spends the gold. The
//! match calls `SimpleBot` for bot seats; any `BotPolicy` can be run with
//! `Match::run_policy`.

use super::economy;
use super::flow::{Difficulty, Player};
use super::ledger::{self, BuildingKind, Material, Piece};
use super::sites;
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

/// Most segments a bot buys in one build.
const MAX_SEGMENTS: u32 = 12;

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
            Effect::Gold(_) | Effect::InterestCap(_) | Effect::Stock(..) | Effect::Discount(..) => {
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
            Effect::Air(_) => 1,
        }
    }

    fn best(offer: &Offer, round: u32) -> (usize, u32) {
        offer
            .cards
            .iter()
            .enumerate()
            .map(|(slot, i)| (slot, Self::score(UPGRADES[*i].effect, round)))
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
        let mut gold = me.purse.gold;
        let reserve = self.reserve();
        let level = me.purse.level;
        if round >= 3 && level < tune::MAX_LEVEL && gold >= reserve + tune::XP_BUY_COST + 4 {
            out.push(BotAction::BuyXp);
            gold -= tune::XP_BUY_COST;
        }
        for kind in [
            BuildingKind::Carpenter,
            BuildingKind::Quarry,
            BuildingKind::Armory,
            BuildingKind::Mine,
        ] {
            let piece = Piece::Building(kind);
            let cost = ledger::building_row(kind).cost;
            if level >= ledger::unlock_level(piece) && !me.ledger.has_building(kind) && gold >= reserve + cost
            {
                let taken: Vec<usize> = me.ledger.entries.iter().filter_map(|e| e.col).collect();
                let col = sites::best_column(&me.terrain, level, kind, &taken);
                out.push(BotAction::PlaceAt(piece, col));
                gold -= cost;
            }
        }
        let material = [Material::Metal, Material::Stone, Material::Wood]
            .into_iter()
            .find(|m| level >= ledger::unlock_level(Piece::Segment(*m)))
            .unwrap_or(Material::Wood);
        let piece = Piece::Segment(material);
        let price = ledger::price(piece, &me.mods);
        let mut stock = match material {
            Material::Wood => me.ledger.stock.wood,
            Material::Stone => me.ledger.stock.stone,
            Material::Metal => me.ledger.stock.metal,
        };
        let mut n = 0;
        while n < MAX_SEGMENTS && (stock > 0 || gold >= reserve + price) {
            if stock > 0 {
                stock -= 1;
            } else {
                gold -= price;
            }
            out.push(BotAction::Place(piece));
            n += 1;
        }
        let _ = economy::level_row(level);
        out.push(BotAction::Ready);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::flow::{Match, Seat, Settings};

    #[test]
    fn a_normal_bot_keeps_twenty_gold_banked() {
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
        assert!(!p.ledger.entries.is_empty());
    }
}
