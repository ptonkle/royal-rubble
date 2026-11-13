//! The build ledger: what a player has placed, what it cost, stockpiles,
//! selling, producer yields and bought shots. Geometry lives in physics; an
//! entry id is how the two refer to one piece (R1.207).

use super::tune::{self, Yield};
use super::upgrades::Modifiers;
use super::Refusal;
use crate::terrain::SpotKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Material {
    Wood,
    Stone,
    Metal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildingKind {
    Carpenter,
    Quarry,
    Mine,
    Armory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Segment(Material),
    Rope,
    Building(BuildingKind),
}

/// Resources banked from producers and upgrades, spent before gold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stock {
    pub wood: u32,
    pub stone: u32,
    pub metal: u32,
}

impl Stock {
    pub fn get_mut(&mut self, m: Material) -> &mut u32 {
        match m {
            Material::Wood => &mut self.wood,
            Material::Stone => &mut self.stone,
            Material::Metal => &mut self.metal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub piece: Piece,
    /// Gold value when placed (after discounts), the base of a later sale.
    pub value: u32,
    pub paid_gold: u32,
    /// Set when a stockpile unit paid for it instead of gold.
    pub paid_stock: Option<Material>,
    pub round: u32,
    /// A building's footprint column on the terrain, once placed there.
    pub col: Option<usize>,
    /// The map spot a building stands on (R1.600).
    pub spot: Option<SpotKind>,
}

/// What surviving buildings give at the end of a siege.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Payout {
    /// Extra normal shots next round (armories and the armory ruin).
    pub shots: u32,
    /// Gold from the gold vein.
    pub gold: u32,
    /// Health from the holy well.
    pub heal: i32,
    /// A building on the watchtower hill stood.
    pub sees_wind: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    pub entries: Vec<Entry>,
    pub stock: Stock,
    pub shots_bought: u32,
    next_id: u32,
}

pub fn building_row(kind: BuildingKind) -> &'static tune::BuildingRow {
    tune::BUILDINGS
        .iter()
        .find(|b| b.kind == kind)
        .expect("every building has a row")
}

fn material_row(m: Material) -> (u32, u8) {
    tune::MATERIALS
        .iter()
        .find(|(k, _, _)| *k == m)
        .map(|(_, c, l)| (*c, *l))
        .expect("every material has a row")
}

/// Level a piece needs.
pub fn unlock_level(piece: Piece) -> u8 {
    match piece {
        Piece::Segment(m) => material_row(m).1,
        Piece::Rope => tune::ROPE_LEVEL,
        Piece::Building(b) => building_row(b).level,
    }
}

/// Gold price of a piece after upgrades; never below 1.
pub fn price(piece: Piece, mods: &Modifiers) -> u32 {
    match piece {
        Piece::Segment(m) => material_row(m).0.saturating_sub(mods.discount(m)).max(1),
        Piece::Rope => tune::ROPE_COST,
        Piece::Building(b) => building_row(b).cost,
    }
}

impl Ledger {
    /// Place a piece in `round`, paying from the stockpile first, then gold.
    /// Returns the entry id.
    pub fn place(
        &mut self,
        piece: Piece,
        round: u32,
        level: u8,
        gold: &mut u32,
        mods: &Modifiers,
    ) -> Result<u32, Refusal> {
        // a stockpiled material is free to use before its level (R1.3701)
        let stocked = matches!(piece, Piece::Segment(m) if *self.stock.get_mut(m) > 0);
        if level < unlock_level(piece) && !stocked {
            return Err(Refusal::Locked);
        }
        let value = price(piece, mods);
        let mut paid_stock = None;
        let mut paid_gold = 0;
        match piece {
            Piece::Segment(m) if *self.stock.get_mut(m) > 0 => {
                *self.stock.get_mut(m) -= 1;
                paid_stock = Some(m);
            }
            _ => {
                if *gold < value {
                    return Err(Refusal::NotEnoughGold);
                }
                *gold -= value;
                paid_gold = value;
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            piece,
            value,
            paid_gold,
            paid_stock,
            round,
            col: None,
            spot: None,
        });
        Ok(id)
    }

    /// Record where a placed building stands.
    pub fn set_site(&mut self, id: u32, col: usize, spot: Option<SpotKind>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.col = Some(col);
            e.spot = spot;
        }
    }

    /// Add a free piece worth nothing (the old ruins' stone). Returns its id.
    pub fn grant_free(&mut self, piece: Piece, round: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            piece,
            value: 0,
            paid_gold: 0,
            paid_stock: None,
            round,
            col: None,
            spot: None,
        });
        id
    }

    /// Sell an entry in `round`. Same round: everything paid comes back.
    /// Later: half the value in gold, rounded down. Returns gold refunded.
    pub fn sell(&mut self, id: u32, round: u32, gold: &mut u32) -> Result<u32, Refusal> {
        let i = self
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or(Refusal::Unknown)?;
        let e = self.entries.remove(i);
        if e.round == round {
            if let Some(m) = e.paid_stock {
                *self.stock.get_mut(m) += 1;
            }
            *gold += e.paid_gold;
            Ok(e.paid_gold)
        } else {
            let back = e.value * tune::SELL_LATER_NUM / tune::SELL_LATER_DEN;
            *gold += back;
            Ok(back)
        }
    }

    /// Buy one extra normal shot for this round's siege.
    pub fn buy_shot(&mut self, gold: &mut u32) -> Result<(), Refusal> {
        self.buy_shot_at(gold, tune::SHOT_COST)
    }

    /// Buy one shot for this round at `cost` gold.
    pub fn buy_shot_at(&mut self, gold: &mut u32, cost: u32) -> Result<(), Refusal> {
        if self.shots_bought >= tune::MAX_SHOTS_BOUGHT {
            return Err(Refusal::Limit);
        }
        if *gold < cost {
            return Err(Refusal::NotEnoughGold);
        }
        *gold -= cost;
        self.shots_bought += 1;
        Ok(())
    }

    /// Bank what the surviving producers make, with their spots' effects.
    /// Returns what goes outside the stockpile.
    pub fn collect_yields(&mut self, survived: &[u32]) -> Payout {
        self.collect_yields_with(survived, &Modifiers::default())
    }

    /// As `collect_yields`, with upgrades: producers make `producer_bonus`
    /// more (before a spot doubles it), and armories give `armory_shots`.
    pub fn collect_yields_with(&mut self, survived: &[u32], mods: &Modifiers) -> Payout {
        let mut out = Payout::default();
        for e in &self.entries {
            let Piece::Building(b) = e.piece else { continue };
            if !survived.contains(&e.id) {
                continue;
            }
            let boosted = e.spot.is_some_and(|s| tune::SPOT_BOOSTS.contains(&(s, b)));
            let mult = if boosted { tune::SPOT_YIELD_MULT } else { 1 };
            match building_row(b).yields {
                Yield::Stock(m, n) => *self.stock.get_mut(m) += (n + mods.producer_bonus) * mult,
                Yield::Shots(n) => out.shots += mods.armory_shots.unwrap_or(n) * mult,
            }
            match e.spot {
                Some(SpotKind::GoldVein) => out.gold += tune::GOLD_VEIN_GOLD,
                Some(SpotKind::HolyWell) => out.heal += tune::HOLY_WELL_HEAL,
                Some(SpotKind::ArmoryRuin) => out.shots += tune::ARMORY_RUIN_SHOTS,
                Some(SpotKind::Watchtower) => out.sees_wind = true,
                _ => {}
            }
        }
        out
    }

    pub fn has_building(&self, kind: BuildingKind) -> bool {
        self.entries.iter().any(|e| e.piece == Piece::Building(kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selling_in_the_same_phase_refunds_in_full() {
        let mut l = Ledger::default();
        let mut gold = 10;
        let id = l
            .place(
                Piece::Segment(Material::Stone),
                3,
                4,
                &mut gold,
                &Modifiers::default(),
            )
            .unwrap();
        assert_eq!(gold, 7);
        assert_eq!(l.sell(id, 3, &mut gold), Ok(3));
        assert_eq!(gold, 10);
        assert!(l.entries.is_empty());
    }

    #[test]
    fn selling_a_later_phase_refunds_half_rounded_down() {
        let mut l = Ledger::default();
        let mut gold = 10;
        let id = l
            .place(
                Piece::Segment(Material::Stone),
                3,
                4,
                &mut gold,
                &Modifiers::default(),
            )
            .unwrap();
        assert_eq!(l.sell(id, 4, &mut gold), Ok(1));
        assert_eq!(gold, 8);
    }

    #[test]
    fn the_stockpile_pays_before_gold_and_comes_back_on_a_same_phase_sale() {
        let mut l = Ledger::default();
        l.stock.wood = 1;
        let mut gold = 5;
        let m = Modifiers::default();
        let a = l
            .place(Piece::Segment(Material::Wood), 1, 1, &mut gold, &m)
            .unwrap();
        assert_eq!((gold, l.stock.wood), (5, 0));
        l.place(Piece::Segment(Material::Wood), 1, 1, &mut gold, &m)
            .unwrap();
        assert_eq!(gold, 4);
        l.sell(a, 1, &mut gold).unwrap();
        assert_eq!((gold, l.stock.wood), (4, 1));
    }

    #[test]
    fn stockpiled_stone_places_before_its_level_and_buys_none() {
        // R1.3701: the starting stone is usable at level 1; gold still can't buy it
        let mut l = Ledger::default();
        l.stock.stone = 1;
        let mut gold = 50;
        let m = Modifiers::default();
        assert!(l
            .place(Piece::Segment(Material::Stone), 1, 1, &mut gold, &m)
            .is_ok());
        assert_eq!((gold, l.stock.stone), (50, 0));
        assert_eq!(
            l.place(Piece::Segment(Material::Stone), 1, 1, &mut gold, &m),
            Err(Refusal::Locked)
        );
    }

    #[test]
    fn locked_materials_and_buildings_are_refused() {
        let mut l = Ledger::default();
        let mut gold = 50;
        let m = Modifiers::default();
        assert_eq!(
            l.place(Piece::Segment(Material::Stone), 1, 3, &mut gold, &m),
            Err(Refusal::Locked)
        );
        assert_eq!(
            l.place(Piece::Segment(Material::Metal), 1, 6, &mut gold, &m),
            Err(Refusal::Locked)
        );
        assert!(l
            .place(Piece::Segment(Material::Metal), 1, 7, &mut gold, &m)
            .is_ok());
        assert_eq!(
            l.place(Piece::Building(BuildingKind::Mine), 1, 6, &mut gold, &m),
            Err(Refusal::Locked)
        );
    }

    #[test]
    fn only_surviving_producers_pay_out() {
        let mut l = Ledger::default();
        let mut gold = 50;
        let m = Modifiers::default();
        let c = l
            .place(Piece::Building(BuildingKind::Carpenter), 1, 9, &mut gold, &m)
            .unwrap();
        let q = l
            .place(Piece::Building(BuildingKind::Quarry), 1, 9, &mut gold, &m)
            .unwrap();
        let a = l
            .place(Piece::Building(BuildingKind::Armory), 1, 9, &mut gold, &m)
            .unwrap();
        let pay = l.collect_yields(&[c, a]);
        assert_eq!((l.stock.wood, l.stock.stone, pay.shots), (3, 0, 1));
        let _ = q;
    }

    #[test]
    fn bought_shots_cost_gold_and_stop_at_the_limit() {
        let mut l = Ledger::default();
        let mut gold = 100;
        for _ in 0..tune::MAX_SHOTS_BOUGHT {
            l.buy_shot(&mut gold).unwrap();
        }
        assert_eq!(l.buy_shot(&mut gold), Err(Refusal::Limit));
        assert_eq!(gold, 100 - tune::MAX_SHOTS_BOUGHT * tune::SHOT_COST);
    }
}
