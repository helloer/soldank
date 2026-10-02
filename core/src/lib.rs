//! Soldank game simulation.
//!
//! Everything here is independent of rendering, audio, windowing and networking. The
//! entry point is [`World`], advanced in fixed 60 Hz ticks with [`World::step`], which
//! takes per-soldier [`Input`] and returns the [`GameEvent`]s that happened.

macro_rules! iif(
    ($cond:expr, $then:expr, $else:expr) => (if $cond { $then } else { $else })
);

mod anims;
pub mod assets;
mod bullet;
mod calc;
pub mod config;
mod control;
mod data;
mod input;
mod mapfile;
mod particles;
mod soldier;
pub mod sprites;
mod state;
mod weapons;
mod world;

pub use anims::*;
pub use bullet::*;
pub use calc::*;
pub use control::*;
pub use data::*;
pub use input::*;
pub use mapfile::*;
pub use particles::*;
pub use soldier::*;
pub use state::*;
pub use weapons::*;
pub use world::*;

/// Simulation rate.
pub const TICKS_PER_SECOND: u32 = 60;
