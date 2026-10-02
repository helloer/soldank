use super::*;
use gfx2d::mq::{KeyCode, MouseButton};
use soldank_core::config::Bindings;

/// Default key bindings, executed before the user's `autoexec.cfg`.
pub const DEFAULT_BINDINGS: &str = "
bind a +left; bind d +right; bind w +jump; bind s +crouch; bind x +prone
bind mouse1 +fire; bind mouse2 +jet
bind q +changeweapon; bind e +throw; bind f +drop
bind kpadd +zoomin; bind kpsubtract +zoomout
bind tab cycleweapon; bind escape quit
";

/// Binding name of a key: lowercase variant name, digits without the `key` prefix
/// (`a`, `space`, `kpadd`, `leftshift`, `1`).
pub fn key_name(keycode: KeyCode) -> String {
    let name = format!("{keycode:?}").to_lowercase();
    match name.strip_prefix("key") {
        Some(digit) if digit.len() == 1 => digit.to_owned(),
        _ => name,
    }
}

pub fn mouse_button_name(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "mouse1".to_owned(),
        MouseButton::Right => "mouse2".to_owned(),
        MouseButton::Middle => "mouse3".to_owned(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Client-side actions bound with `+action`, besides the gameplay [`Buttons`].
#[derive(Debug, Default)]
pub struct InputState {
    pub buttons: Buttons,
    pub zoom_in: bool,
    pub zoom_out: bool,
}

impl InputState {
    /// Handles a key or mouse button transition. Returns a console command to run for
    /// non-`+action` bindings (only on press).
    pub fn handle(&mut self, bindings: &Bindings, key: &str, pressed: bool) -> Option<String> {
        let command = bindings.get(key)?;

        let Some(action) = command.strip_prefix('+') else {
            return pressed.then(|| command.to_owned());
        };

        if let Some(button) = Buttons::from_action(action) {
            self.buttons.set(button, pressed);
        } else {
            match action {
                "zoomin" => self.zoom_in = pressed,
                "zoomout" => self.zoom_out = pressed,
                _ => tracing::warn!(action, "unknown action"),
            }
        }

        None
    }

    pub fn zoom_dir(&self) -> f32 {
        match (self.zoom_in, self.zoom_out) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }
}
