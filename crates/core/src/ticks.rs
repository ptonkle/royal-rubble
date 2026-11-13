//! Fixed ticks from display frames, with taps latched between ticks. Part of
//! the realtime kit (`kits/realtime/README.md`), after Pursuer's `advance`.
//!
//! The page calls `Clock::frame` once per display frame with the frame's
//! timestamp and the input bits from `web/input.js`. The clock says which
//! ticks run, and with which bits. Rules run on ticks; the display frame
//! rate never changes a result.
//!
//! The page's timestamp is milliseconds with a fraction. The shim converts
//! it to whole microseconds (one multiplication and a cast, which decides
//! nothing), so the core sees an integer.
//!
//! A display frame can come sooner than a tick. A key pressed and released
//! inside such a frame reaches the core in that frame's bits, and no tick
//! runs to use it. The clock keeps those bits and adds them to the next
//! tick that runs, so a quick tap is never lost.

/// Ticks a second, and the length of one in microseconds.
pub const TICKS_PER_SECOND: i64 = 60;
pub const TICK_US: i64 = 1_000_000 / TICKS_PER_SECOND;

/// The most ticks one display frame may run. After a stall (a hidden tab, a
/// debugger) the rest is dropped, so the run does not race to catch up.
pub const MAX_TICKS_PER_FRAME: usize = 5;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Clock {
    last: Option<i64>,
    owed: i64,
    latched: u32,
}

impl Clock {
    /// The input bits for each tick to run this frame, in order. Often
    /// empty, often one, more after a slow frame.
    pub fn frame(&mut self, now_us: i64, bits: u32) -> Vec<u32> {
        let elapsed = match self.last {
            Some(prev) => (now_us - prev).clamp(0, TICK_US * MAX_TICKS_PER_FRAME as i64),
            // the first frame runs one tick
            None => TICK_US,
        };
        self.last = Some(now_us);
        self.owed += elapsed;
        let mut ticks = Vec::new();
        while self.owed >= TICK_US && ticks.len() < MAX_TICKS_PER_FRAME {
            self.owed -= TICK_US;
            ticks.push(bits | self.latched);
            self.latched = 0;
        }
        if ticks.is_empty() {
            self.latched |= bits;
        }
        ticks
    }
}

/// Input bits for the example world.
pub const FIRE: u32 = 1;
pub const RIGHT: u32 = 2;

/// An example world that counts presses (not held ticks) and moves while a
/// key is held. Replace it with the project's own.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct World {
    pub presses: u32,
    pub x: i32,
    prev: u32,
}

impl World {
    pub fn tick(&mut self, bits: u32) {
        if bits & FIRE != 0 && self.prev & FIRE == 0 {
            self.presses += 1;
        }
        if bits & RIGHT != 0 {
            self.x += 1;
        }
        self.prev = bits;
    }
}

/// One display frame: what the shim's `advance` calls.
pub fn advance(clock: &mut Clock, world: &mut World, now_us: i64, bits: u32) -> usize {
    let ticks = clock.frame(now_us, bits);
    for b in &ticks {
        world.tick(*b);
    }
    ticks.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_released_before_any_tick_ran_still_counts() {
        let (mut c, mut w) = (Clock::default(), World::default());
        assert_eq!(advance(&mut c, &mut w, 0, 0), 1);
        // pressed and let go inside a frame too short for a tick
        assert_eq!(advance(&mut c, &mut w, TICK_US / 4, FIRE), 0);
        // the next frame's bits no longer hold it
        assert_eq!(advance(&mut c, &mut w, TICK_US + 1, 0), 1);
        assert_eq!(w.presses, 1, "the tap was lost");
    }

    #[test]
    fn a_latched_tap_is_used_once() {
        let (mut c, mut w) = (Clock::default(), World::default());
        advance(&mut c, &mut w, 0, 0);
        advance(&mut c, &mut w, TICK_US / 4, FIRE);
        advance(&mut c, &mut w, TICK_US + 1, 0);
        advance(&mut c, &mut w, 2 * TICK_US + 1, 0);
        advance(&mut c, &mut w, 3 * TICK_US + 1, FIRE);
        assert_eq!(w.presses, 2);
    }

    #[test]
    fn held_bits_reach_every_tick_of_a_slow_frame() {
        let (mut c, mut w) = (Clock::default(), World::default());
        advance(&mut c, &mut w, 0, 0);
        assert_eq!(advance(&mut c, &mut w, 3 * TICK_US, RIGHT), 3);
        assert_eq!(w.x, 3);
        assert_eq!(w.presses, 0);
    }

    #[test]
    fn a_tap_counts_once_when_a_frame_runs_several_ticks() {
        let (mut c, mut w) = (Clock::default(), World::default());
        advance(&mut c, &mut w, 0, 0);
        assert_eq!(advance(&mut c, &mut w, 3 * TICK_US, FIRE), 3);
        assert_eq!(w.presses, 1);
    }

    #[test]
    fn a_stall_runs_at_most_the_cap_and_drops_the_rest() {
        let (mut c, mut w) = (Clock::default(), World::default());
        advance(&mut c, &mut w, 0, 0);
        let n = advance(&mut c, &mut w, 10 * 1_000_000, RIGHT);
        assert_eq!(n, MAX_TICKS_PER_FRAME);
        assert_eq!(advance(&mut c, &mut w, 10 * 1_000_000 + TICK_US, 0), 1);
    }

    #[test]
    fn a_clock_that_goes_backwards_runs_nothing() {
        let (mut c, mut w) = (Clock::default(), World::default());
        advance(&mut c, &mut w, 5 * TICK_US, 0);
        assert_eq!(advance(&mut c, &mut w, TICK_US, RIGHT), 0);
        assert_eq!(w.x, 0);
        // and owes nothing for it: one tick later, one tick runs
        assert_eq!(advance(&mut c, &mut w, 2 * TICK_US, RIGHT), 1);
    }

    #[test]
    fn the_same_frames_give_the_same_world_at_any_frame_rate() {
        // 30, 60 and 144 frames a second over one second, holding RIGHT
        let mut worlds = Vec::new();
        for fps in [30i64, 60, 144] {
            let (mut c, mut w) = (Clock::default(), World::default());
            for f in 0..=fps {
                advance(&mut c, &mut w, f * 1_000_000 / fps, RIGHT);
            }
            worlds.push(w.x);
        }
        // the first frame runs one tick; the second that follows holds 60
        // whole ticks of 16 666 us (999 960 us), whatever the frame rate
        assert_eq!(worlds, vec![61, 61, 61]);
    }
}
