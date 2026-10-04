//! Sparks (`CreateSpark`): the client's particles for smoke, blood, shells, explosions and
//! the like. The simulation asks for them from the same places as Soldat's client code.
//! The dice those places roll come from a generator of their own ([`fx`]), never from the
//! world's, which stays bit-compatible with the server.

use super::*;
use std::cell::RefCell;

/// `r_maxsparks` at most (Soldat's `MAX_SPARKS` is one more, slot 0 unused).
pub const MAX_SPARKS: i32 = 557;

/// Who a spark belongs to: its colours (shirt, jets) and the polygons it collides with
/// come from its soldier. Sparks without one fly through walls.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SparkOwner {
    None,
    /// The soldier whose update created it.
    Me,
    Soldier(SoldierId),
}

/// A spark to create.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SparkSpawn {
    pub pos: Vec2,
    pub velocity: Vec2,
    /// Soldat's spark style (1-73), which says how it looks and behaves.
    pub style: u8,
    pub owner: SparkOwner,
    /// Ticks to live.
    pub life: u8,
}

impl SparkSpawn {
    pub fn new(pos: Vec2, velocity: Vec2, style: u8, owner: SparkOwner, life: i32) -> SparkSpawn {
        SparkSpawn {
            pos,
            velocity,
            style,
            owner,
            life: life.clamp(0, 255) as u8,
        }
    }
}

thread_local! {
    static FX: RefCell<PascalRandom> = RefCell::new(PascalRandom::new(0x5eed));
}

/// The generator of the client's dice (`Random` in client-only code).
pub mod fx {
    use super::*;

    /// `Random(n)`
    pub fn random(n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        FX.with(|r| r.borrow_mut().below(n))
    }

    /// `Random`: in [0, 1).
    pub fn random_float() -> f32 {
        FX.with(|r| r.borrow_mut().float()) as f32
    }

    /// `RandomRange(a, b)`: from the smaller to just below the larger.
    pub fn random_range(a: i32, b: i32) -> i32 {
        random((a - b).abs()) + a.min(b)
    }
}

/// Soldat's `MAX_SPARKS`: `r_maxsparks` is compared with it.
pub(crate) const SPARK_SLOTS: i32 = 558;

/// `r_maxsparks > MAX_SPARKS - 10`: the extra effects are on.
pub(crate) fn many_sparks(config: &WorldConfig) -> bool {
    config.max_sparks > SPARK_SLOTS - 10
}

/// `r_maxsparks < MAX_SPARKS - 10`: effects are thinned out.
pub(crate) fn few_sparks(config: &WorldConfig) -> bool {
    config.max_sparks < SPARK_SLOTS - 10
}

impl Soldier {
    /// `CreateSpark(pos, velocity, style, Num, life)`
    pub(crate) fn spark(&mut self, pos: Vec2, velocity: Vec2, style: u8, life: i32) {
        self.sparks
            .push(SparkSpawn::new(pos, velocity, style, SparkOwner::Me, life));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_range_is_half_open() {
        for _ in 0..100 {
            let r = fx::random_range(4, 5);
            assert_eq!(r, 4);
            let r = fx::random_range(1, 3);
            assert!((1..3).contains(&r));
        }
        assert_eq!(fx::random(0), 0);
    }
}
