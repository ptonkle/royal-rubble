//! Who attacks whom each round, TFT-style. Pairs attack each other. With an
//! odd count one player attacks a ghost copy of another player's castle;
//! kills on a ghost never cost its owner health. A player may not meet any
//! of their last REMATCH_WINDOW opponents (a ghost counts as meeting its
//! owner); when the window cannot be kept the search narrows it, and only
//! two players ever meet in consecutive rounds (R1.202).

use std::collections::BTreeMap;

use super::dice::Dice;
use super::tune;
use super::PlayerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairing {
    /// Each attacks the other's castle.
    Duel(PlayerId, PlayerId),
    /// `attacker` hits a copy of `copy_of`'s castle; a bot trebuchet aimed
    /// as `copy_of` attacks `attacker`'s real castle (R1.205).
    Ghost { attacker: PlayerId, copy_of: PlayerId },
}

impl Pairing {
    /// The castle `p` attacks this round, and whether it is a ghost.
    pub fn target_of(&self, p: PlayerId) -> Option<(PlayerId, bool)> {
        match *self {
            Pairing::Duel(a, b) if a == p => Some((b, false)),
            Pairing::Duel(a, b) if b == p => Some((a, false)),
            Pairing::Ghost { attacker, copy_of } if attacker == p => Some((copy_of, true)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// Each player's opponents, oldest first.
    pub met: BTreeMap<PlayerId, Vec<PlayerId>>,
    pub ghosts: BTreeMap<PlayerId, u32>,
    pub last_ghost: Option<PlayerId>,
}

impl History {
    fn recent(&self, p: PlayerId, k: usize) -> &[PlayerId] {
        match self.met.get(&p) {
            Some(v) => &v[v.len().saturating_sub(k)..],
            None => &[],
        }
    }

    fn met_lately(&self, a: PlayerId, b: PlayerId, k: usize) -> bool {
        self.recent(a, k).contains(&b) || self.recent(b, k).contains(&a)
    }

    /// The opponent `p` faced last round, if any.
    pub fn last_opponent(&self, p: PlayerId) -> Option<PlayerId> {
        self.met.get(&p).and_then(|v| v.last().copied())
    }

    fn record(&mut self, pairs: &[Pairing]) {
        for pr in pairs {
            match *pr {
                Pairing::Duel(a, b) => {
                    self.met.entry(a).or_default().push(b);
                    self.met.entry(b).or_default().push(a);
                }
                Pairing::Ghost { attacker, copy_of } => {
                    self.met.entry(attacker).or_default().push(copy_of);
                    *self.ghosts.entry(attacker).or_default() += 1;
                }
            }
        }
        self.last_ghost = pairs.iter().find_map(|p| match p {
            Pairing::Ghost { attacker, .. } => Some(*attacker),
            _ => None,
        });
    }
}

/// Pair the alive players for one round and record it in `history`.
pub fn pair_round(alive: &[PlayerId], history: &mut History, dice: &mut Dice) -> Vec<Pairing> {
    let n = alive.len();
    if n < 2 {
        return Vec::new();
    }
    let window = tune::REMATCH_WINDOW[n.min(tune::REMATCH_WINDOW.len() - 1)];
    let mut order: Vec<PlayerId> = alive.to_vec();
    dice.shuffle(&mut order);

    // Ghost candidates: fewest ghosts first, last round's ghost last.
    let mut ghost_order = order.clone();
    ghost_order.sort_by_key(|p| {
        (
            history.last_ghost == Some(*p),
            history.ghosts.get(p).copied().unwrap_or(0),
        )
    });
    let ghost_choices: Vec<Option<PlayerId>> = if n % 2 == 1 {
        ghost_order.iter().copied().map(Some).collect()
    } else {
        vec![None]
    };
    // First pass: only the players most due a ghost, keeping at least the
    // last opponent off limits. Second pass: anyone, narrowing to nothing.
    let key = |p: &PlayerId| {
        (
            history.last_ghost == Some(*p),
            history.ghosts.get(p).copied().unwrap_or(0),
        )
    };
    let due: Vec<Option<PlayerId>> = match ghost_order.first() {
        Some(f) if n % 2 == 1 => ghost_order
            .iter()
            .filter(|p| key(p) == key(f))
            .copied()
            .map(Some)
            .collect(),
        _ => vec![None],
    };
    let passes: Vec<(usize, &Vec<Option<PlayerId>>)> = (1..=window)
        .rev()
        .map(|k| (k, &due))
        .chain((0..=window).rev().map(|k| (k, &ghost_choices)))
        .collect();

    for (k, choices) in passes {
        for g in choices {
            let rest: Vec<PlayerId> = order.iter().copied().filter(|p| Some(*p) != *g).collect();
            let ghost = match g {
                None => None,
                Some(gp) => {
                    let mut targets: Vec<PlayerId> = order.iter().copied().filter(|p| p != gp).collect();
                    targets.retain(|t| !history.recent(*gp, k).contains(t));
                    match targets.first() {
                        Some(t) => Some(Pairing::Ghost {
                            attacker: *gp,
                            copy_of: *t,
                        }),
                        None => continue,
                    }
                }
            };
            let mut pairs = Vec::new();
            if match_rest(&rest, k, history, &mut pairs) {
                pairs.extend(ghost);
                history.record(&pairs);
                return pairs;
            }
        }
    }
    unreachable!("window 0 always pairs")
}

fn match_rest(rest: &[PlayerId], k: usize, history: &History, out: &mut Vec<Pairing>) -> bool {
    let Some((&first, others)) = rest.split_first() else {
        return true;
    };
    for (i, &b) in others.iter().enumerate() {
        if history.met_lately(first, b, k) {
            continue;
        }
        let remaining: Vec<PlayerId> = others
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| *p)
            .collect();
        out.push(Pairing::Duel(first, b));
        if match_rest(&remaining, k, history, out) {
            return true;
        }
        out.pop();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opponents(pairs: &[Pairing], p: PlayerId) -> Option<PlayerId> {
        pairs.iter().find_map(|pr| pr.target_of(p)).map(|(t, _)| t)
    }

    #[test]
    fn no_one_meets_the_same_opponent_two_rounds_running() {
        for n in 3..=8u8 {
            for seed in 0..20u64 {
                let alive: Vec<PlayerId> = (0..n).collect();
                let mut h = History::default();
                let mut last: BTreeMap<PlayerId, PlayerId> = BTreeMap::new();
                for round in 1..=30 {
                    let pairs = pair_round(&alive, &mut h, &mut Dice::derive(seed, round, 1));
                    for &p in &alive {
                        let t = opponents(&pairs, p).expect("everyone has a target");
                        assert_ne!(last.get(&p), Some(&t), "n={n} seed={seed} round={round} p={p}");
                        last.insert(p, t);
                    }
                }
            }
        }
    }

    #[test]
    fn eight_players_avoid_their_last_three_opponents() {
        let alive: Vec<PlayerId> = (0..8).collect();
        let mut h = History::default();
        let mut seen: BTreeMap<PlayerId, Vec<PlayerId>> = BTreeMap::new();
        for round in 1..=40 {
            let pairs = pair_round(&alive, &mut h, &mut Dice::derive(9, round, 1));
            for &p in &alive {
                let t = opponents(&pairs, p).unwrap();
                let v = seen.entry(p).or_default();
                assert!(!v[v.len().saturating_sub(3)..].contains(&t), "round {round}");
                v.push(t);
            }
        }
    }

    #[test]
    fn two_players_always_meet_each_other() {
        let mut h = History::default();
        for round in 1..=5 {
            let pairs = pair_round(&[2, 5], &mut h, &mut Dice::derive(1, round, 1));
            assert_eq!(pairs.len(), 1);
            assert_eq!(opponents(&pairs, 2), Some(5));
        }
    }

    #[test]
    fn an_odd_count_sends_one_player_against_a_ghost() {
        for n in [3u8, 5, 7] {
            let alive: Vec<PlayerId> = (0..n).collect();
            let pairs = pair_round(&alive, &mut History::default(), &mut Dice::new(4));
            let ghosts: Vec<&Pairing> = pairs
                .iter()
                .filter(|p| matches!(p, Pairing::Ghost { .. }))
                .collect();
            assert_eq!(ghosts.len(), 1, "n={n}");
            if let Pairing::Ghost { attacker, copy_of } = ghosts[0] {
                assert_ne!(attacker, copy_of);
                assert!(alive.contains(copy_of));
            }
            assert_eq!(pairs.len(), (n as usize).div_ceil(2));
        }
    }

    #[test]
    fn the_ghost_moves_around_the_table() {
        let alive: Vec<PlayerId> = (0..5).collect();
        let mut h = History::default();
        for round in 1..=10 {
            pair_round(&alive, &mut h, &mut Dice::derive(2, round, 1));
        }
        // ten ghost rounds over five players: two each
        assert!(
            alive.iter().all(|p| h.ghosts.get(p) == Some(&2)),
            "{:?}",
            h.ghosts
        );
    }

    #[test]
    fn eliminated_players_leave_the_pairings() {
        let mut h = History::default();
        let mut alive: Vec<PlayerId> = (0..6).collect();
        pair_round(&alive, &mut h, &mut Dice::new(1));
        alive.retain(|p| *p != 3);
        for round in 2..=8 {
            let pairs = pair_round(&alive, &mut h, &mut Dice::derive(1, round, 1));
            for pr in &pairs {
                let ids = match *pr {
                    Pairing::Duel(a, b) => [a, b],
                    Pairing::Ghost { attacker, copy_of } => [attacker, copy_of],
                };
                assert!(!ids.contains(&3));
            }
        }
    }
}
