//! Interface layouts (`ui_style`): where the HUD's icons, bars and texts go. The default
//! one (`LoadDefaultInterfaceData`) or a custom one from
//! `custom-interfaces/<name>/setup.sif`, a raw dump of Soldat's `TInterface` record.

use soldank_core::assets::Vfs;

/// `RenderBar`'s `PosType`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BarStyle {
    /// `TEXTSTYLE`: a percentage instead of a bar.
    Text,
    Horizontal,
    Vertical,
}

impl BarStyle {
    fn from_byte(b: u8) -> BarStyle {
        match b {
            0 => BarStyle::Text,
            2 => BarStyle::Vertical,
            _ => BarStyle::Horizontal,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Icon {
    pub x: f32,
    pub y: f32,
    /// Degrees.
    pub rotate: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Bar {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub style: BarStyle,
    /// Degrees.
    pub rotate: f32,
}

/// Which parts are shown.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Shown {
    pub health: bool,
    pub ammo: bool,
    pub vest: bool,
    pub jet: bool,
    pub nades: bool,
    pub bullets: bool,
    pub weapon: bool,
    pub fire: bool,
    pub team: bool,
    pub ping: bool,
    pub status: bool,
}

/// `IntAlign`: right-aligned (`1`) parts.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RightAligned {
    pub weapon: bool,
    pub bullets: bool,
    pub health_bar: bool,
    pub ammo_bar: bool,
    pub reload_bar: bool,
    pub fire_bar: bool,
    pub jet_bar: bool,
    pub vest_bar: bool,
}

/// `TInterfaceRelInfo`: the icon each group of parts moves with when the screen is wider
/// than 640 (the parts keep their offset from it).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Anchors {
    pub health_bar: (f32, f32),
    pub jet_bar: (f32, f32),
    pub ammo_bar: (f32, f32),
    pub fire_bar: (f32, f32),
    pub nades_bar: (f32, f32),
}

/// `TInterface` with `IntAlign` and `TInterfaceRelInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The custom interface's directory name, `None` for the default one.
    pub name: Option<String>,
    pub alpha: u8,
    pub shown: Shown,
    pub health_ico: Icon,
    pub health_bar: Bar,
    pub ammo_ico: Icon,
    pub ammo_bar: Bar,
    pub jet_ico: Icon,
    pub jet_bar: Bar,
    pub vest_bar: Bar,
    pub nades: Bar,
    pub bullets: (f32, f32),
    pub weapon: (f32, f32),
    pub fire_ico: Icon,
    pub fire_bar: Bar,
    pub team_box: (f32, f32),
    pub ping: (f32, f32),
    pub status: (f32, f32),
    pub right: RightAligned,
    pub anchors: Anchors,
}

/// The interface sprites a custom interface may replace (`CUSTOM_FIRST..CUSTOM_LAST`).
pub const CUSTOM_FILES: &[&str] = &[
    "cursor",
    "health",
    "ammo",
    "jet",
    "health-bar",
    "jet-bar",
    "reload-bar",
    "back",
    "overlay",
    "nade",
    "noflag",
    "flag",
    "cluster-nade",
    "dot",
    "fire-bar",
    "fire-bar-r",
    "vest-bar",
    "menucursor",
    "arrow",
    "title-l",
    "title-r",
];

/// `IsDefaultInterface`
pub fn is_default(name: &str) -> bool {
    name.is_empty() || name == "Default"
}

impl Default for Layout {
    /// `LoadDefaultInterfaceData`
    fn default() -> Layout {
        let bar = |x, y, width, height| Bar {
            x,
            y,
            width,
            height,
            style: BarStyle::Horizontal,
            rotate: 0.0,
        };
        let icon = |x, y| Icon { x, y, rotate: 0.0 };
        let health_ico = icon(5.0, 439.0);
        let ammo_ico = icon(275.0, 439.0);
        let jet_ico = icon(480.0, 439.0);
        Layout {
            name: None,
            alpha: 255,
            shown: Shown {
                health: true,
                ammo: true,
                vest: true,
                jet: true,
                nades: true,
                bullets: true,
                weapon: true,
                fire: true,
                team: true,
                ping: true,
                status: true,
            },
            health_ico,
            health_bar: bar(45.0, 449.0, 115.0, 9.0),
            ammo_ico,
            ammo_bar: bar(352.0, 449.0, 120.0, 9.0),
            jet_ico,
            jet_bar: bar(520.0, 449.0, 115.0, 9.0),
            vest_bar: bar(45.0, 459.0, 115.0, 9.0),
            nades: bar(308.0, 462.0, 10.0, 10.0),
            bullets: (348.0, 451.0),
            weapon: (285.0, 454.0),
            fire_ico: icon(409.0, 464.0),
            fire_bar: bar(402.0, 464.0, 57.0, 4.0),
            team_box: (575.0, 330.0),
            ping: (600.0, 18.0),
            status: (575.0, 421.0),
            right: RightAligned {
                weapon: true,
                bullets: true,
                health_bar: false,
                ammo_bar: false,
                reload_bar: false,
                fire_bar: true,
                jet_bar: false,
                vest_bar: false,
            },
            anchors: Anchors {
                health_bar: (health_ico.x, health_ico.y),
                jet_bar: (jet_ico.x, jet_ico.y),
                ammo_bar: (ammo_ico.x, ammo_ico.y),
                fire_bar: (ammo_ico.x, ammo_ico.y),
                nades_bar: (ammo_ico.x, ammo_ico.y),
            },
        }
    }
}

/// `setup.sif` is 240 bytes: FPC's natural alignment of `TInterface`.
const SIF_SIZE: usize = 240;

impl Layout {
    /// `LoadInterfaceData`: the `ui_style` interface, or the default one when it's the
    /// default or can't be loaded.
    pub fn load(vfs: &Vfs, name: &str) -> Layout {
        if is_default(name) {
            return Layout::default();
        }
        let dir = format!("custom-interfaces/{name}");
        let data = match vfs.read(&format!("{dir}/setup.sif")) {
            Ok(data) if data.len() >= SIF_SIZE => data,
            _ => {
                tracing::warn!(name, "cannot load setup.sif, using the default interface");
                return Layout::default();
            }
        };
        let has_image = |file: &str| {
            vfs.find_with_extensions(&format!("{dir}/{file}"), super::IMAGE_EXTENSIONS)
                .is_some()
        };
        let mut layout = Layout::parse(&data, has_image);
        layout.name = Some(name.to_string());
        layout
    }

    /// Reads a `setup.sif` record. `has_image` tells which icons the interface brings: the
    /// bars move with them.
    pub fn parse(data: &[u8], has_image: impl Fn(&str) -> bool) -> Layout {
        let int = |at: usize| i32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as f32;
        let flag = |at: usize| data[at] != 0;
        let icon = |at: usize| Icon {
            x: int(at),
            y: int(at + 4),
            rotate: int(at + 8),
        };
        // x, y, width, height, the style byte, then the rotation after padding
        let bar = |at: usize, rotate: Option<usize>| Bar {
            x: int(at),
            y: int(at + 4),
            width: int(at + 8),
            height: int(at + 12),
            style: BarStyle::from_byte(data[at + 16]),
            rotate: rotate.map_or(0.0, int),
        };
        let pair = |at: usize| (int(at), int(at + 4));

        let health_ico = icon(12);
        let ammo_ico = icon(48);
        let jet_ico = icon(84);
        let at = |icon: Icon| (icon.x, icon.y);

        // the bars move with the last of the health, jet and ammo icons the interface has,
        // then each bar with its own icon
        let mut anchors = Anchors {
            health_bar: (0.0, 0.0),
            jet_bar: (0.0, 0.0),
            ammo_bar: (0.0, 0.0),
            fire_bar: (0.0, 0.0),
            nades_bar: (0.0, 0.0),
        };
        for (file, icon) in [("health", health_ico), ("jet", jet_ico), ("ammo", ammo_ico)] {
            if has_image(file) {
                let p = at(icon);
                anchors = Anchors {
                    health_bar: p,
                    jet_bar: p,
                    ammo_bar: p,
                    fire_bar: p,
                    nades_bar: p,
                };
            }
        }
        if has_image("health") {
            anchors.health_bar = at(health_ico);
        }
        if has_image("jet") {
            anchors.jet_bar = at(jet_ico);
        }
        if has_image("ammo") {
            anchors.ammo_bar = at(ammo_ico);
        }

        Layout {
            name: None,
            alpha: data[0],
            shown: Shown {
                health: flag(1),
                ammo: flag(2),
                vest: flag(3),
                jet: flag(4),
                nades: flag(5),
                bullets: flag(6),
                weapon: flag(7),
                fire: flag(8),
                team: flag(9),
                ping: flag(10),
                status: flag(11),
            },
            health_ico,
            health_bar: bar(24, Some(44)),
            ammo_ico,
            ammo_bar: bar(60, Some(80)),
            jet_ico,
            jet_bar: bar(96, Some(116)),
            vest_bar: bar(120, Some(140)),
            nades: bar(144, None),
            bullets: pair(164),
            weapon: pair(172),
            fire_ico: icon(180),
            fire_bar: bar(192, Some(212)),
            team_box: pair(216),
            ping: pair(224),
            status: pair(232),
            // LoadInterfaceData's alignment for custom interfaces
            right: RightAligned {
                weapon: false,
                bullets: false,
                health_bar: true,
                ammo_bar: true,
                reload_bar: false,
                fire_bar: true,
                jet_bar: true,
                vest_bar: true,
            },
            anchors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `custom-interfaces/cabbage/setup.sif` from Soldat's base files.
    const CABBAGE: [u8; 240] = {
        let mut d = [0u8; 240];
        let ints: [(usize, i32); 43] = [
            (12, 5),
            (16, 445),
            (24, 45),
            (28, 455),
            (32, 115),
            (36, 9),
            (48, 285),
            (52, 445),
            (60, 352),
            (64, 455),
            (68, 115),
            (72, 9),
            (84, 480),
            (88, 445),
            (96, 520),
            (100, 455),
            (104, 115),
            (108, 9),
            (120, 45),
            (124, 465),
            (128, 115),
            (132, 9),
            (144, 305),
            (148, 468),
            (152, 10),
            (156, 10),
            (164, 315),
            (168, 455),
            (172, 182),
            (176, 460),
            (180, 409),
            (184, 470),
            (192, 409),
            (196, 470),
            (200, 57),
            (204, 4),
            (216, 583),
            (220, 330),
            (224, 600),
            (228, 18),
            (232, 590),
            (236, 421),
            (20, 0),
        ];
        let mut i = 0;
        while i < ints.len() {
            let (at, v) = ints[i];
            let b = v.to_le_bytes();
            d[at] = b[0];
            d[at + 1] = b[1];
            d[at + 2] = b[2];
            d[at + 3] = b[3];
            i += 1;
        }
        d[0] = 255;
        let mut f = 1;
        while f <= 11 {
            d[f] = 1;
            f += 1;
        }
        // the bars' style bytes: horizontal
        d[40] = 1;
        d[76] = 1;
        d[112] = 1;
        d[136] = 1;
        d[160] = 1;
        d[208] = 1;
        d
    };

    #[test]
    fn parses_a_setup_sif() {
        let layout = Layout::parse(&CABBAGE, |file| file == "health" || file == "ammo");
        assert_eq!(layout.alpha, 255);
        assert!(layout.shown.status);
        assert_eq!(
            layout.health_ico,
            Icon {
                x: 5.0,
                y: 445.0,
                rotate: 0.0
            }
        );
        assert_eq!(layout.ammo_bar.width, 115.0);
        assert_eq!(layout.ammo_bar.style, BarStyle::Horizontal);
        assert_eq!(
            layout.nades,
            Bar {
                x: 305.0,
                y: 468.0,
                width: 10.0,
                height: 10.0,
                style: BarStyle::Horizontal,
                rotate: 0.0
            }
        );
        assert_eq!(layout.weapon, (182.0, 460.0));
        assert_eq!(layout.status, (590.0, 421.0));
        // no jet icon: its bar moves with the ammo icon, the last one there
        assert_eq!(layout.anchors.health_bar, (5.0, 445.0));
        assert_eq!(layout.anchors.jet_bar, (285.0, 445.0));
        assert_eq!(layout.anchors.fire_bar, (285.0, 445.0));
    }
}
