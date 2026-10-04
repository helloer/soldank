//! The game menus (`GameMenus.pas`): escape, team, weapons (limbo), kick and map menus,
//! their buttons and actions. Positions are in interface units (480 high, like Soldat's
//! 640x480 interface).

use soldank_core::*;

/// Primary weapons in the limbo menu (Soldat `PRIMARY_WEAPONS`), then 4 secondaries.
pub const PRIMARY_WEAPONS: usize = 10;
pub const MAIN_WEAPONS: usize = 14;

#[derive(Debug, Clone)]
pub struct Button {
    pub min: Vec2,
    pub max: Vec2,
    pub caption: String,
    pub active: bool,
}

impl Button {
    /// `InitButton`
    fn new(caption: String, x: f32, y: f32, w: f32, h: f32) -> Button {
        Button {
            min: vec2(x, y),
            max: vec2(x + w, y + h),
            caption,
            active: true,
        }
    }

    fn contains(&self, p: Vec2) -> bool {
        self.active && p.x > self.min.x && p.x < self.max.x && p.y > self.min.y && p.y < self.max.y
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MenuId {
    Esc,
    Team,
    Limbo,
    Kick,
    Map,
}

/// The escape menu's size (`EscMenu.w`, `EscMenu.h`).
pub const ESC_MENU_SIZE: Vec2 = Vec2::new(300.0, 200.0);
/// Where the kick and map menus are, and their size.
pub const VOTE_MENU_POS: Vec2 = Vec2::new(125.0, 355.0);
pub const VOTE_MENU_SIZE: Vec2 = Vec2::new(370.0, 90.0);

/// What a click or key in a menu asks for.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MenuAction {
    Team(Team),
    /// A primary weapon (`SelWeapon`).
    Primary(WeaponKind),
    /// A secondary weapon by `cl_player_secwep` number (0 USSOCOM .. 3 LAW).
    Secondary(i64),
    /// Escape menu: leave the game, pick another team, the settings.
    Quit,
    ChangeTeam,
    Settings,
    /// Kick menu: the previous or next player, kick the one shown.
    KickPrev,
    KickNext,
    Kick,
    /// Map menu: the previous or next map, play the one shown.
    MapPrev,
    MapNext,
    MapSelect,
}

pub struct Menus {
    pub team_active: bool,
    pub limbo_active: bool,
    /// `LimboLock`: the weapons menu doesn't pop up on death.
    pub limbo_lock: bool,
    /// `FragsMenuShow`: the scoreboard.
    pub frags: bool,
    /// `FragsScrollLev`: the frags menu scrolled down this many steps.
    pub frags_scroll: u8,
    /// `StatsMenuShow`: the player's weapon stats.
    pub stats: bool,
    /// `MiniMapShow`
    pub minimap: bool,
    /// `PlayerNamesShow`: teammates' names (at the screen's edge when out of sight).
    pub player_names: bool,
    /// `ConInfoShow`: the frame rate, ping and connection (the GameStats key).
    pub con_info: bool,
    pub esc_active: bool,
    pub kick_active: bool,
    pub map_active: bool,
    /// `LimboWasActive`: the weapons menu comes back when the escape menu closes.
    limbo_was_active: bool,
    pub esc: Vec<Button>,
    pub team: Vec<Button>,
    pub limbo: Vec<Button>,
    pub kick: Vec<Button>,
    pub map: Vec<Button>,
    /// The escape menu's corner (centered on the screen).
    pub esc_pos: Vec2,
    pub hovered: Option<(MenuId, usize)>,
}

/// The weapons of the limbo menu buttons, in Soldat's `Guns` order.
pub fn limbo_weapon(index: usize) -> WeaponKind {
    WeaponKind::values()[index]
}

impl Menus {
    /// `InitGameMenus`, for an interface `width` wide.
    pub fn new(weapons: &WeaponTable, width: f32) -> Menus {
        let esc_pos = ((vec2(width, 480.0) - ESC_MENU_SIZE) / 2.0).round();
        let at = |pos: Vec2, caption: &str, x: f32, y: f32, w: f32, h: f32| {
            Button::new(caption.to_string(), pos.x + x, pos.y + y, w, h)
        };
        let esc = [
            "1 Exit to menu",
            "2 Change map",
            "3 Kick player",
            "4 Change team",
            // soldank's: the settings in place of Soldat's launcher
            "5 Settings",
        ]
        .iter()
        .enumerate()
        .map(|(i, caption)| at(esc_pos, caption, 5.0, 25.0 * (i + 1) as f32, 240.0, 25.0))
        .collect();
        let v = VOTE_MENU_POS;
        let mut ban = at(v, "Ban", 195.0, 55.0, 80.0, 25.0);
        ban.active = false;
        let kick = vec![
            at(v, "<<<<", 15.0, 35.0, 90.0, 25.0),
            at(v, ">>>>", 265.0, 35.0, 90.0, 25.0),
            at(v, "Kick", 105.0, 55.0, 90.0, 25.0),
            ban,
        ];
        let map = vec![
            at(v, "<<<<", 15.0, 35.0, 90.0, 25.0),
            at(v, ">>>>", 265.0, 35.0, 90.0, 25.0),
            at(v, "Select", 120.0, 55.0, 90.0, 25.0),
        ];

        let mut team = vec![Button::new("0 Player".into(), 40.0, 180.0, 215.0, 35.0)];
        for (i, name) in ["Alpha Team", "Bravo Team", "Charlie Team", "Delta Team"]
            .iter()
            .enumerate()
        {
            let i = i + 1;
            team.push(Button::new(
                format!("{i} {name}"),
                40.0,
                140.0 + 40.0 * i as f32,
                215.0,
                35.0,
            ));
        }
        team.push(Button::new("5 Spectator".into(), 40.0, 340.0, 215.0, 35.0));

        let limbo = (0..MAIN_WEAPONS)
            .map(|i| {
                let name = weapons.get(limbo_weapon(i)).name;
                let caption = if i < PRIMARY_WEAPONS {
                    format!("{} {name}", (i + 1) % 10)
                } else {
                    name.to_string()
                };
                let row = (i + usize::from(i >= PRIMARY_WEAPONS)) as f32;
                Button::new(caption, 35.0, 154.0 + 18.0 * row, 235.0, 16.0)
            })
            .collect();

        Menus {
            team_active: false,
            limbo_active: false,
            limbo_lock: false,
            frags: false,
            frags_scroll: 0,
            stats: false,
            minimap: false,
            player_names: true,
            con_info: false,
            esc_active: false,
            kick_active: false,
            map_active: false,
            limbo_was_active: false,
            esc,
            team,
            limbo,
            kick,
            map,
            esc_pos,
            hovered: None,
        }
    }

    pub fn any_active(&self) -> bool {
        self.team_active
            || self.limbo_active
            || self.esc_active
            || self.kick_active
            || self.map_active
    }

    fn hide_all(&mut self) {
        self.esc_active = false;
        self.team_active = false;
        self.limbo_active = false;
        self.kick_active = false;
        self.map_active = false;
    }

    /// The interface is now `width` wide: the escape menu stays in the middle.
    pub fn resize(&mut self, width: f32) {
        let esc_pos = ((vec2(width, 480.0) - ESC_MENU_SIZE) / 2.0).round();
        let shift = esc_pos - self.esc_pos;
        for button in &mut self.esc {
            button.min += shift;
            button.max += shift;
        }
        self.esc_pos = esc_pos;
    }

    /// `GameMenuShow(EscMenu)`: everything else closes (the weapons menu comes back after).
    pub fn show_esc(&mut self, show: bool) {
        if show {
            self.limbo_was_active |= self.limbo_active;
            self.hide_all();
            self.frags = false;
            self.stats = false;
        } else {
            self.hide_all();
            if std::mem::take(&mut self.limbo_was_active) {
                self.limbo_active = true;
            }
        }
        self.esc_active = show;
    }

    /// The scoreboard or the weapon stats are up: other texts fade.
    pub fn scores_shown(&self) -> bool {
        self.frags || self.stats
    }

    /// The `fragslist` key: the scoreboard, not with the escape menu.
    pub fn toggle_frags(&mut self) {
        if !self.esc_active {
            self.frags_scroll = 0;
            self.frags = !self.frags;
            self.stats &= !self.frags;
        }
    }

    /// The `statsmenu` key: the weapon stats, not with the escape menu.
    pub fn toggle_stats(&mut self) {
        if !self.esc_active {
            self.stats = !self.stats;
            self.frags &= !self.stats;
        }
    }

    /// `MenuKeyDown` for Escape: back from the kick or map menu, or the escape menu on
    /// and off.
    pub fn escape(&mut self) {
        if self.kick_active || self.map_active {
            self.show_esc(true);
        } else {
            self.show_esc(!self.esc_active);
        }
    }

    /// `GameMenuShow(TeamMenu)`: the buttons of the game mode's teams.
    pub fn show_team(&mut self, mode: GameMode) {
        let teams: [bool; 5] = match mode {
            GameMode::CaptureTheFlag | GameMode::Infiltration | GameMode::HoldTheFlag => {
                [false, true, true, false, false]
            }
            GameMode::Teammatch => [false, true, true, true, true],
            _ => [true, false, false, false, false],
        };
        self.hide_all();
        for (button, active) in self.team.iter_mut().zip(teams) {
            button.active = active;
        }
        self.team_active = true;
        self.limbo_active = false;
    }

    pub fn show_limbo(&mut self, show: bool) {
        tracing::debug!(show, "weapons menu");
        self.limbo_active = show;
        if !show {
            self.limbo_was_active = false;
        }
    }

    /// `GameMenuMouseMove`
    pub fn mouse_move(&mut self, pos: Vec2) {
        self.hovered = None;
        for (id, active, buttons) in [
            (MenuId::Esc, self.esc_active, &self.esc),
            (MenuId::Team, self.team_active, &self.team),
            (MenuId::Limbo, self.limbo_active, &self.limbo),
            (MenuId::Kick, self.kick_active, &self.kick),
            (MenuId::Map, self.map_active, &self.map),
        ] {
            if !active {
                continue;
            }
            if let Some(i) = buttons.iter().position(|b| b.contains(pos)) {
                self.hovered = Some((id, i));
                return;
            }
        }
    }

    /// `GameMenuAction`
    pub fn action(&mut self, menu: MenuId, index: usize) -> Option<MenuAction> {
        tracing::debug!(?menu, index, "menu action");
        match menu {
            MenuId::Esc => {
                if !self.esc_active || !self.esc.get(index)?.active {
                    return None;
                }
                match index {
                    0 => Some(MenuAction::Quit),
                    1 => {
                        self.map_active = !self.map_active;
                        self.kick_active = false;
                        None
                    }
                    2 => {
                        self.kick_active = !self.kick_active;
                        self.map_active = false;
                        None
                    }
                    3 => Some(MenuAction::ChangeTeam),
                    // (back to the escape menu after)
                    4 => {
                        self.esc_active = false;
                        Some(MenuAction::Settings)
                    }
                    _ => None,
                }
            }
            MenuId::Kick => {
                if !self.kick_active || !self.kick.get(index)?.active {
                    return None;
                }
                [MenuAction::KickPrev, MenuAction::KickNext, MenuAction::Kick]
                    .get(index)
                    .copied()
            }
            MenuId::Map => {
                if !self.map_active || !self.map.get(index)?.active {
                    return None;
                }
                [
                    MenuAction::MapPrev,
                    MenuAction::MapNext,
                    MenuAction::MapSelect,
                ]
                .get(index)
                .copied()
            }
            MenuId::Team => {
                let button = self.team.get(index)?;
                if !self.team_active || !button.active {
                    return None;
                }
                self.team_active = false;
                Some(MenuAction::Team(match index {
                    1 => Team::Alpha,
                    2 => Team::Bravo,
                    3 => Team::Charlie,
                    4 => Team::Delta,
                    5 => Team::Spectator,
                    _ => Team::None,
                }))
            }
            MenuId::Limbo => {
                let button = self.limbo.get(index)?;
                if !self.limbo_active || !button.active {
                    return None;
                }
                if index < PRIMARY_WEAPONS {
                    self.limbo_active = false;
                    Some(MenuAction::Primary(limbo_weapon(index)))
                } else {
                    Some(MenuAction::Secondary((index - PRIMARY_WEAPONS) as i64))
                }
            }
        }
    }

    /// The limbo menu offers the weapons the player may pick (`WeaponSel`): bits by menu
    /// number.
    pub fn allow_weapons(&mut self, weapon_sel: u16) {
        for (i, button) in self.limbo.iter_mut().enumerate() {
            button.active = weapon_sel & (1 << i) != 0;
        }
    }

    /// Number keys: `digit` 1..9, 0 (as 10); with ctrl the secondaries in the limbo menu.
    pub fn key(&mut self, digit: usize, ctrl: bool) -> Option<MenuAction> {
        if self.team_active {
            (!ctrl).then_some(())?;
            self.action(MenuId::Team, digit % 10)
        } else if self.esc_active {
            (!ctrl).then_some(())?;
            self.action(MenuId::Esc, digit - 1)
        } else if self.limbo_active {
            let index = digit - 1 + if ctrl { PRIMARY_WEAPONS } else { 0 };
            self.action(MenuId::Limbo, index)
        } else {
            None
        }
    }

    /// `GameMenuClick`: the action of the hovered button, or `None`. `hide_limbo` tells if a
    /// click elsewhere hides the weapons menu (a weapon was picked before).
    pub fn click(&mut self, hide_limbo: bool) -> (Option<MenuAction>, bool) {
        match self.hovered {
            Some((menu, index)) => (self.action(menu, index), true),
            None if hide_limbo && self.limbo_active => {
                self.limbo_active = false;
                (None, true)
            }
            None => (None, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menus() -> Menus {
        Menus::new(&WeaponTable::default(), 640.0)
    }

    #[test]
    fn escape_menu_hides_and_brings_back_the_weapons_menu() {
        let mut m = menus();
        m.show_limbo(true);
        m.escape();
        assert!(m.esc_active && !m.limbo_active);
        assert_eq!(m.key(4, false), Some(MenuAction::ChangeTeam));
        assert_eq!(m.key(3, false), None, "the kick menu opens");
        assert!(m.kick_active && m.esc_active);
        m.escape();
        assert!(
            m.esc_active && !m.kick_active,
            "escape goes back to the escape menu"
        );
        m.escape();
        assert!(!m.esc_active && m.limbo_active);
        assert_eq!(m.esc_pos, vec2(170.0, 140.0));
    }

    #[test]
    fn limbo_buttons_follow_soldat_layout() {
        let m = menus();
        assert_eq!(m.limbo.len(), MAIN_WEAPONS);
        assert_eq!(m.limbo[0].caption, "1 Desert Eagles");
        assert_eq!(m.limbo[9].caption, "0 XM214 Minigun");
        assert_eq!(m.limbo[10].caption, "USSOCOM");
        // a gap row between primaries and secondaries
        assert_eq!(m.limbo[9].min.y, 154.0 + 18.0 * 9.0);
        assert_eq!(m.limbo[10].min.y, 154.0 + 18.0 * 11.0);
    }

    #[test]
    fn number_keys_pick_weapons() {
        let mut m = menus();
        assert_eq!(m.key(3, false), None, "closed menus ignore keys");

        m.show_limbo(true);
        assert_eq!(m.key(5, true), None, "only 4 secondaries");
        assert!(m.limbo_active, "a secondary keeps the menu open");
        assert_eq!(
            m.key(10, false),
            Some(MenuAction::Primary(WeaponKind::Minigun))
        );
        assert!(!m.limbo_active, "a primary closes it");
    }

    #[test]
    fn ctrl_numbers_pick_secondaries() {
        let mut m = menus();
        m.show_limbo(true);
        assert_eq!(m.key(1, true), Some(MenuAction::Secondary(0)));
        assert_eq!(m.key(4, true), Some(MenuAction::Secondary(3)));
        assert_eq!(m.key(5, true), None);
    }

    #[test]
    fn clicks_use_the_hovered_button() {
        let mut m = menus();
        m.show_limbo(true);

        m.mouse_move(vec2(100.0, 154.0 + 18.0 * 2.0 + 8.0));
        assert_eq!(m.hovered, Some((MenuId::Limbo, 2)));
        assert_eq!(
            m.click(false),
            (Some(MenuAction::Primary(WeaponKind::Ak74)), true)
        );

        // a click elsewhere hides the menu only once a weapon was picked
        m.show_limbo(true);
        m.mouse_move(vec2(400.0, 50.0));
        assert_eq!(m.click(false), (None, false));
        assert!(m.limbo_active);
        assert_eq!(m.click(true), (None, true));
        assert!(!m.limbo_active);
    }

    #[test]
    fn team_menu_offers_the_mode_teams() {
        let mut m = menus();
        m.show_team(GameMode::CaptureTheFlag);
        assert_eq!(m.key(3, false), None, "no charlie in CTF");
        assert_eq!(m.key(2, false), Some(MenuAction::Team(Team::Bravo)));
        assert!(!m.team_active);

        m.show_team(GameMode::Teammatch);
        assert_eq!(m.key(4, false), Some(MenuAction::Team(Team::Delta)));
    }
}
