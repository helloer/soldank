//! Console variables (cvars), a Quake/Soldat style command language and key bindings.
//!
//! Lines look like `sv_gravity 0.06; bind space +jump // comment`. A line is split on
//! `;` into commands, each command into whitespace separated tokens (double quotes group
//! tokens). The first token is resolved, in order, as a builtin or registered command,
//! an alias, or a cvar (`name` prints it, `name value` sets it).

mod console;
mod cvar;
mod parse;

pub use console::{Bindings, Console, Deferred, normalize_key};
pub use cvar::{Cvar, CvarFlags, CvarValue, Cvars};
pub use parse::{split_commands, tokenize};

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ConfigError {
    #[error("unknown command or cvar \"{0}\"")]
    Unknown(String),
    #[error("invalid value \"{value}\" for {name}: expected {expected}")]
    InvalidValue {
        name: String,
        value: String,
        expected: &'static str,
    },
    #[error("value {value} for {name} is out of range [{min}, {max}]")]
    OutOfRange {
        name: String,
        value: f64,
        min: f64,
        max: f64,
    },
    #[error("{0} can only be set at startup")]
    InitOnly(String),
    #[error("usage: {0}")]
    Usage(&'static str),
    #[error("cannot exec {path}: {reason}")]
    Exec { path: String, reason: String },
    #[error("alias recursion limit reached while running \"{0}\"")]
    Recursion(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
