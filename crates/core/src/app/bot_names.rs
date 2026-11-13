//! The bots' names: drawn from `data/bot-names.json` without repeats,
//! seeded by the match, so every browser in a room names them alike.

use crate::rng::Rng;

/// Keeps the names' stream apart from every other use of the match seed.
const SALT: u64 = 0x4E41_4D45_5342_4F54;

#[derive(serde::Deserialize)]
struct NamesList {
    names: Vec<String>,
}

/// The names list, built into the core.
pub fn all() -> Vec<String> {
    serde_json::from_str::<NamesList>(include_str!("../../../../data/bot-names.json"))
        .map(|f| f.names)
        .unwrap_or_default()
}

/// `count` different names from `list`, seeded by `seed`. Past the list's
/// length the names repeat in order.
pub fn draw(list: &[String], seed: u64, count: usize) -> Vec<String> {
    let mut pool: Vec<usize> = (0..list.len()).collect();
    let mut rng = Rng::new(seed ^ SALT);
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        if pool.is_empty() {
            out.push(list.get(k % list.len().max(1)).cloned().unwrap_or_default());
            continue;
        }
        let i = rng.below(pool.len() as u32) as usize;
        out.push(list[pool.swap_remove(i)].clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abc() -> Vec<String> {
        ["A", "B", "C"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bot_names_never_repeat_within_the_list() {
        let mut got = draw(&abc(), 5, 3);
        got.sort();
        assert_eq!(got, abc());
    }

    #[test]
    fn bot_names_follow_the_seed() {
        assert_eq!(draw(&abc(), 9, 2), draw(&abc(), 9, 2));
        let differ = (0..20u64).any(|s| draw(&abc(), s, 1) != draw(&abc(), 9, 1));
        assert!(differ);
    }

    #[test]
    fn bot_names_file_holds_enough_for_a_full_lobby() {
        let names = all();
        assert!(names.len() >= crate::lobby::MAX_SEATS);
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }
}
