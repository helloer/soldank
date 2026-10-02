use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum Team {
    #[default]
    None,
    Alpha,
    Bravo,
    Charlie,
    Delta,
}

#[derive(Debug, Copy, Clone)]
pub enum EmitterItem {
    Bullet(BulletParams),
    /// The soldier killed itself (fall damage).
    Died(DeathKind),
}
