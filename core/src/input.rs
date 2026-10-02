use super::*;
use bitflags::bitflags;

bitflags! {
    /// Buttons held during a tick.
    #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct Buttons: u16 {
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
        const JUMP = 1 << 2;
        const CROUCH = 1 << 3;
        const FIRE = 1 << 4;
        const JETS = 1 << 5;
        const CHANGE_WEAPON = 1 << 6;
        const THROW = 1 << 7;
        const DROP = 1 << 8;
        const PRONE = 1 << 9;
    }
}

impl Buttons {
    /// Maps an action name as used in key bindings (`+jump` without the `+`).
    pub fn from_action(action: &str) -> Option<Buttons> {
        Some(match action {
            "left" => Buttons::LEFT,
            "right" => Buttons::RIGHT,
            "jump" => Buttons::JUMP,
            "crouch" => Buttons::CROUCH,
            "fire" => Buttons::FIRE,
            "jet" => Buttons::JETS,
            "changeweapon" => Buttons::CHANGE_WEAPON,
            "throw" => Buttons::THROW,
            "drop" => Buttons::DROP,
            "prone" => Buttons::PRONE,
            _ => return None,
        })
    }
}

/// Everything a player controls in one tick. This is what gets sent over the network
/// and recorded in demos.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Input {
    pub buttons: Buttons,
    /// Aim point in world coordinates.
    pub aim: Vec2,
}

impl Soldier {
    pub fn apply_input(&mut self, input: &Input) {
        let b = input.buttons;
        let c = &mut self.control;

        c.left = b.contains(Buttons::LEFT);
        c.right = b.contains(Buttons::RIGHT);
        c.up = b.contains(Buttons::JUMP);
        c.down = b.contains(Buttons::CROUCH);
        c.fire = b.contains(Buttons::FIRE);
        c.jets = b.contains(Buttons::JETS);
        c.change_weapon = b.contains(Buttons::CHANGE_WEAPON);
        c.throw_nade = b.contains(Buttons::THROW);
        c.throw_weapon = b.contains(Buttons::DROP);
        c.prone = b.contains(Buttons::PRONE);
        c.mouse_aim_x = input.aim.x.round() as i32;
        c.mouse_aim_y = input.aim.y.round() as i32;
    }
}
