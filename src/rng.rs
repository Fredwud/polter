// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Deterministic randomness. Same seed, same numbers, every run: that's what makes replays work.

/// A small, fast random number generator (xorshift64*).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng((seed ^ 0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    /// An integer in `lo..hi` (hi excluded). Returns `lo` if the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }

    /// A float in `0.0..1.0`.
    pub fn float(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.float() < p
    }
}
