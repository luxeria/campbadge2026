//! A small deterministic pseudo-random number generator for game logic.

/// A xorshift32 random number generator.
///
/// Deliberately kept dependency free and cheap so games can get varied,
/// non-blocking behaviour without pulling a heavyweight RNG. Not suitable for
/// cryptographic use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prng(u32);

impl Prng {
    /// Default seed used when an explicit seed of zero is supplied.
    const FALLBACK_SEED: u32 = 0x9e37_79b9;

    /// Creates a generator from a seed.
    ///
    /// A zero seed is remapped so the generator never enters the all-zeros
    /// state, which would otherwise produce a constant stream of zeroes.
    pub const fn new(seed: u32) -> Self {
        if seed == 0 {
            Prng(Self::FALLBACK_SEED)
        } else {
            Prng(seed)
        }
    }

    /// Returns the next 32-bit value from the sequence.
    pub fn next_u32(&mut self) -> u32 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        self.0 = state;
        state
    }

    /// Returns a value uniformly distributed in `0..bound`.
    ///
    /// The modulo introduces a small bias that is irrelevant for games.
    pub fn next_range(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        self.next_u32() % bound
    }

    /// Returns a value in the half-open unit interval `[0, 1)`.
    pub fn next_unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::Prng;

    #[test]
    fn seeded_sequence_is_reproducible() {
        let mut first = Prng::new(12345);
        let mut second = Prng::new(12345);
        for _ in 0..64 {
            assert_eq!(first.next_u32(), second.next_u32());
        }
    }

    #[test]
    fn zero_seed_does_not_degenerate() {
        let mut generator = Prng::new(0);
        let first = generator.next_u32();
        let second = generator.next_u32();
        assert_ne!(first, 0);
        assert_ne!(second, 0);
    }

    #[test]
    fn unit_values_stay_within_range() {
        let mut generator = Prng::new(7);
        for _ in 0..256 {
            let value = generator.next_unit();
            assert!((0.0..1.0).contains(&value));
        }
    }

    #[test]
    fn bounded_values_respect_bound() {
        let mut generator = Prng::new(42);
        for _ in 0..256 {
            assert!(generator.next_range(10) < 10);
        }
    }
}
