use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum Team {
    #[default]
    None,
    Alpha,
    Bravo,
    Charlie,
    Delta,
    /// `TEAM_SPECTATOR`: a player without a body, always dead, far off the map.
    Spectator,
}

#[derive(Debug, Copy, Clone)]
pub enum EmitterItem {
    Bullet(BulletParams),
    /// The soldier killed itself (fall damage).
    Died(DeathKind),
}
