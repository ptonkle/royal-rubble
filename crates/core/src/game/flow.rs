//! The match state machine: lobby, then rounds of Upgrade (some rounds),
//! Build and Siege, then results. Timers count ticks; a phase also ends once
//! every alive player is ready. The siege's outcome comes in through
//! `apply_siege` as a `SiegeResult`.

use super::bots::{BotAction, BotPolicy, SimpleBot};
use super::dice::Dice;
use super::economy::{self, Purse};
use super::ledger::{Ledger, Material, Piece};
use super::pairing::{self, History, Pairing};
use super::royals::{self, Plot, Royal, RoyalKind, Spot};
use super::tune;
use super::upgrades::{self, AirKind, AmmoKind, Modifiers, Offer, Tier, UPGRADES};
use super::{sites, PlayerId, Refusal};
use crate::terrain::{self, Segment};

// Salts that keep each kind of roll on its own stream.
const SALT_PAIRS: u32 = 1;
const SALT_TIER: u32 = 2;
const SALT_WIND: u32 = 3;
const SALT_ROYALS: u32 = 0x100;
const SALT_CARDS: u32 = 0x200;
const SALT_REROLL: u32 = 0x1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seat {
    Human,
    Bot(Difficulty),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub seats: Vec<Seat>,
    pub wind: bool,
    pub seed: u64,
}

impl Settings {
    pub fn validate(&self) -> Result<(), Refusal> {
        let n = self.seats.len();
        if (tune::MIN_PLAYERS..=tune::MAX_PLAYERS).contains(&n) {
            Ok(())
        } else {
            Err(Refusal::BadSettings)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Lobby,
    Upgrade,
    Build,
    Siege,
    /// The siege timer ran out; waiting for its result.
    SiegeOver,
    Results,
}

/// What one castle went through in a siege.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CastleOutcome {
    pub owner: PlayerId,
    pub attacker: PlayerId,
    /// A ghost copy: kills pay the attacker but cost the owner nothing.
    pub ghost: bool,
    pub royals_killed: Vec<RoyalKind>,
    /// Ledger ids of the producer buildings still standing.
    pub buildings_survived: Vec<u32>,
}

/// What the siege produces for the rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiegeResult {
    pub castles: Vec<CastleOutcome>,
}

/// Shots a player takes into a siege.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub normal: u32,
    pub ammo: Vec<(AmmoKind, u32)>,
    pub air: Vec<AirKind>,
    pub power_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerId,
    pub seat: Seat,
    pub health: i32,
    pub purse: Purse,
    pub ledger: Ledger,
    pub royals: Vec<Royal>,
    pub upgrades: Vec<usize>,
    pub mods: Modifiers,
    pub offer: Option<Offer>,
    pub ready: bool,
    /// Extra normal shots this round from armories that survived last round.
    pub armory_shots: u32,
    /// Where royals may stand, from the build layer; empty means the floor.
    pub standing_spots: Vec<Spot>,
    /// The round this player was knocked out, if they were.
    pub out_round: Option<u32>,
    /// This player's copy of the match terrain.
    pub terrain: terrain::Plot,
    /// The old ruins' free stone: ledger id and the segment it is (R1.600).
    pub ruin_pieces: Vec<(u32, Segment)>,
    /// A building on the watchtower hill stood last siege: this player sees
    /// next round's wind.
    pub sees_next_wind: bool,
}

impl Player {
    fn new(id: PlayerId, seat: Seat, terrain: terrain::Plot) -> Self {
        let mut ledger = Ledger::default();
        let ruin_pieces = terrain
            .spots
            .iter()
            .flat_map(|s| s.segments.iter())
            .map(|seg| (ledger.grant_free(Piece::Segment(Material::Stone), 0), *seg))
            .collect();
        Player {
            id,
            seat,
            health: tune::START_HEALTH,
            purse: Purse::default(),
            ledger,
            royals: Vec::new(),
            upgrades: Vec::new(),
            mods: Modifiers::default(),
            offer: None,
            ready: false,
            armory_shots: 0,
            standing_spots: Vec::new(),
            out_round: None,
            terrain,
            ruin_pieces,
            sees_next_wind: false,
        }
    }

    pub fn alive(&self) -> bool {
        self.out_round.is_none()
    }

    pub fn plot(&self) -> Plot {
        let row = economy::level_row(self.purse.level);
        Plot {
            width: row.plot_width,
            height: row.plot_height,
        }
    }

    pub fn loadout(&self) -> Loadout {
        let row = economy::level_row(self.purse.level);
        Loadout {
            normal: row.normal_shots + self.ledger.shots_bought + self.armory_shots + self.mods.extra_shots,
            ammo: self.mods.ammo.clone(),
            air: self.mods.air.clone(),
            power_pct: row.shot_power_pct,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub settings: Settings,
    pub phase: Phase,
    pub round: u32,
    pub ticks_left: u32,
    pub players: Vec<Player>,
    pub pairings: Vec<Pairing>,
    pub history: History,
    pub tier: Option<Tier>,
    upgrade_rounds: usize,
    /// The match's seeded terrain; every player's plot is a copy.
    pub terrain: terrain::Plot,
    /// This round's wind, -WIND_MAX..=WIND_MAX; 0 with wind off.
    pub wind: i32,
    /// Knocked-out players, first out first.
    pub knocked_out: Vec<PlayerId>,
    pub winner: Option<PlayerId>,
}

pub fn is_upgrade_round(round: u32) -> bool {
    round >= tune::FIRST_UPGRADE_ROUND
        && (round - tune::FIRST_UPGRADE_ROUND).is_multiple_of(tune::UPGRADE_EVERY)
}

impl Match {
    pub fn new(settings: Settings) -> Result<Self, Refusal> {
        settings.validate()?;
        let ground = terrain::Plot::generate(settings.seed);
        let players = settings
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| Player::new(i as PlayerId, *s, ground.clone()))
            .collect();
        Ok(Match {
            settings,
            phase: Phase::Lobby,
            round: 0,
            ticks_left: 0,
            players,
            pairings: Vec::new(),
            history: History::default(),
            tier: None,
            upgrade_rounds: 0,
            terrain: ground,
            wind: 0,
            knocked_out: Vec::new(),
            winner: None,
        })
    }

    pub fn start(&mut self) -> Result<(), Refusal> {
        if self.phase != Phase::Lobby {
            return Err(Refusal::WrongPhase);
        }
        self.begin_round(1);
        Ok(())
    }

    pub fn alive(&self) -> Vec<PlayerId> {
        self.players.iter().filter(|p| p.alive()).map(|p| p.id).collect()
    }

    pub fn player(&self, p: PlayerId) -> Option<&Player> {
        self.players.get(p as usize)
    }

    fn player_mut(&mut self, p: PlayerId) -> Result<&mut Player, Refusal> {
        let pl = self.players.get_mut(p as usize).ok_or(Refusal::Unknown)?;
        if !pl.alive() {
            return Err(Refusal::Eliminated);
        }
        Ok(pl)
    }

    fn need(&self, phase: Phase) -> Result<(), Refusal> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(Refusal::WrongPhase)
        }
    }

    /// The castle `p` attacks this round, and whether it is a ghost.
    pub fn target_of(&self, p: PlayerId) -> Option<(PlayerId, bool)> {
        self.pairings.iter().find_map(|pr| pr.target_of(p))
    }

    // ---- Phases ------------------------------------------------------------

    fn begin_round(&mut self, round: u32) {
        self.round = round;
        self.pairings.clear();
        if is_upgrade_round(round) {
            self.begin_upgrade();
        } else {
            self.begin_build();
        }
    }

    fn set_phase(&mut self, phase: Phase, seconds: u32) {
        self.phase = phase;
        self.ticks_left = seconds * tune::TICKS_PER_SECOND;
        for p in &mut self.players {
            p.ready = false;
        }
    }

    fn begin_upgrade(&mut self) {
        self.set_phase(Phase::Upgrade, tune::UPGRADE_SECONDS);
        let seed = self.settings.seed;
        let tier = upgrades::roll_tier(
            self.upgrade_rounds,
            &mut Dice::derive(seed, self.round, SALT_TIER),
        );
        self.upgrade_rounds += 1;
        self.tier = Some(tier);
        for p in self.players.iter_mut().filter(|p| p.alive()) {
            let mut d = Dice::derive(seed, self.round, SALT_CARDS + p.id as u32);
            p.offer = Some(Offer::new(tier, &p.upgrades, &mut d));
        }
        self.run_bots();
    }

    fn begin_build(&mut self) {
        self.set_phase(Phase::Build, tune::BUILD_SECONDS);
        let seed = self.settings.seed;
        let alive = self.alive();
        self.pairings = pairing::pair_round(
            &alive,
            &mut self.history,
            &mut Dice::derive(seed, self.round, SALT_PAIRS),
        );
        self.wind = self.wind_for(self.round);
        let arriving = royals::arrivals(self.round);
        for p in self.players.iter_mut().filter(|p| p.alive()) {
            p.offer = None;
            p.ledger.shots_bought = 0;
            p.purse.collect_income();
            p.purse.add_xp(tune::FREE_XP_PER_ROUND);
            p.royals
                .extend(arriving.iter().map(|k| Royal { kind: *k, spot: None }));
        }
        self.run_bots();
    }

    fn end_upgrade(&mut self) {
        let ids = self.alive();
        for id in ids {
            let unpicked = self.players[id as usize]
                .offer
                .as_ref()
                .is_some_and(|o| o.picked.is_none());
            if unpicked {
                let _ = self.pick(id, 0);
            }
        }
        self.begin_build();
    }

    fn end_build(&mut self) {
        let seed = self.settings.seed;
        let round = self.round;
        for p in self.players.iter_mut().filter(|p| p.alive()) {
            let plot = p.plot();
            let mut d = Dice::derive(seed, round, SALT_ROYALS + p.id as u32);
            royals::auto_place(&mut p.royals, &p.standing_spots, plot, &mut d);
        }
        self.set_phase(Phase::Siege, tune::SIEGE_SECONDS);
    }

    fn all_ready(&self) -> bool {
        self.players.iter().filter(|p| p.alive()).all(|p| p.ready)
    }

    /// Advance one tick. A phase ends when its timer runs out or everyone
    /// alive is ready.
    pub fn tick(&mut self) {
        if !matches!(self.phase, Phase::Upgrade | Phase::Build | Phase::Siege) {
            return;
        }
        self.ticks_left = self.ticks_left.saturating_sub(1);
        if self.ticks_left == 0 || self.all_ready() {
            self.end_phase();
        }
    }

    fn end_phase(&mut self) {
        match self.phase {
            Phase::Upgrade => self.end_upgrade(),
            Phase::Build => self.end_build(),
            Phase::Siege => {
                self.phase = Phase::SiegeOver;
                self.ticks_left = 0;
            }
            _ => {}
        }
    }

    /// Wind for `round`, the same for everyone; 0 with wind off.
    fn wind_for(&self, round: u32) -> i32 {
        if !self.settings.wind {
            return 0;
        }
        let span = (2 * tune::WIND_MAX + 1) as u32;
        Dice::derive(self.settings.seed, round, SALT_WIND).below(span) as i32 - tune::WIND_MAX
    }

    /// Next round's wind, when `p` holds the watchtower hill and wind is on.
    pub fn forecast(&self, p: PlayerId) -> Option<i32> {
        let pl = self.player(p)?;
        (pl.sees_next_wind && self.settings.wind).then(|| self.wind_for(self.round + 1))
    }

    pub fn set_ready(&mut self, p: PlayerId) -> Result<(), Refusal> {
        self.player_mut(p)?.ready = true;
        Ok(())
    }

    /// Apply the siege's outcome: damage, kill gold, producer yields,
    /// knockouts; then the next round or the results.
    pub fn apply_siege(&mut self, result: &SiegeResult) -> Result<(), Refusal> {
        if !matches!(self.phase, Phase::Siege | Phase::SiegeOver) {
            return Err(Refusal::WrongPhase);
        }
        let round = self.round;
        for p in &mut self.players {
            p.armory_shots = 0;
            p.sees_next_wind = false;
        }
        for c in &result.castles {
            if self.target_of(c.attacker) == Some((c.owner, c.ghost)) {
                if let Some(a) = self.players.get_mut(c.attacker as usize) {
                    a.purse.gold += economy::kill_gold(&c.royals_killed);
                }
            }
            if c.ghost {
                continue;
            }
            let Some(o) = self.players.get_mut(c.owner as usize) else {
                continue;
            };
            if !o.alive() {
                continue;
            }
            o.health -= royals::damage(&c.royals_killed, round) as i32;
            let pay = o.ledger.collect_yields(&c.buildings_survived);
            o.armory_shots += pay.shots;
            o.purse.gold += pay.gold;
            o.sees_next_wind |= pay.sees_wind;
            if o.health > 0 {
                o.health = (o.health + pay.heal).min(tune::START_HEALTH);
            }
        }
        let mut out: Vec<&Player> = self
            .players
            .iter()
            .filter(|p| p.alive() && p.health <= 0)
            .collect();
        out.sort_by_key(|p| (p.health, p.id));
        let out: Vec<PlayerId> = out.iter().map(|p| p.id).collect();
        for id in out {
            self.players[id as usize].out_round = Some(round);
            self.knocked_out.push(id);
        }
        let alive = self.alive();
        if alive.len() <= 1 {
            self.winner = alive.first().copied().or(self.knocked_out.last().copied());
            self.phase = Phase::Results;
            self.pairings.clear();
        } else {
            self.begin_round(round + 1);
        }
        Ok(())
    }

    /// Final standings, winner first.
    pub fn standings(&self) -> Vec<PlayerId> {
        let mut v = self.alive();
        v.extend(self.knocked_out.iter().rev());
        v
    }

    // ---- Player actions ------------------------------------------------------

    pub fn buy_xp(&mut self, p: PlayerId) -> Result<(), Refusal> {
        self.need(Phase::Build)?;
        self.player_mut(p)?.purse.buy_xp()
    }

    pub fn buy_shot(&mut self, p: PlayerId) -> Result<(), Refusal> {
        self.need(Phase::Build)?;
        let pl = self.player_mut(p)?;
        pl.ledger.buy_shot(&mut pl.purse.gold)
    }

    /// Place a segment or rope; returns its ledger id for the physics world.
    /// Buildings go through `place_at`.
    pub fn place(&mut self, p: PlayerId, piece: Piece) -> Result<u32, Refusal> {
        self.need(Phase::Build)?;
        if matches!(piece, Piece::Building(_)) {
            return Err(Refusal::BadSpot);
        }
        let round = self.round;
        let pl = self.player_mut(p)?;
        pl.ledger
            .place(piece, round, pl.purse.level, &mut pl.purse.gold, &pl.mods)
    }

    /// Place a building with its footprint at terrain column `col`, which
    /// must lie in the player's buildable span. Returns its ledger id.
    pub fn place_at(&mut self, p: PlayerId, piece: Piece, col: usize) -> Result<u32, Refusal> {
        self.need(Phase::Build)?;
        let round = self.round;
        let pl = self.player_mut(p)?;
        let spot = sites::site(&pl.terrain, pl.purse.level, col)?;
        let id = pl
            .ledger
            .place(piece, round, pl.purse.level, &mut pl.purse.gold, &pl.mods)?;
        pl.ledger.set_site(id, col, spot);
        Ok(id)
    }

    pub fn sell(&mut self, p: PlayerId, id: u32) -> Result<u32, Refusal> {
        self.need(Phase::Build)?;
        let round = self.round;
        let pl = self.player_mut(p)?;
        pl.ledger.sell(id, round, &mut pl.purse.gold)
    }

    pub fn place_royal(&mut self, p: PlayerId, index: usize, spot: Spot) -> Result<(), Refusal> {
        self.need(Phase::Build)?;
        let pl = self.player_mut(p)?;
        if index >= pl.royals.len() {
            return Err(Refusal::Unknown);
        }
        if !royals::valid_spot(&pl.royals, index, spot, pl.plot()) {
            return Err(Refusal::BadSpot);
        }
        pl.royals[index].spot = Some(spot);
        Ok(())
    }

    pub fn set_standing_spots(&mut self, p: PlayerId, spots: Vec<Spot>) -> Result<(), Refusal> {
        self.player_mut(p)?.standing_spots = spots;
        Ok(())
    }

    pub fn reroll(&mut self, p: PlayerId) -> Result<(), Refusal> {
        self.need(Phase::Upgrade)?;
        let (seed, round) = (self.settings.seed, self.round);
        let pl = self.player_mut(p)?;
        let offer = pl.offer.as_mut().ok_or(Refusal::WrongPhase)?;
        let mut d = Dice::derive(seed, round, SALT_REROLL + (p as u32) * 64 + offer.rerolls);
        offer.reroll(&mut pl.purse, &pl.upgrades, &mut d)
    }

    pub fn pick(&mut self, p: PlayerId, slot: usize) -> Result<(), Refusal> {
        self.need(Phase::Upgrade)?;
        let pl = self.player_mut(p)?;
        let offer = pl.offer.as_mut().ok_or(Refusal::WrongPhase)?;
        let i = offer.pick(slot)?;
        upgrades::apply(
            UPGRADES[i].effect,
            &mut pl.purse,
            &mut pl.ledger.stock,
            &mut pl.mods,
        );
        pl.upgrades.push(i);
        Ok(())
    }

    // ---- Bots -----------------------------------------------------------------

    /// Apply a policy's actions for `p`; refused actions are skipped.
    pub fn apply_bot_actions(&mut self, p: PlayerId, actions: &[BotAction]) {
        for a in actions {
            let _ = match *a {
                BotAction::BuyXp => self.buy_xp(p),
                BotAction::Place(piece) => self.place(p, piece).map(|_| ()),
                BotAction::PlaceAt(piece, col) => self.place_at(p, piece, col).map(|_| ()),
                BotAction::BuyShot => self.buy_shot(p),
                BotAction::Reroll => self.reroll(p),
                BotAction::Pick(slot) => self.pick(p, slot),
                BotAction::Ready => self.set_ready(p),
            };
        }
    }

    /// Let `policy` act for `p` in the current phase.
    pub fn run_policy(&mut self, p: PlayerId, policy: &dyn BotPolicy) {
        let Some(pl) = self.player(p) else { return };
        let actions = match self.phase {
            Phase::Upgrade => match &pl.offer {
                Some(o) => policy.upgrade(pl, o, self.round),
                None => Vec::new(),
            },
            Phase::Build => policy.build(pl, self.round),
            _ => Vec::new(),
        };
        self.apply_bot_actions(p, &actions);
    }

    fn run_bots(&mut self) {
        let bots: Vec<(PlayerId, Difficulty)> = self
            .players
            .iter()
            .filter(|p| p.alive())
            .filter_map(|p| match p.seat {
                Seat::Bot(d) => Some((p.id, d)),
                Seat::Human => None,
            })
            .collect();
        for (id, d) in bots {
            self.run_policy(id, &SimpleBot { difficulty: d });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn humans(n: usize) -> Match {
        let mut m = Match::new(Settings {
            seats: vec![Seat::Human; n],
            wind: true,
            seed: 11,
        })
        .unwrap();
        m.start().unwrap();
        m
    }

    fn run_out(m: &mut Match) {
        let phase = m.phase;
        while m.phase == phase {
            m.tick();
        }
    }

    #[test]
    fn lobby_needs_two_to_eight_seats() {
        let s = |n| Settings {
            seats: vec![Seat::Human; n],
            wind: false,
            seed: 0,
        };
        assert_eq!(Match::new(s(1)).unwrap_err(), Refusal::BadSettings);
        assert_eq!(Match::new(s(9)).unwrap_err(), Refusal::BadSettings);
        assert!(Match::new(s(2)).is_ok() && Match::new(s(8)).is_ok());
    }

    #[test]
    fn rounds_run_build_siege_then_upgrade_on_even_rounds() {
        let mut m = humans(2);
        assert_eq!((m.round, m.phase), (1, Phase::Build));
        assert_eq!(m.ticks_left, 60 * tune::TICKS_PER_SECOND);
        run_out(&mut m);
        assert_eq!(m.phase, Phase::Siege);
        run_out(&mut m);
        assert_eq!(m.phase, Phase::SiegeOver);
        m.apply_siege(&SiegeResult::default()).unwrap();
        assert_eq!((m.round, m.phase), (2, Phase::Upgrade));
        assert_eq!(m.ticks_left, 20 * tune::TICKS_PER_SECOND);
        run_out(&mut m);
        assert_eq!((m.round, m.phase), (2, Phase::Build));
        assert!(!is_upgrade_round(3) && is_upgrade_round(4) && !is_upgrade_round(1));
    }

    #[test]
    fn a_phase_ends_early_once_everyone_is_ready() {
        let mut m = humans(3);
        m.set_ready(0).unwrap();
        m.set_ready(1).unwrap();
        m.tick();
        assert_eq!(m.phase, Phase::Build);
        m.set_ready(2).unwrap();
        m.tick();
        assert_eq!(m.phase, Phase::Siege);
    }

    #[test]
    fn round_one_pays_income_and_free_xp_and_brings_a_squire() {
        let m = humans(2);
        let p = &m.players[0];
        assert_eq!(p.purse.gold, tune::BASE_INCOME);
        assert_eq!(p.purse.level, 2);
        assert_eq!(p.royals.len(), 1);
        assert_eq!(m.target_of(0), Some((1, false)));
    }

    #[test]
    fn killed_royals_cost_health_and_pay_the_attacker() {
        let mut m = humans(2);
        run_out(&mut m);
        let r = SiegeResult {
            castles: vec![CastleOutcome {
                owner: 1,
                attacker: 0,
                ghost: false,
                royals_killed: vec![RoyalKind::Squire],
                buildings_survived: vec![],
            }],
        };
        m.apply_siege(&r).unwrap();
        assert_eq!(m.players[1].health, 99);
        // 5 income round 1, +1 kill gold, and round 2 has not paid yet (upgrade phase)
        assert_eq!(m.players[0].purse.gold, 6);
    }

    #[test]
    fn ghost_kills_pay_the_attacker_but_spare_the_owner() {
        let mut m = humans(3);
        run_out(&mut m);
        let (attacker, owner) = m
            .pairings
            .iter()
            .find_map(|p| match p {
                Pairing::Ghost { attacker, copy_of } => Some((*attacker, *copy_of)),
                _ => None,
            })
            .unwrap();
        let r = SiegeResult {
            castles: vec![CastleOutcome {
                owner,
                attacker,
                ghost: true,
                royals_killed: vec![RoyalKind::Squire],
                buildings_survived: vec![],
            }],
        };
        let gold = m.players[attacker as usize].purse.gold;
        m.apply_siege(&r).unwrap();
        assert_eq!(m.players[owner as usize].health, tune::START_HEALTH);
        assert_eq!(m.players[attacker as usize].purse.gold, gold + 1);
    }

    #[test]
    fn a_knocked_out_player_leaves_the_pairings_and_the_last_one_wins() {
        let mut m = humans(3);
        run_out(&mut m);
        m.players[2].health = 1;
        let r = SiegeResult {
            castles: vec![CastleOutcome {
                owner: 2,
                attacker: 9,
                ghost: false,
                royals_killed: vec![RoyalKind::Squire],
                buildings_survived: vec![],
            }],
        };
        m.apply_siege(&r).unwrap();
        assert_eq!(m.alive(), vec![0, 1]);
        run_out(&mut m); // upgrade phase
        assert_eq!(m.pairings.len(), 1);
        assert!(matches!(m.pairings[0], Pairing::Duel(0, 1) | Pairing::Duel(1, 0)));
        assert_eq!(
            m.place(2, Piece::Segment(Material::Wood)),
            Err(Refusal::Eliminated)
        );
        run_out(&mut m);
        m.players[1].health = 0;
        m.apply_siege(&SiegeResult::default()).unwrap();
        assert_eq!((m.phase, m.winner), (Phase::Results, Some(0)));
        assert_eq!(m.standings(), vec![0, 1, 2]);
    }

    #[test]
    fn unplaced_royals_are_placed_when_build_ends() {
        let mut m = humans(2);
        assert_eq!(m.players[0].royals[0].spot, None);
        run_out(&mut m);
        let pl = &m.players[0];
        let s = pl.royals[0].spot.expect("placed");
        assert!(royals::valid_spot(&pl.royals, 0, s, pl.plot()));
    }

    #[test]
    fn level_unlocks_stone_at_four_and_grow_the_plot() {
        let mut m = humans(2);
        m.players[0].purse.gold = 100;
        assert_eq!(m.place(0, Piece::Segment(Material::Stone)), Err(Refusal::Locked));
        let small = m.players[0].plot();
        m.players[0].purse.add_xp(8); // level 2 -> 4
        assert_eq!(m.players[0].purse.level, 4);
        assert!(m.place(0, Piece::Segment(Material::Stone)).is_ok());
        let big = m.players[0].plot();
        assert!(big.width > small.width && big.height > small.height);
        assert_eq!(m.players[0].loadout().normal, 4);
    }

    #[test]
    fn picking_an_upgrade_card_applies_it() {
        let mut m = humans(2);
        run_out(&mut m);
        m.apply_siege(&SiegeResult::default()).unwrap();
        let card = m.players[0].offer.as_ref().unwrap().cards[0];
        m.pick(0, 0).unwrap();
        assert_eq!(m.players[0].upgrades, vec![card]);
        assert_eq!(m.pick(0, 1), Err(Refusal::AlreadyPicked));
        // player 1 never picked: the first card is taken for them at phase end
        run_out(&mut m);
        assert_eq!(m.players[1].upgrades.len(), 1);
    }

    #[test]
    fn a_bot_match_runs_to_a_winner() {
        let seats = vec![Seat::Bot(Difficulty::Normal); 8];
        let mut m = Match::new(Settings {
            seats,
            wind: true,
            seed: 5,
        })
        .unwrap();
        m.start().unwrap();
        let mut dice = Dice::new(99);
        let mut guard = 0;
        while m.phase != Phase::Results {
            guard += 1;
            assert!(guard < 1_000_000);
            match m.phase {
                Phase::Siege | Phase::SiegeOver => {
                    let mut r = SiegeResult::default();
                    for &p in &m.alive() {
                        let (t, ghost) = m.target_of(p).unwrap();
                        let kills: Vec<RoyalKind> = m.players[t as usize]
                            .royals
                            .iter()
                            .filter(|_| dice.below(3) == 0)
                            .map(|r| r.kind)
                            .collect();
                        r.castles.push(CastleOutcome {
                            owner: t,
                            attacker: p,
                            ghost,
                            royals_killed: kills,
                            buildings_survived: vec![],
                        });
                    }
                    m.apply_siege(&r).unwrap();
                }
                _ => m.tick(),
            }
        }
        assert!(m.winner.is_some());
        assert_eq!(m.standings().len(), 8);
        assert!(m.round >= 10, "ended in round {}", m.round);
    }
}
