//! The settings, in place of Soldat's launcher: the player, controls, graphics, sound and game
//! options of Soldat's config files (`configs/player.cfg`, `controls.cfg`, ...), shown by the
//! front end's settings screen (`front`) and saved back to those files in the config directory
//! (which `configs/client.cfg` runs at the next start).

use soldank_core::config::{Console, CvarValue, normalize_key};
use std::path::Path;

/// How a setting is changed.
pub enum Kind {
    /// Typed text, at most this long.
    Text(usize),
    Toggle,
    /// From `min` to `max` by `step`.
    Range(f64, f64, f64),
    /// The values, and what they're called.
    Choice(&'static [(&'static str, &'static str)]),
    Color,
    /// The key of a bound command (Soldat's name for it).
    Key(&'static str),
    /// The window's size: the cvar is the width, `r_screenheight` goes with it (after a
    /// restart).
    Resolution,
    /// The interface: Default or one of `custom-interfaces/`.
    Interface,
}

/// The window sizes the settings offer.
pub const RESOLUTIONS: [(i64, i64); 7] = [
    (1024, 768),
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

pub struct Setting {
    pub label: &'static str,
    /// The cvar (none for a key).
    pub cvar: &'static str,
    pub kind: Kind,
}

/// A page of settings, and the config file they're kept in.
pub struct Page {
    pub name: &'static str,
    pub file: &'static str,
    pub settings: &'static [Setting],
}

const fn cvar(label: &'static str, cvar: &'static str, kind: Kind) -> Setting {
    Setting { label, cvar, kind }
}

const fn key(label: &'static str, command: &'static str) -> Setting {
    Setting {
        label,
        cvar: "",
        kind: Kind::Key(command),
    }
}

const ON_OFF: Kind = Kind::Toggle;

pub const PAGES: &[Page] = &[
    Page {
        name: "Player",
        file: "player.cfg",
        settings: &[
            cvar("Name", "cl_player_name", Kind::Text(24)),
            cvar("Shirt", "cl_player_shirt", Kind::Color),
            cvar("Pants", "cl_player_pants", Kind::Color),
            cvar("Skin", "cl_player_skin", Kind::Color),
            cvar("Hair", "cl_player_hair", Kind::Color),
            cvar("Jets", "cl_player_jet", Kind::Color),
            cvar(
                "Hair style",
                "cl_player_hairstyle",
                Kind::Choice(&[
                    ("0", "Bald"),
                    ("1", "Dreadlocks"),
                    ("2", "Punk"),
                    ("3", "Mr. T"),
                    ("4", "Normal"),
                ]),
            ),
            cvar(
                "Headgear",
                "cl_player_headstyle",
                Kind::Choice(&[("0", "None"), ("1", "Helmet"), ("2", "Hat")]),
            ),
            cvar(
                "Chain",
                "cl_player_chainstyle",
                Kind::Choice(&[("0", "None"), ("1", "Silver"), ("2", "Gold")]),
            ),
            cvar(
                "Secondary weapon",
                "cl_player_secwep",
                Kind::Choice(&[
                    ("0", "USSOCOM"),
                    ("1", "Combat Knife"),
                    ("2", "Chainsaw"),
                    ("3", "LAW"),
                ]),
            ),
        ],
    },
    Page {
        name: "Controls",
        file: "controls.cfg",
        settings: &[
            cvar(
                "Mouse sensitivity",
                "cl_sensitivity",
                Kind::Range(0.1, 3.0, 0.05),
            ),
            key("Move left", "+left"),
            key("Move right", "+right"),
            key("Jump", "+jump"),
            key("Crouch", "+crouch"),
            key("Lie down", "+prone"),
            key("Fire", "+fire"),
            key("Jets", "+jet"),
            key("Change weapon", "+changeweapon"),
            key("Reload", "+reload"),
            key("Throw weapon", "+dropweapon"),
            key("Throw grenade", "+throwgrenade"),
            key("Throw flag", "+flagthrow"),
            key("Chat", "+chat"),
            key("Team chat", "+teamchat"),
            key("Command", "+cmd"),
            key("Radio", "+radio"),
            key("Weapons menu", "+weapons"),
            key("Scoreboard", "+fragslist"),
            key("Weapon stats", "+statsmenu"),
            key("Minimap", "+minimap"),
            key("Screenshot", "screenshot"),
        ],
    },
    Page {
        name: "Graphics",
        file: "graphics.cfg",
        settings: &[
            cvar(
                "Display",
                "r_fullscreen",
                Kind::Choice(&[("0", "Window"), ("1", "Fullscreen")]),
            ),
            cvar(
                "Window size (after a restart)",
                "r_screenwidth",
                Kind::Resolution,
            ),
            cvar("Interface", "ui_style", Kind::Interface),
            cvar("Background scenery", "r_renderbackground", ON_OFF),
            cvar("Plain sky", "r_forcebg", ON_OFF),
            cvar("Sky top", "r_forcebg_color1", Kind::Color),
            cvar("Sky bottom", "r_forcebg_color2", Kind::Color),
            cvar("Weather", "r_weathereffects", ON_OFF),
            cvar("Player indicator", "ui_playerindicator", ON_OFF),
            cvar("Kill list", "ui_killconsole", ON_OFF),
        ],
    },
    Page {
        name: "Sound",
        file: "sound.cfg",
        settings: &[
            cvar("Volume", "snd_volume", Kind::Range(0.0, 100.0, 5.0)),
            cvar("Battle effects", "snd_effects_battle", ON_OFF),
            cvar("Explosion effects", "snd_effects_explosions", ON_OFF),
        ],
    },
    Page {
        name: "Game",
        file: "game.cfg",
        settings: &[
            cvar("Screen shake", "cl_screenshake", ON_OFF),
            cvar("Screenshot after a match", "cl_endscreenshot", ON_OFF),
            cvar("Server mods", "cl_servermods", ON_OFF),
            cvar("Record a demo of each map", "demo_autorecord", ON_OFF),
        ],
    },
];

/// The taunt keys of `configs/bindings.cfg`: Alt with a digit or a letter.
pub fn taunt_keys() -> impl Iterator<Item = String> {
    ('0'..='9').chain('a'..='z').map(|c| format!("alt+{c}"))
}

/// The taunts a key can play besides a chat line: the player commands, and what they're called.
pub const TAUNT_ACTIONS: [(&str, &str); 6] = [
    ("smoke", "Light a cigar"),
    ("tabac", "Chew tobacco"),
    ("takeoff", "Helmet off and on"),
    ("victory", "Cheer"),
    ("piss", "Piss"),
    ("pwn", "Pwn"),
];

/// What a taunt key does.
#[derive(Debug, Clone, PartialEq)]
pub enum Taunt {
    None,
    /// Says this in the chat.
    Say(String),
    /// One of [`TAUNT_ACTIONS`].
    Action(&'static str),
    /// Some other command, kept as it is.
    Command(String),
}

/// What `key` does as a taunt.
pub fn taunt_of(console: &Console, key: &str) -> Taunt {
    let Some(command) = console.bindings.get(key) else {
        return Taunt::None;
    };
    // (the line as typed: spaces at its end too, while typing)
    if let Some(text) = command.trim_start().strip_prefix("say ") {
        // (bindings.cfg's quotes went with the reading, unless the binding kept some)
        let text = text
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .unwrap_or(text);
        return Taunt::Say(text.to_string());
    }
    let command = command.trim();
    if command == "say" {
        return Taunt::Say(String::new());
    }
    match TAUNT_ACTIONS.iter().find(|(name, _)| *name == command) {
        Some((name, _)) => Taunt::Action(name),
        None => Taunt::Command(command.to_string()),
    }
}

/// `key` does `taunt` now.
pub fn set_taunt(console: &mut Console, key: &str, taunt: &Taunt) {
    let command = match taunt {
        Taunt::None => {
            console.bindings.remove(key);
            return;
        }
        // (quotes can't be in a config line's quoted text)
        Taunt::Say(text) => format!("say {}", text.replace('"', "")),
        Taunt::Action(name) => name.to_string(),
        Taunt::Command(command) => command.clone(),
    };
    console.bindings.insert(key, command);
}

/// Writes `configs/bindings.cfg` in the config directory: the taunt keys as they are now, the
/// file's other lines as they were.
pub fn save_taunts(console: &Console, config_dir: &Path) -> std::io::Result<()> {
    let keys: Vec<String> = taunt_keys().collect();
    let mut lines: Vec<String> = config_text(console, config_dir, "bindings.cfg")
        .lines()
        .filter(|l| {
            first_word(l) != Some("bind")
                || !second_word(l).is_some_and(|k| keys.contains(&normalize_key(k)))
        })
        .map(str::to_string)
        .collect();
    for key in &keys {
        // Soldat's way: `"bind" "ALT+5" "say ""Medic!"""`
        let command = match taunt_of(console, key) {
            Taunt::None => continue,
            Taunt::Say(text) => format!("say \"{text}\""),
            Taunt::Action(name) => name.to_string(),
            Taunt::Command(command) => command,
        };
        lines.push(format!(
            "\"bind\" {} {}",
            quoted(&key_label(key)),
            quoted(&command)
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    let dir = config_dir.join("configs");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("bindings.cfg"), text)
}

/// A bound command and Soldat's name for it are the same action (`+dropweapon` is `+drop`).
fn same_action(bound: &str, command: &str) -> bool {
    let plain = |c: &str| match c.trim_start_matches('+') {
        "dropweapon" => "drop".to_string(),
        "throwgrenade" => "throw".to_string(),
        c => c.to_ascii_lowercase(),
    };
    plain(bound) == plain(command)
}

/// The keys bound to `command`.
pub fn keys_of(console: &Console, command: &str) -> Vec<String> {
    let mut keys: Vec<String> = console
        .bindings
        .iter()
        .filter(|(_, bound)| same_action(bound, command))
        .map(|(key, _)| key.to_string())
        .collect();
    keys.sort();
    keys
}

/// `command` goes on `key` alone (the keys it was on are free).
pub fn bind(console: &mut Console, command: &str, key: &str) {
    for bound in keys_of(console, command) {
        console.bindings.remove(&bound);
    }
    console.bindings.insert(key, command.to_string());
}

/// A key as Soldat's controls.cfg names it (`MOUSE1`, `/`).
pub fn key_label(key: &str) -> String {
    match key {
        "slash" => "/".to_string(),
        key => key.to_uppercase(),
    }
}

/// A cvar's value as a number.
pub fn number(console: &Console, name: &str) -> f64 {
    match console.cvars.get(name).map(|c| &c.value) {
        Some(CvarValue::Int(v)) => *v as f64,
        Some(CvarValue::Float(v)) => f64::from(*v),
        _ => 0.0,
    }
}

/// A config file's text: the config directory's, else the game's.
fn config_text(console: &Console, config_dir: &Path, file: &str) -> String {
    std::fs::read_to_string(config_dir.join("configs").join(file))
        .ok()
        .or_else(|| {
            console
                .fallback_files
                .get(&format!("configs/{file}"))
                .cloned()
        })
        .unwrap_or_default()
}

/// A value in Soldat's config syntax: quoted, quotes doubled, colours `$00RRGGBB`.
fn quoted(value: &str) -> String {
    let value = match value.strip_prefix('$') {
        Some(hex) if hex.len() == 6 => format!("$00{hex}"),
        _ => value.to_string(),
    };
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// The first quoted word of a config line.
fn first_word(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// The second quoted word (a bind line's key).
fn second_word(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('"')?;
    let rest = &rest[rest.find('"')? + 1..];
    let rest = rest.trim_start().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// Writes `page`'s file in the config directory: its lines with the page's cvars as they are
/// now (the rest as it was); for the controls, all key bindings but `bindings.cfg`'s.
pub fn save_page(console: &Console, config_dir: &Path, page: &Page) -> std::io::Result<()> {
    let text = save_text(console, config_dir, page);
    let dir = config_dir.join("configs");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(page.file), text)
}

fn save_text(console: &Console, config_dir: &Path, page: &Page) -> String {
    // (the page's cvars this game has; the height with the width)
    let cvars: Vec<&str> = page
        .settings
        .iter()
        .flat_map(|s| match s.kind {
            Kind::Resolution => vec![s.cvar, "r_screenheight"],
            _ => vec![s.cvar],
        })
        .filter(|c| console.cvars.get(c).is_some())
        .collect();
    let value = |name: &str| {
        console
            .cvars
            .get(name)
            .map_or(String::new(), |c| quoted(&c.value.to_string()))
    };
    let controls = page.settings.iter().any(|s| matches!(s.kind, Kind::Key(_)));
    let mut written = Vec::new();
    let mut lines = Vec::new();
    for line in config_text(console, config_dir, page.file).lines() {
        match first_word(line) {
            Some(name) if cvars.contains(&name) => {
                lines.push(format!("\"{name}\" {}", value(name)));
                written.push(name.to_string());
            }
            // the controls' bindings are written anew below
            Some("bind") if controls => {}
            _ => lines.push(line.to_string()),
        }
    }
    for name in cvars.iter().filter(|c| !written.iter().any(|w| w == *c)) {
        lines.push(format!("\"{name}\" {}", value(name)));
    }
    if controls {
        // bindings.cfg keeps its own (the taunts)
        let theirs: Vec<String> = config_text(console, config_dir, "bindings.cfg")
            .lines()
            .filter(|l| first_word(l) == Some("bind"))
            .filter_map(|l| second_word(l).map(normalize_key))
            .collect();
        let mut binds: Vec<(&str, &str)> = console
            .bindings
            .iter()
            .filter(|(key, _)| !theirs.iter().any(|t| t == key))
            .collect();
        binds.sort();
        for (key, command) in binds {
            lines.push(format!("\"bind\" {} {}", quoted(key), quoted(command)));
        }
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use soldank_core::config::{Cvar, Cvars};

    fn console() -> Console {
        let mut cvars = Cvars::new();
        cvars.register(Cvar::string("cl_player_name", "Major", ""));
        cvars.register(Cvar::color("cl_player_shirt", 0x304289, ""));
        cvars.register(Cvar::int("cl_player_hairstyle", 0, ""));
        cvars.register(Cvar::float("cl_sensitivity", 0.8, ""));
        cvars.register(Cvar::bool("cl_screenshake", true, ""));
        let mut console = Console::new(cvars);
        console.bindings.insert("a", "+left".into());
        console.bindings.insert("f", "+dropweapon".into());
        console.bindings.insert("alt+1", "smoke".into());
        console
    }

    #[test]
    fn keys_bind_in_place_of_the_old_ones() {
        let mut console = console();
        assert_eq!(keys_of(&console, "+drop"), ["f"]);
        bind(&mut console, "+dropweapon", "g");
        assert_eq!(keys_of(&console, "+dropweapon"), ["g"]);
        assert_eq!(console.bindings.get("f"), None);
        assert_eq!(key_label("mouse1"), "MOUSE1");
        assert_eq!(key_label("slash"), "/");
        assert!((number(&console, "cl_sensitivity") - 0.8).abs() < 1e-6);
    }

    #[test]
    fn pages_save_in_soldats_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let mut console = console();
        console.fallback_files.insert(
            "configs/player.cfg".into(),
            "// mine\n\"cl_player_name\" \"Major\"\n\"cl_player_hair\" \"$00000000\"\n".into(),
        );
        console.fallback_files.insert(
            "configs/bindings.cfg".into(),
            "\"bind\" \"ALT+1\" \"smoke\"\n".into(),
        );
        console.cvars.set("cl_player_name", "Dutch \"D\"").unwrap();
        save_page(&console, dir.path(), &PAGES[0]).unwrap();
        let text = std::fs::read_to_string(dir.path().join("configs/player.cfg")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "// mine");
        assert_eq!(lines[1], "\"cl_player_name\" \"Dutch \"\"D\"\"\"");
        assert_eq!(lines[2], "\"cl_player_hair\" \"$00000000\"");
        assert!(lines.contains(&"\"cl_player_shirt\" \"$00304289\""));

        save_page(&console, dir.path(), &PAGES[1]).unwrap();
        let text = std::fs::read_to_string(dir.path().join("configs/controls.cfg")).unwrap();
        assert!(text.contains("\"cl_sensitivity\" \"0.8\""));
        assert!(text.contains("\"bind\" \"a\" \"+left\""));
        assert!(text.contains("\"bind\" \"f\" \"+dropweapon\""));
        // the taunt stays bindings.cfg's
        assert!(!text.contains("smoke"));
    }

    #[test]
    fn taunts_save_in_soldats_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let mut console = console();
        console.fallback_files.insert(
            "configs/bindings.cfg".into(),
            "// taunts\n\"bind\" \"ALT+1\" \"smoke\"\n\"bind\" \"F9\" \"kill\"\n".into(),
        );
        // as reading `"bind" "ALT+5" "say ""Medic!"""` leaves it
        console.bindings.insert("alt+5", "say Medic!".into());
        assert_eq!(taunt_of(&console, "alt+1"), Taunt::Action("smoke"));
        assert_eq!(taunt_of(&console, "alt+5"), Taunt::Say("Medic!".into()));
        assert_eq!(taunt_of(&console, "alt+6"), Taunt::None);

        set_taunt(
            &mut console,
            "alt+5",
            &Taunt::Say("Get the \"flag\"!".into()),
        );
        set_taunt(&mut console, "alt+a", &Taunt::Action("victory"));
        set_taunt(&mut console, "alt+1", &Taunt::None);
        save_taunts(&console, dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join("configs/bindings.cfg")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "// taunts",
                "\"bind\" \"F9\" \"kill\"",
                "\"bind\" \"ALT+5\" \"say \"\"Get the flag!\"\"\"",
                "\"bind\" \"ALT+A\" \"victory\"",
            ]
        );
    }
}
