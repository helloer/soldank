use super::{ConfigError, Result};
use bitflags::bitflags;
use std::collections::BTreeMap;
use std::fmt;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct CvarFlags: u8 {
        /// Client-side setting.
        const CLIENT = 1 << 0;
        /// Server/game rule setting.
        const SERVER = 1 << 1;
        /// Replicated from server to clients.
        const SYNC = 1 << 2;
        /// Only changeable before the game starts (command line / config files).
        const INIT_ONLY = 1 << 3;
        /// Debug/cheat setting.
        const CHEAT = 1 << 4;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CvarValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    /// 0xRRGGBB
    Color(u32),
}

impl fmt::Display for CvarValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CvarValue::Bool(v) => write!(f, "{}", u8::from(*v)),
            CvarValue::Int(v) => write!(f, "{v}"),
            CvarValue::Float(v) => write!(f, "{v}"),
            CvarValue::Str(v) => write!(f, "{v}"),
            CvarValue::Color(v) => write!(f, "${v:06X}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cvar {
    pub name: String,
    pub description: String,
    pub flags: CvarFlags,
    pub default: CvarValue,
    pub value: CvarValue,
    pub range: Option<(f64, f64)>,
}

impl Cvar {
    fn new(name: &str, description: &str, default: CvarValue) -> Cvar {
        Cvar {
            name: name.to_owned(),
            description: description.to_owned(),
            flags: CvarFlags::empty(),
            value: default.clone(),
            default,
            range: None,
        }
    }

    pub fn bool(name: &str, default: bool, description: &str) -> Cvar {
        Cvar::new(name, description, CvarValue::Bool(default))
    }

    pub fn int(name: &str, default: i64, description: &str) -> Cvar {
        Cvar::new(name, description, CvarValue::Int(default))
    }

    pub fn float(name: &str, default: f32, description: &str) -> Cvar {
        Cvar::new(name, description, CvarValue::Float(default))
    }

    pub fn string(name: &str, default: &str, description: &str) -> Cvar {
        Cvar::new(name, description, CvarValue::Str(default.to_owned()))
    }

    pub fn color(name: &str, default: u32, description: &str) -> Cvar {
        Cvar::new(name, description, CvarValue::Color(default))
    }

    pub fn flags(mut self, flags: CvarFlags) -> Cvar {
        self.flags = flags;
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Cvar {
        self.range = Some((min, max));
        self
    }

    fn parse(&self, input: &str) -> Result<CvarValue> {
        let invalid = |expected| ConfigError::InvalidValue {
            name: self.name.clone(),
            value: input.to_owned(),
            expected,
        };

        let value = match self.default {
            CvarValue::Bool(_) => match input.to_ascii_lowercase().as_str() {
                "1" | "true" | "on" | "yes" => CvarValue::Bool(true),
                "0" | "false" | "off" | "no" => CvarValue::Bool(false),
                _ => return Err(invalid("0 or 1")),
            },
            CvarValue::Int(_) => CvarValue::Int(input.parse().map_err(|_| invalid("an integer"))?),
            CvarValue::Float(_) => {
                CvarValue::Float(input.parse().map_err(|_| invalid("a number"))?)
            }
            CvarValue::Str(_) => CvarValue::Str(input.to_owned()),
            CvarValue::Color(_) => {
                CvarValue::Color(parse_color(input).ok_or(invalid("a color like $RRGGBB"))?)
            }
        };

        if let Some((min, max)) = self.range {
            let n = match value {
                CvarValue::Int(v) => Some(v as f64),
                CvarValue::Float(v) => Some(f64::from(v)),
                CvarValue::Str(ref s) => Some(s.chars().count() as f64),
                _ => None,
            };

            if let Some(n) = n.filter(|n| *n < min || *n > max) {
                return Err(ConfigError::OutOfRange {
                    name: self.name.clone(),
                    value: n,
                    min,
                    max,
                });
            }
        }

        Ok(value)
    }
}

fn parse_color(input: &str) -> Option<u32> {
    let hex = input
        .strip_prefix('$')
        .or_else(|| input.strip_prefix('#'))
        .or_else(|| input.strip_prefix("0x"))
        .or_else(|| input.strip_prefix("0X"));

    let value = match hex {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => input.parse().ok()?,
    };

    // Soldat stores colors as $AARRGGBB/$00RRGGBB; keep the RGB part.
    Some(value & 0x00FF_FFFF)
}

/// Registry of all console variables, sorted by name.
#[derive(Debug, Default)]
pub struct Cvars {
    vars: BTreeMap<String, Cvar>,
    started: bool,
}

impl Cvars {
    pub fn new() -> Cvars {
        Cvars::default()
    }

    /// Registers a cvar. Registering the same name twice is a programming error.
    pub fn register(&mut self, cvar: Cvar) {
        let previous = self.vars.insert(cvar.name.clone(), cvar);
        debug_assert!(previous.is_none(), "cvar registered twice");
    }

    /// After this, `INIT_ONLY` cvars can no longer be changed.
    pub fn mark_started(&mut self) {
        self.started = true;
    }

    pub fn get(&self, name: &str) -> Option<&Cvar> {
        self.vars.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Cvar> {
        self.vars.values()
    }

    pub fn set(&mut self, name: &str, input: &str) -> Result<()> {
        let started = self.started;
        let cvar = self
            .vars
            .get_mut(name)
            .ok_or_else(|| ConfigError::Unknown(name.to_owned()))?;

        if started && cvar.flags.contains(CvarFlags::INIT_ONLY) {
            return Err(ConfigError::InitOnly(name.to_owned()));
        }

        cvar.value = cvar.parse(input)?;
        Ok(())
    }

    pub fn reset(&mut self, name: &str) -> Result<()> {
        let cvar = self
            .vars
            .get_mut(name)
            .ok_or_else(|| ConfigError::Unknown(name.to_owned()))?;
        cvar.value = cvar.default.clone();
        Ok(())
    }

    fn value(&self, name: &str) -> &CvarValue {
        match self.vars.get(name) {
            Some(cvar) => &cvar.value,
            None => panic!("cvar {name} is not registered"),
        }
    }

    pub fn bool(&self, name: &str) -> bool {
        match self.value(name) {
            CvarValue::Bool(v) => *v,
            other => panic!("cvar {name} is not a bool: {other:?}"),
        }
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.value(name) {
            CvarValue::Int(v) => *v,
            other => panic!("cvar {name} is not an int: {other:?}"),
        }
    }

    pub fn float(&self, name: &str) -> f32 {
        match self.value(name) {
            CvarValue::Float(v) => *v,
            other => panic!("cvar {name} is not a float: {other:?}"),
        }
    }

    pub fn string(&self, name: &str) -> &str {
        match self.value(name) {
            CvarValue::Str(v) => v,
            other => panic!("cvar {name} is not a string: {other:?}"),
        }
    }

    /// Returns (r, g, b).
    pub fn color(&self, name: &str) -> (u8, u8, u8) {
        match self.value(name) {
            CvarValue::Color(v) => ((v >> 16) as u8, (v >> 8) as u8, *v as u8),
            other => panic!("cvar {name} is not a color: {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cvars() -> Cvars {
        let mut cvars = Cvars::new();
        cvars.register(Cvar::float("sv_gravity", 0.06, "Gravity"));
        cvars.register(Cvar::bool("sv_realisticmode", false, "").flags(CvarFlags::INIT_ONLY));
        cvars.register(Cvar::int("r_maxsparks", 557, "").range(0.0, 557.0));
        cvars.register(Cvar::string("cl_player_name", "Major", "").range(1.0, 24.0));
        cvars.register(Cvar::color("cl_player_skin", 0xE6B478, ""));
        cvars
    }

    #[test]
    fn parses_typed_values() {
        let mut cvars = cvars();
        cvars.set("sv_gravity", "0.1").unwrap();
        cvars.set("sv_realisticmode", "on").unwrap();
        cvars.set("cl_player_skin", "$00FF8000").unwrap();

        assert_eq!(cvars.float("sv_gravity"), 0.1);
        assert!(cvars.bool("sv_realisticmode"));
        assert_eq!(cvars.color("cl_player_skin"), (255, 128, 0));
        assert_eq!(
            cvars.get("cl_player_skin").unwrap().value.to_string(),
            "$FF8000"
        );
    }

    #[test]
    fn rejects_invalid_and_out_of_range_values() {
        let mut cvars = cvars();
        assert!(matches!(
            cvars.set("sv_gravity", "heavy"),
            Err(ConfigError::InvalidValue { .. })
        ));
        assert!(matches!(
            cvars.set("r_maxsparks", "9000"),
            Err(ConfigError::OutOfRange { .. })
        ));
        assert!(matches!(
            cvars.set("cl_player_name", ""),
            Err(ConfigError::OutOfRange { .. })
        ));
        assert_eq!(
            cvars.set("nope", "1"),
            Err(ConfigError::Unknown("nope".into()))
        );
        assert_eq!(cvars.int("r_maxsparks"), 557);
    }

    #[test]
    fn init_only_cvars_lock_after_start() {
        let mut cvars = cvars();
        cvars.mark_started();
        assert_eq!(
            cvars.set("sv_realisticmode", "1"),
            Err(ConfigError::InitOnly("sv_realisticmode".into()))
        );
        cvars.set("sv_gravity", "0.2").unwrap();
        cvars.reset("sv_gravity").unwrap();
        assert_eq!(cvars.float("sv_gravity"), 0.06);
    }
}
