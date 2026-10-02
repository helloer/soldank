//! Bit-exact port of Free Pascal's `Random`/`RandSeed` (FPC 3.2.2 `rtl/inc/system.inc`,
//! Mersenne Twister MT19937), so gameplay randomness can match opensoldat.
//!
//! Like Pascal, assigning [`PascalRandom::rand_seed`] reseeds the generator on the next
//! call, and after reseeding `rand_seed` holds the bit-inverted seed (FPC uses that to
//! detect reassignments).

const N: usize = 624;
const M: usize = 397;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7FFF_FFFF;
const MATRIX_A: u32 = 0x9908_B0DF;

#[derive(Clone)]
pub struct PascalRandom {
    /// Pascal's `RandSeed` variable.
    pub rand_seed: u32,
    old_rand_seed: u32,
    index: usize,
    state: [u32; N],
}

impl std::fmt::Debug for PascalRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PascalRandom")
            .field("rand_seed", &self.rand_seed)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl PascalRandom {
    pub fn new(seed: u32) -> PascalRandom {
        PascalRandom {
            rand_seed: seed,
            old_rand_seed: 0,
            index: N + 1,
            state: [0; N],
        }
    }

    fn init(&mut self, seed: u32) {
        self.state[0] = seed;
        for i in 1..N {
            let prev = self.state[i - 1];
            self.state[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = N;
    }

    fn twist(u: u32, v: u32) -> u32 {
        let mixed = (u & UPPER_MASK) | (v & LOWER_MASK);
        (mixed >> 1) ^ ((v & 1).wrapping_neg() & MATRIX_A)
    }

    fn update_state(&mut self) {
        // FPC iterates N-M times in the first loop (see the comment in system.inc)
        for i in 0..N - M {
            self.state[i] = self.state[i + M] ^ Self::twist(self.state[i], self.state[i + 1]);
        }
        for i in N - M..N - 1 {
            self.state[i] = self.state[i + M - N] ^ Self::twist(self.state[i], self.state[i + 1]);
        }
        self.state[N - 1] = self.state[M - 1] ^ Self::twist(self.state[N - 1], self.state[0]);
        self.index = 0;
    }

    /// `mtwist_u32rand`
    pub fn next_u32(&mut self) -> u32 {
        let mut index = self.index;
        self.index += 1;

        if self.rand_seed != self.old_rand_seed || index > N {
            self.init(self.rand_seed);
            self.rand_seed = !self.rand_seed;
            self.old_rand_seed = self.rand_seed;
            index = N;
        }

        if index == N {
            self.update_state();
            index = 0;
            self.index = 1;
        }

        let mut y = self.state[index];
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// `Random(l)`: integer in `0..l`.
    pub fn below(&mut self, l: i32) -> i32 {
        let l = if l < 0 { l + 1 } else { l };
        ((i64::from(self.next_u32()) * i64::from(l)) >> 32) as i32
    }

    /// `Random(l: Int64)` (e.g. `Random(Round(x))`, Round returns Int64): two draws
    /// combined into 63 bits, modulo `l`.
    pub fn below_i64(&mut self, l: i64) -> i64 {
        let lo = u64::from(self.next_u32());
        let hi = u64::from(self.next_u32());
        let value = ((lo | (hi << 32)) & 0x7fff_ffff_ffff_ffff) as i64;
        if l != 0 { value % l } else { 0 }
    }

    /// `Random`: float in `[0, 1)`.
    pub fn float(&mut self) -> f64 {
        f64::from(self.next_u32()) * (1.0 / 4_294_967_296.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // reference values printed by a program compiled with fpc 3.2.2
    #[test]
    fn matches_free_pascal() {
        let mut r = PascalRandom::new(1);
        assert_eq!(
            [r.below(1000), r.below(1000), r.below(1000)],
            [417, 997, 720]
        );
        assert_eq!(format!("{:.17}", r.float()), "0.93255736120045185");
        assert_eq!(format!("{:.17}", r.float()), "0.00011438108049333");
        assert_eq!(format!("{:.17}", r.float()), "0.12812444777227938");
        assert_eq!(r.rand_seed, 4_294_967_294);

        // reseed and restore, like TSprite.Fire does for the Desert Eagles
        let save = r.rand_seed;
        r.rand_seed = 7;
        assert_eq!(r.below(100), 7);
        assert_eq!(format!("{:.17}", r.float()), "0.22733907494693995");
        r.rand_seed = save;
        assert_eq!(r.below(100), 52);
        assert_eq!(r.rand_seed, 1);

        // state regeneration after 624 outputs
        let mut r = PascalRandom::new(1);
        let sum: i32 = (0..700).map(|_| r.below(10)).sum();
        assert_eq!(sum, 3034);
        assert_eq!(r.below(1_000_000), 770_238);
    }
}
