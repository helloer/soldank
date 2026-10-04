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
mod background;
mod bots;
mod bullet;
mod calc;
pub mod config;
mod control;
mod data;
pub mod demo;
mod effects;
mod extended;
mod game;
mod health;
mod input;
mod mapfile;
pub mod net;
mod particles;
mod random;
mod soldier;
mod sound;
pub mod sprites;
mod state;
mod things;
mod weapons;
mod world;

pub use anims::*;
pub use background::*;
pub use bots::*;
pub use bullet::*;
pub use calc::*;
pub use control::*;
pub use data::*;
pub use effects::*;
pub use extended::Ext;
pub use game::*;
pub use health::*;
pub use input::*;
pub use mapfile::*;
pub use particles::*;
pub use random::PascalRandom;
pub use soldier::*;
pub use sound::*;
pub use state::*;
pub use things::*;
pub use weapons::*;
pub use world::*;

/// Simulation rate.
pub const TICKS_PER_SECOND: u32 = 60;
