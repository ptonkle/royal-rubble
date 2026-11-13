//! Chance from a seed. Part of the realtime kit (`kits/realtime/README.md`).
//!
//! The core never reads a random device (BUILDER-SPEC.md, D2). The page
//! draws a seed once, from `crypto.getRandomValues`, and passes it in; the
//! core keeps it, so a replay, a test or the other browser in a lockstep
//! match can run the same sequence again. (Peter's games pass entropy in
//! the same way.)
//!
//! ```text
//! // web/app.js
//! const seed = new Uint32Array(1);
//! crypto.getRandomValues(seed);
//! const game = new Game(strings, seed[0]);
//! ```
//!
//! The generator is SplitMix64: one 64-bit state, a fixed step, and a mix.
//! It is fast and well spread for games. It is not for secrets.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A whole number from 0 up to, not including, `n`. `n` must be above 0.
    /// Uses the high half of a 64-bit product, so there is no modulo; the
    /// bias is below one part in four billion.
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }

    /// From `lo` to `hi`, both included.
    pub fn between(&mut self, lo: i32, hi: i32) -> i32 {
        let span = (hi as i64 - lo as i64 + 1) as u32;
        lo + self.below(span) as i32
    }

    /// True `num` times in `den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_values_match_the_published_splitmix64_sequence() {
        // SplitMix64 from seed 0, worked out separately from the published
        // algorithm (a five-line transcription in another language)
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(r.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn the_same_seed_gives_the_same_sequence_and_another_does_not() {
        let a: Vec<u32> = {
            let mut r = Rng::new(42);
            (0..8).map(|_| r.next_u32()).collect()
        };
        let b: Vec<u32> = {
            let mut r = Rng::new(42);
            (0..8).map(|_| r.next_u32()).collect()
        };
        let c: Vec<u32> = {
            // differs from 42 only in a high bit
            let mut r = Rng::new(42 + (1 << 40));
            (0..8).map(|_| r.next_u32()).collect()
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn below_and_between_stay_in_range_and_reach_every_value() {
        let mut r = Rng::new(7);
        let mut seen = [0u32; 6];
        for _ in 0..600 {
            let v = r.between(-2, 3);
            assert!((-2..=3).contains(&v), "{v}");
            seen[(v + 2) as usize] += 1;
        }
        assert!(seen.iter().all(|&n| n > 50), "{seen:?}");
        assert!((0..100).all(|_| r.below(1) == 0));
        // seed 0's first u32 is 0xe220a839 = 3793791033; scaled to ten
        // values that is 3793791033 * 10 / 2^32 = 8.83, so 8 (a modulo
        // would give 3)
        assert_eq!(Rng::new(0).below(10), 8);
    }

    #[test]
    fn chance_is_never_and_always_at_its_ends() {
        let mut r = Rng::new(9);
        assert!((0..100).all(|_| !r.chance(0, 3)));
        assert!((0..100).all(|_| r.chance(3, 3)));
    }
}
