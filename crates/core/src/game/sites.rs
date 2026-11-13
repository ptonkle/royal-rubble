//! Where buildings stand on the match terrain, and which map spot that puts
//! them on (R1.600). A building is on a spot when its footprint column lies in
//! the spot's columns and in the owner's buildable span.

use super::ledger::BuildingKind;
use super::tune;
use super::Refusal;
use crate::terrain::{Plot, SpotKind};

/// The spot a building at `col` stands on, or a refusal when `col` is
/// outside the buildable span at `level`.
pub fn site(terrain: &Plot, level: u8, col: usize) -> Result<Option<SpotKind>, Refusal> {
    if !terrain.buildable(level).contains(col) {
        return Err(Refusal::BadSpot);
    }
    Ok(terrain
        .spots
        .iter()
        .find(|s| s.cols.contains(col))
        .map(|s| s.kind))
}

/// A good column for `kind` at `level`: the first open spot that boosts it,
/// else any open spot with an effect for every building, else the plot's
/// front column. Used by bots.
pub fn best_column(terrain: &Plot, level: u8, kind: BuildingKind, taken: &[usize]) -> usize {
    let span = terrain.buildable(level);
    let open = |k: SpotKind| {
        terrain
            .spots
            .iter()
            .filter(|s| s.kind == k)
            .flat_map(|s| s.cols.start..s.cols.end)
            .find(|c| span.contains(*c) && !taken.contains(c))
    };
    let own = tune::SPOT_BOOSTS
        .iter()
        .filter(|(_, b)| *b == kind)
        .find_map(|(s, _)| open(*s));
    own.or_else(|| {
        [
            SpotKind::GoldVein,
            SpotKind::ArmoryRuin,
            SpotKind::HolyWell,
            SpotKind::Watchtower,
        ]
        .into_iter()
        .find_map(open)
    })
    .unwrap_or(span.start)
}

#[cfg(test)]
mod tests {
    use crate::game::flow::{CastleOutcome, Match, Phase, Seat, Settings, SiegeResult};
    use crate::game::ledger::{BuildingKind, Piece};
    use crate::game::Refusal;
    use crate::terrain::{ColRange, Spot, SpotKind};

    /// Two humans in round 1's build; player 0 at level 9 with 100 gold and
    /// one `kind` spot on columns front+2..front+7 of their plot.
    fn on_spot(kind: SpotKind, wind: bool) -> (Match, usize) {
        let mut m = Match::new(Settings {
            seats: vec![Seat::Human; 2],
            wind,
            seed: 11,
        })
        .unwrap();
        m.start().unwrap();
        let p = &mut m.players[0];
        p.purse.level = 9;
        p.purse.gold = 100;
        let front = p.terrain.plot.start;
        p.terrain.spots = vec![Spot {
            kind,
            cols: ColRange {
                start: front + 2,
                end: front + 7,
            },
            level: 1,
            segments: vec![],
        }];
        (m, front + 3)
    }

    /// End the build and report `survived` as standing on player 0's castle.
    fn siege(m: &mut Match, survived: Vec<u32>) {
        while m.phase == Phase::Build {
            m.tick();
        }
        let r = SiegeResult {
            castles: vec![CastleOutcome {
                owner: 0,
                attacker: 1,
                ghost: false,
                royals_killed: vec![],
                buildings_survived: survived,
            }],
        };
        m.apply_siege(&r).unwrap();
    }

    fn build(m: &mut Match, kind: BuildingKind, col: usize) -> u32 {
        m.place_at(0, Piece::Building(kind), col).unwrap()
    }

    #[test]
    fn spot_stone_deposit_doubles_a_quarry() {
        let (mut m, col) = on_spot(SpotKind::StoneDeposit, false);
        let id = build(&mut m, BuildingKind::Quarry, col);
        siege(&mut m, vec![id]);
        assert_eq!(
            m.players[0].ledger.stock.stone,
            crate::game::tune::START_STONE + 2
        );
    }

    #[test]
    fn spot_ore_deposit_doubles_a_mine() {
        let (mut m, col) = on_spot(SpotKind::OreDeposit, false);
        let id = build(&mut m, BuildingKind::Mine, col);
        siege(&mut m, vec![id]);
        assert_eq!(m.players[0].ledger.stock.metal, 2);
    }

    #[test]
    fn spot_forest_doubles_a_carpenter_but_not_off_the_spot() {
        let (mut m, col) = on_spot(SpotKind::Forest, false);
        let on = build(&mut m, BuildingKind::Carpenter, col);
        let off = build(&mut m, BuildingKind::Carpenter, col + 10);
        siege(&mut m, vec![on, off]);
        assert_eq!(
            m.players[0].ledger.stock.wood,
            crate::game::tune::START_WOOD + 6 + 3
        );
    }

    #[test]
    fn spot_gold_vein_pays_gold_per_surviving_round() {
        let (mut m, col) = on_spot(SpotKind::GoldVein, false);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        let gold = m.players[0].purse.gold;
        siege(&mut m, vec![id]);
        // round 2 opens with the upgrade phase: no income yet
        assert_eq!(m.players[0].purse.gold, gold + 2);
    }

    #[test]
    fn spot_holy_well_heals_the_owner() {
        let (mut m, col) = on_spot(SpotKind::HolyWell, false);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        m.players[0].health = 90;
        siege(&mut m, vec![id]);
        assert_eq!(m.players[0].health, 93);
    }

    #[test]
    fn spot_holy_well_heal_stops_at_full_health() {
        let (mut m, col) = on_spot(SpotKind::HolyWell, false);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        m.players[0].health = 99;
        siege(&mut m, vec![id]);
        assert_eq!(m.players[0].health, 100);
    }

    #[test]
    fn spot_armory_ruin_gives_a_shot() {
        let (mut m, col) = on_spot(SpotKind::ArmoryRuin, false);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        siege(&mut m, vec![id]);
        assert_eq!(m.players[0].armory_shots, 1);
    }

    #[test]
    fn spot_watchtower_shows_next_rounds_wind() {
        let (mut m, col) = on_spot(SpotKind::Watchtower, true);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        siege(&mut m, vec![id]);
        let seen = m.forecast(0).expect("the watchtower's holder sees the wind");
        assert_eq!(m.forecast(1), None);
        // round 2: upgrade, build, siege; the forecast was round 3's wind
        while m.phase != Phase::Build {
            m.tick();
        }
        siege(&mut m, vec![]);
        while m.phase != Phase::Build {
            m.tick();
        }
        assert_eq!(m.round, 3);
        assert_eq!(m.wind, seen);
    }

    #[test]
    fn spot_watchtower_shows_nothing_with_wind_off() {
        let (mut m, col) = on_spot(SpotKind::Watchtower, false);
        let id = build(&mut m, BuildingKind::Carpenter, col);
        siege(&mut m, vec![id]);
        assert!(m.players[0].sees_next_wind);
        assert_eq!(m.forecast(0), None);
    }

    #[test]
    fn spot_old_ruins_give_free_stone_worth_nothing() {
        let mut m = Match::new(Settings {
            seats: vec![Seat::Human; 2],
            wind: false,
            seed: 11,
        })
        .unwrap();
        let segs: usize = m.terrain.spots.iter().map(|s| s.segments.len()).sum();
        assert!(segs > 1, "every map has old ruins");
        m.start().unwrap();
        let p = &mut m.players[1];
        assert_eq!(p.ruin_pieces.len(), segs);
        let (a, b) = (p.ruin_pieces[0].0, p.ruin_pieces[1].0);
        let mut gold = 0;
        assert_eq!(p.ledger.sell(a, 1, &mut gold), Ok(0));
        assert_eq!(p.ledger.sell(b, 5, &mut gold), Ok(0));
    }

    #[test]
    fn spot_building_outside_the_buildable_span_is_refused() {
        let (mut m, _) = on_spot(SpotKind::Forest, false);
        m.players[0].purse.level = 2;
        let span = m.players[0].terrain.buildable(2);
        // a spot straddling the span's back edge: its outer columns are refused
        m.players[0].terrain.spots[0].cols = ColRange {
            start: span.end - 2,
            end: span.end + 3,
        };
        let piece = Piece::Building(BuildingKind::Carpenter);
        assert_eq!(m.place_at(0, piece, span.end), Err(Refusal::BadSpot));
        assert!(m.place_at(0, piece, span.end - 1).is_ok());
        let last = m.players[0].ledger.entries.last().unwrap();
        assert_eq!(last.spot, Some(SpotKind::Forest));
    }
}
