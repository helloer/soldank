use super::*;
use gfx2d::mq::{KeyCode, MouseButton};
use soldank_core::config::Bindings;

/// Default key bindings, executed before the user's `autoexec.cfg`.
pub const DEFAULT_BINDINGS: &str = "
bind a +left; bind d +right; bind w +jump; bind s +crouch; bind x +prone
bind mouse1 +fire; bind mouse3 +jet
bind q +changeweapon; bind e +throw; bind f +drop; bind g +flagthrow; bind r +reload
bind alt+1 smoke; bind alt+2 tabac; bind alt+3 takeoff; bind alt+4 victory
bind kpadd +zoomin; bind kpsubtract +zoomout
bind tab weapons; bind m changeteam; bind v radio
bind t chat; bind y teamchat; bind slash cmd
bind f1 fragslist; bind f2 statsmenu; bind f3 minimap; bind f4 screenshot
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

/// What a key types unshifted, for the letters, digits and slash.
pub fn key_char(keycode: KeyCode) -> Option<char> {
    let name = key_name(keycode);
    match name.as_str() {
        "slash" => Some('/'),
        _ if name.chars().count() == 1 => name.chars().next(),
        _ => None,
    }
}

/// Binding name of a mouse button, numbered like SDL (and Soldat's configs): `mouse1`
/// left, `mouse2` middle, `mouse3` right.
pub fn mouse_button_name(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "mouse1".to_owned(),
        MouseButton::Middle => "mouse2".to_owned(),
        MouseButton::Right => "mouse3".to_owned(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Client-side actions bound with `+action`, besides the gameplay [`Buttons`], and what the
/// keyboard and mouse left for the next event.
#[derive(Debug, Default)]
pub struct InputState {
    pub buttons: Buttons,
    pub zoom_in: bool,
    pub zoom_out: bool,
    /// A key just opened the chat: its character isn't typed into it.
    pub skip_char: bool,
    /// The character of a key while not typing, since the last frame (X11 hands it over
    /// before the key), and the character of the key being pressed.
    pub batch_char: Option<char>,
    pub key_char: Option<char>,
    /// Last OS cursor position: the game cursor moves by deltas so recoil can push it.
    pub last_mouse: Option<Vec2>,
}

impl InputState {
    /// Handles a key or mouse button transition. Returns a console command to run for
    /// non-`+action` bindings (only on press).
    pub fn handle(&mut self, bindings: &Bindings, key: &str, pressed: bool) -> Option<String> {
        let command = bindings.get(key)?;

        let Some(action) = command.strip_prefix('+') else {
            return pressed.then(|| command.to_owned());
        };
        // Soldat's names for the same actions (controls.cfg)
        let action = match action {
            "dropweapon" => "drop",
            "throwgrenade" => "throw",
            action => action,
        };

        if let Some(button) = Buttons::from_action(action) {
            self.buttons.set(button, pressed);
        } else {
            match action {
                "zoomin" => self.zoom_in = pressed,
                "zoomout" => self.zoom_out = pressed,
                // Soldat's other actions are commands on key press
                _ => return pressed.then(|| action.to_owned()),
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
