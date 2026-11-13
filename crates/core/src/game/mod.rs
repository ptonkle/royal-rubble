//! The match rules of Royal Rubble: lobby settings, rounds and phases,
//! pairings, health, economy, the build ledger, royals and upgrades. Pure
//! and deterministic; no physics. The siege hands its outcome back as a
//! `SiegeResult`, so these rules never look inside the physics world.
//! Every balance number is in `tune`.

pub mod bots;
pub mod dice;
pub mod economy;
pub mod flow;
pub mod ledger;
pub mod pairing;
pub mod royals;
pub mod sites;
pub mod tune;
pub mod upgrades;

pub use flow::{CastleOutcome, Match, Phase, Seat, Settings, SiegeResult};

/// A seat index, 0-based, stable for the whole match.
pub type PlayerId = u8;

/// Why a rule refused an action. The page words each one from data/.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotEnoughGold,
    Locked,
    MaxLevel,
    Limit,
    Unknown,
    WrongPhase,
    Eliminated,
    AlreadyPicked,
    BadSpot,
    BadSettings,
}
