//! A small seeded number source for the match rules (SplitMix64). The match
//! seed comes from the lobby, so every machine rolls the same values.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Self {
        Dice(seed)
    }

    /// A stream for one purpose in one round, so rolls for pairings never
    /// shift the rolls for royals or cards.
    pub fn derive(seed: u64, round: u32, salt: u32) -> Self {
        let mut d = Dice(seed ^ ((round as u64) << 32) ^ (salt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        d.next_u64();
        d
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`; `n` must be above zero.
    pub fn below(&mut self, n: u32) -> u32 {
        debug_assert!(n > 0);
        (self.next_u64() % n as u64) as u32
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dice_with_one_seed_roll_the_same_values() {
        let mut a = Dice::new(7);
        let mut b = Dice::new(7);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Dice::new(7).next_u64(), Dice::new(8).next_u64());
    }
}
