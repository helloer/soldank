//! Interface drawn over the game in interface units (480 high): the team and weapons menus
//! (`InterfaceGraphics.pas`) and the menu cursor.

use super::*;
use crate::menus::{MAIN_WEAPONS, MenuId, Menus, PRIMARY_WEAPONS};
use gfx::{Interface, SpriteData};

/// `BACKGROUND_WIDTH`: the menu backgrounds are scaled relative to this.
const BACKGROUND_WIDTH: f32 = 64.0;

/// What the menus show about the local player.
pub struct InterfaceState<'a> {
    pub menus: &'a Menus,
    /// The local player, for the HUD.
    pub player: Option<SoldierId>,
    /// The local player watches from the spectators.
    pub spectating: bool,
    pub messages: &'a crate::render::hud::HudMessages,
    /// `SelWeapon` and `cl_player_secwep`.
    pub sel_weapon: Option<WeaponKind>,
    pub secondary: i64,
    /// Players in each team, for the team menu.
    pub team_counts: [usize; 6],
    /// The cursor in interface units.
    pub mouse: Vec2,
    /// Draw the menu cursor (a menu is open or the player is dead).
    pub show_cursor: bool,
    /// `ui_status_transparency`
    pub alpha: u8,
    pub cvars: &'a Cvars,
    pub chat: &'a crate::chat::Chat,
    /// The soldier the camera follows (`None`: the free camera).
    pub follow: Option<SoldierId>,
    /// The kick menu's player (name, shirt colour) and the map menu's map.
    pub kick: Option<(String, u32)>,
    pub map_name: Option<String>,
    pub radio: Option<RadioView>,
    pub vote: Option<VoteView>,
    /// A weapons mod's changes to each menu weapon (`GetWeaponAttribs`), when there is one.
    pub weapon_tips: Option<Vec<String>>,
    pub stats: &'a crate::stats::WeaponStats,
    /// Debug drawing over the world (dev overlay).
    pub debug: super::debug::DebugDraw,
    /// The GameStats texts, when shown (`ConInfoShow`).
    pub con_info: Option<ConInfo>,
    /// A demo is being recorded: the blinking REC.
    pub recording: bool,
    pub snap_offered: bool,
    pub no_crosshair: bool,
    /// Bullet time: the screen cut wide (`WideScreenCut`).
    pub wide_cut: bool,
}

/// The GameStats texts: the frame rate, the player's ping (`RealPing`), and a demo's tick
/// and length or the connection's numbers.
pub struct ConInfo {
    pub fps: u32,
    pub ping: Option<u16>,
    pub demo: Option<(usize, usize)>,
    pub net: Option<NetStats>,
}

/// A connection's numbers: round trip (seconds), packets lost (a fraction), bytes a second.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetStats {
    pub rtt: f64,
    pub packet_loss: f64,
    pub sent: f64,
    pub received: f64,
}

/// A server's running vote as shown.
pub struct VoteView {
    pub kick: bool,
    /// The map, or the player to kick.
    pub target: String,
    pub starter: String,
    pub reason: String,
}

/// The radio menu as shown: the subjects, the first choice and then where.
pub struct RadioView {
    pub subjects: [String; 3],
    pub chosen: Option<usize>,
    pub places: [String; 3],
}

fn sprite_rect(batch: &mut DrawBatch, sprite: &Sprite, pos: Vec2, size: Vec2, color: Color) {
    let (tx0, tx1) = sprite.texcoords_x;
    let (ty0, ty1) = sprite.texcoords_y;
    let p1 = pos + size;
    batch.add_quad(
        sprite.texture.as_ref(),
        &[
            vertex(pos, vec2(tx0, ty0), color),
            vertex(vec2(p1.x, pos.y), vec2(tx1, ty0), color),
            vertex(p1, vec2(tx1, ty1), color),
            vertex(vec2(pos.x, p1.y), vec2(tx0, ty1), color),
        ],
    );
}

fn interface_sprite(sprites: &[Vec<Sprite>], id: Interface) -> &Sprite {
    &sprites[id.group().id()][id.id()]
}

/// The gun icon of a limbo menu button.
fn gun_icon(index: usize) -> Interface {
    if index < PRIMARY_WEAPONS {
        Interface::GunsDeagles + index
    } else {
        [
            Interface::GunsSocom,
            Interface::GunsKnife,
            Interface::GunsChainsaw,
            Interface::GunsLaw,
        ][index - PRIMARY_WEAPONS]
    }
}

fn scaled_alpha(alpha: u8, factor: f32) -> u8 {
    (f32::from(alpha) * factor).round() as u8
}

/// The menu backgrounds and gun icons, drawn under the interface texts.
pub fn render_menu_backgrounds(
    batch: &mut DrawBatch,
    sprites: &[Vec<Sprite>],
    state: &InterfaceState,
) {
    let menus = state.menus;
    let back = interface_sprite(sprites, Interface::Back);
    let back_color = rgba(255, 255, 255, scaled_alpha(state.alpha, 0.56));

    if menus.team_active {
        sprite_rect(
            batch,
            back,
            vec2(45.0, 140.0),
            vec2(262.0, 250.0) * (back.width / BACKGROUND_WIDTH),
            back_color,
        );
    }

    if menus.limbo_active {
        let scale = back.width / BACKGROUND_WIDTH;
        sprite_rect(
            batch,
            back,
            vec2(45.0, 140.0),
            vec2(252.0, 210.0) * scale,
            back_color,
        );
        sprite_rect(
            batch,
            back,
            vec2(45.0, 350.0),
            vec2(252.0, 80.0) * scale,
            back_color,
        );

        // guns
        for index in 0..MAIN_WEAPONS {
            let icon = interface_sprite(sprites, gun_icon(index));
            let row = if index < PRIMARY_WEAPONS {
                index as f32
            } else {
                (index + 1) as f32
            };
            let dy = f32::max(0.0, 18.0 - icon.height) / 2.0;
            let pos = vec2(55.0, (157.0 + 18.0 * row + dy).round());
            let alpha =
                if index < PRIMARY_WEAPONS || state.secondary == (index - PRIMARY_WEAPONS) as i64 {
                    state.alpha
                } else {
                    scaled_alpha(state.alpha, 0.5)
                };
            sprite_rect(
                batch,
                icon,
                pos,
                vec2(icon.width, icon.height),
                rgba(255, 255, 255, alpha),
            );
        }
    }
}

/// The menu texts and the menu cursor, over everything else (`RenderGameMenuTexts`).
pub fn render_menu_texts(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &InterfaceState,
) {
    if state.menus.team_active {
        render_team_menu_text(batch, fonts, state);
    }
    if state.menus.limbo_active {
        render_weapon_menu_text(batch, fonts, state);
    }
    if state.menus.esc_active {
        render_esc_menu(batch, fonts, sprites, state);
    }
    if let Some(radio) = &state.radio {
        render_radio_menu(batch, fonts, sprites, state, radio);
    }
    let picked = |name: &Option<String>, color: Color| name.as_ref().map(|n| (n.clone(), color));
    if state.menus.kick_active {
        let shown = state
            .kick
            .as_ref()
            .map(|(name, shirt)| (name.clone(), hex_color(*shirt)));
        render_vote_menu(batch, fonts, sprites, state, MenuId::Kick, shown);
    }
    if state.menus.map_active {
        let shown = picked(&state.map_name, rgba(135, 235, 135, 230));
        render_vote_menu(batch, fonts, sprites, state, MenuId::Map, shown);
    }
    if let Some(vote) = &state.vote {
        render_vote_texts(batch, fonts, vote);
    }
    // the default keys, for the first game runs
    if state.menus.esc_active && state.cvars.int("cl_runs") < 3 && !state.chat.active() {
        let black = Some(rgb(0, 0, 0));
        let title = "Default keys (shown for first 3 game runs)";
        fonts.draw(
            batch,
            FontStyle::Smallest,
            title,
            vec2(30.0, 28.0),
            rgb(250, 90, 95),
            black,
        );
        let keys = [
            "[A]/[D] move left/right",
            "[W]/[S]/[X] jump / crouch / lie down",
            "[Left Mouse] fire!",
            "[Right Mouse] jet boots",
            "hold [E] to toss grenade",
            "[R] reloads weapon",
            "[Q] change weapon / [F] throw weapon",
            "[T] chat / [Y] team chat",
        ];
        for (i, text) in (1u8..).zip(keys) {
            let pos = vec2(30.0, 28.0 + 12.0 * f32::from(i));
            fonts.draw(
                batch,
                FontStyle::Small,
                text,
                pos,
                rgb(230, 232 - 2 * i, 255),
                black,
            );
        }
    }

    if state.show_cursor {
        let cursor = interface_sprite(sprites, Interface::Menucursor);
        sprite_rect(
            batch,
            cursor,
            state.mouse.round(),
            vec2(cursor.width, cursor.height),
            rgba(255, 255, 255, state.alpha),
        );
    }
}

/// `RenderVoteMenuTexts`: what the vote is about, who started it and why, and the keys.
fn render_vote_texts(batch: &mut DrawBatch, fonts: &Fonts, vote: &VoteView) {
    let (x, y) = (45.0, 400.0);
    let lines = [
        (
            if vote.kick { "Kick" } else { "Map" },
            x + 30.0,
            y,
            rgba(254, 104, 104, 225),
        ),
        (&vote.target, x + 65.0, y, rgba(244, 244, 244, 225)),
        (
            &format!("Voter: {}", vote.starter),
            x + 10.0,
            y + 11.0,
            rgba(224, 218, 244, 205),
        ),
        (
            &format!("Reason:{}", vote.reason),
            x + 10.0,
            y + 20.0,
            rgba(224, 218, 244, 205),
        ),
        (
            "F12 - Yes   F11 - No",
            x + 50.0,
            y + 31.0,
            rgba(234, 234, 114, 205),
        ),
    ];
    for (text, x, y, color) in lines {
        fonts.draw(batch, FontStyle::WeaponsMenu, text, vec2(x, y), color, None);
    }
}

fn hex_color(c: u32) -> Color {
    rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Captions of a menu's buttons, the hovered one nudged up right (`Ord(Btn = HoveredButton)`).
fn button_captions(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    state: &InterfaceState,
    id: MenuId,
    buttons: &[crate::menus::Button],
) {
    for (i, button) in buttons.iter().enumerate() {
        if !button.active {
            continue;
        }
        let lift = f32::from(u8::from(state.menus.hovered == Some((id, i))));
        let height = fonts.measure(FontStyle::Menu, &button.caption).y;
        let pos = vec2(
            button.min.x + 10.0 + lift,
            button.min.y - lift + (button.max.y - button.min.y) / 2.0 - height / 2.0,
        );
        fonts.draw(
            batch,
            FontStyle::Menu,
            &button.caption,
            pos,
            rgba(255, 255, 255, 250),
            Some(rgb(0, 0, 0)),
        );
    }
}

/// `RenderEscMenuText`
fn render_esc_menu(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &InterfaceState,
) {
    let back = interface_sprite(sprites, Interface::Back);
    let pos = state.menus.esc_pos;
    let size = crate::menus::ESC_MENU_SIZE;
    let color = rgba(255, 255, 255, scaled_alpha(state.alpha, 0.56));
    sprite_rect(
        batch,
        back,
        pos,
        size * (back.width / BACKGROUND_WIDTH),
        color,
    );

    let black = Some(rgb(0, 0, 0));
    fonts.draw(
        batch,
        FontStyle::Small,
        "ESC - return to game",
        pos + vec2(20.0, size.y - 45.0),
        rgba(250, 245, 255, 240),
        black,
    );
    // bottom aligned in the corner
    let version = concat!("Soldank ", env!("CARGO_PKG_VERSION"));
    let v = fonts.measure(FontStyle::Small, version);
    fonts.draw(
        batch,
        FontStyle::Small,
        version,
        pos + size - vec2(2.0 + v.x, v.y),
        rgba(230, 235, 255, 190),
        black,
    );
    button_captions(batch, fonts, state, MenuId::Esc, &state.menus.esc);
}

/// `RenderKickWindowText`, `RenderMapWindowText`: the player or map to pick and the buttons.
fn render_vote_menu(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &InterfaceState,
    id: MenuId,
    shown: Option<(String, Color)>,
) {
    let back = interface_sprite(sprites, Interface::Back);
    let color = rgba(255, 255, 255, scaled_alpha(state.alpha, 0.56));
    let size = crate::menus::VOTE_MENU_SIZE * (back.width / BACKGROUND_WIDTH);
    sprite_rect(batch, back, crate::menus::VOTE_MENU_POS, size, color);

    let buttons = if id == MenuId::Kick {
        &state.menus.kick
    } else {
        &state.menus.map
    };
    if let Some((text, color)) = shown {
        let pos = buttons[0].min - vec2(0.0, 15.0);
        fonts.draw(
            batch,
            FontStyle::Menu,
            &text,
            pos,
            color,
            Some(rgb(0, 0, 0)),
        );
    }
    button_captions(batch, fonts, state, id, buttons);
}

/// `RenderRadioMenuTexts`
fn render_radio_menu(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &InterfaceState,
    radio: &RadioView,
) {
    let back = interface_sprite(sprites, Interface::Back);
    let color = rgba(255, 255, 255, scaled_alpha(state.alpha, 0.56));
    let size = vec2(180.0, 80.0) * (back.width / BACKGROUND_WIDTH);
    sprite_rect(batch, back, vec2(5.0, 250.0), size, color);
    if radio.chosen.is_some() {
        sprite_rect(batch, back, vec2(185.0, 250.0), size, color);
    }

    let alpha = if state.menus.scores_shown() { 80 } else { 230 };
    let black = Some(rgb(0, 0, 0));
    fonts.draw(
        batch,
        FontStyle::Menu,
        "Radio:",
        vec2(10.0, 252.0),
        rgba(255, 255, 255, alpha),
        black,
    );
    let normal = rgba(200, 200, 200, alpha);
    let picked = rgba(210, 210, 5, alpha);
    for (i, subject) in radio.subjects.iter().enumerate() {
        let color = if radio.chosen == Some(i) {
            picked
        } else {
            normal
        };
        let text = format!("{}: {subject}", i + 1);
        fonts.draw(
            batch,
            FontStyle::Small,
            &text,
            vec2(10.0, 270.0 + 12.0 * i as f32),
            color,
            black,
        );
    }
    if radio.chosen.is_some() {
        for (i, place) in radio.places.iter().enumerate() {
            let text = format!("{}: {place}", i + 1);
            fonts.draw(
                batch,
                FontStyle::Small,
                &text,
                vec2(190.0, 270.0 + 12.0 * i as f32),
                normal,
                black,
            );
        }
    }
}

/// `RenderTeamMenuText`
fn render_team_menu_text(batch: &mut DrawBatch, fonts: &Fonts, state: &InterfaceState) {
    let menus = state.menus;
    let black = Some(rgb(0, 0, 0));
    // DM, alpha, bravo, charlie, delta, spectator; normal and hovered
    let colors = [
        (rgb(255, 255, 255), rgba(255, 255, 255, 250)),
        (rgb(210, 15, 5), rgba(210, 15, 5, 250)),
        (rgb(5, 15, 205), rgba(5, 15, 205, 250)),
        (rgb(210, 210, 5), rgba(210, 210, 5, 250)),
        (rgb(5, 210, 5), rgba(5, 210, 5, 250)),
        (rgb(210, 210, 105), rgba(210, 210, 105, 250)),
    ];

    fonts.draw(
        batch,
        FontStyle::Menu,
        "Select Team:",
        vec2(55.0, 165.0),
        rgb(234, 234, 234),
        black,
    );

    for (i, button) in menus.team.iter().enumerate() {
        if !button.active {
            continue;
        }
        let hovered = menus.hovered == Some((MenuId::Team, i));
        let shadow = if i == 2 {
            Some(rgb(0x33, 0x33, 0x33))
        } else {
            black
        };
        let mut color = if hovered { colors[i].1 } else { colors[i].0 };
        if !hovered && menus.scores_shown() {
            color.set_a(80);
        }
        let lift = f32::from(u8::from(hovered));
        let height = fonts.measure(FontStyle::Menu, &button.caption).y;
        let y = button.min.y - lift + (button.max.y - button.min.y) / 2.0 - height / 2.0;

        fonts.draw(
            batch,
            FontStyle::Menu,
            &button.caption,
            vec2(button.min.x + 10.0 + lift, y),
            color,
            shadow,
        );

        if (1..5).contains(&i) {
            fonts.draw(
                batch,
                FontStyle::Menu,
                &format!("({})", state.team_counts[i]),
                vec2(269.0 + lift, y),
                color,
                shadow,
            );
        }
    }
}

/// `RenderWeaponMenuText`
fn render_weapon_menu_text(batch: &mut DrawBatch, fonts: &Fonts, state: &InterfaceState) {
    let menus = state.menus;
    let black = Some(rgb(0, 0, 0));

    fonts.draw(
        batch,
        FontStyle::Small,
        "Primary Weapon:",
        vec2(65.0, 142.0),
        rgb(234, 234, 234),
        black,
    );
    // drawn on its baseline
    fonts.draw(
        batch,
        FontStyle::Small,
        "Secondary Weapon:",
        vec2(65.0, 349.0 - fonts.ascent(FontStyle::Small)),
        rgb(214, 214, 214),
        black,
    );

    for (i, button) in menus.limbo.iter().enumerate() {
        if !button.active {
            continue;
        }
        let hovered = menus.hovered == Some((MenuId::Limbo, i));
        let selected = if i < PRIMARY_WEAPONS {
            state.sel_weapon == Some(crate::menus::limbo_weapon(i))
        } else {
            state.secondary == (i - PRIMARY_WEAPONS) as i64
        };

        let mut pos = vec2(
            button.min.x + 85.0,
            button.min.y + (button.max.y - button.min.y) / 2.0 - 2.0,
        );
        let color = match (selected, hovered) {
            (true, true) => rgba(85, 105, 55, 230),
            (true, false) => rgba(55, 165, 55, 230),
            (false, true) => {
                pos += vec2(1.0, -1.0);
                rgba(255, 255, 255, 230)
            }
            (false, false) => rgba(255, 255, 255, 230),
        };
        // top aligned text starting just above the middle of the button
        fonts.draw(batch, FontStyle::Small, &button.caption, pos, color, black);
    }

    if state.weapon_tips.is_some() {
        let text = "Weapons Mod";
        let x = 45.0 + 252.0 - fonts.measure(FontStyle::Small, text).x;
        let pos = vec2(x, 139.0 - fonts.ascent(FontStyle::Small));
        fonts.draw(
            batch,
            FontStyle::Small,
            text,
            pos,
            rgba(204, 94, 94, 205),
            black,
        );
    }

    // about the weapon under the cursor (the first without one)
    let i = match menus.hovered {
        Some((MenuId::Limbo, i)) => i,
        _ => 0,
    };
    let (Some(button), Some(row)) = (menus.limbo.get(i), menus.limbo.get(i.min(9))) else {
        return;
    };
    let x = button.min.x;
    let mut tip_y = (button.min.y + button.max.y) / 2.0;
    let y = (row.min.y + row.max.y) / 2.0;
    if let Some(tips) = &state.weapon_tips {
        tip_y = y - 26.0;
        let color = rgba(215, 215, 155, 230);
        let pos = vec2(x + 245.0, y - 16.0);
        fonts.draw(batch, FontStyle::WeaponsMenu, &tips[i], pos, color, black);
    }
    // for the first game runs
    if state.cvars.int("cl_runs") < 4 {
        let hint = match i + 1 {
            8 => "Hold fire to shoot, inaccurate while moving",
            12 => "Can be thrown by holding throw weapon button",
            14 => "Hold fire to shoot, while crouching or prone",
            _ => return,
        };
        let pos = vec2(x + 245.0, tip_y - 2.0);
        fonts.draw(
            batch,
            FontStyle::WeaponsMenu,
            hint,
            pos,
            rgba(225, 195, 195, 250),
            black,
        );
    }
}

/// `GetWeaponAttribs`: what a weapons mod changed of each of the 14 menu weapons, as Soldat's
/// limbo menu shows it; `None` without a change.
pub fn weapon_tips(current: &WeaponTable, default: &WeaponTable) -> Option<Vec<String>> {
    let attrs = |w: &Weapon| {
        [
            (
                "Damage",
                w.hit_multiply * (w.modifier_legs + w.modifier_chest + w.modifier_head) / 3.0,
            ),
            ("Ammo", f32::from(w.ammo)),
            ("Reload", f32::from(w.reload_time)),
            ("Speed", w.speed),
            ("Rate", f32::from(w.fire_interval)),
            ("Acc.", w.movement_acc),
            ("Bink", f32::from(w.bink)),
            ("Delay", f32::from(w.start_up_time)),
            ("Spread", w.bullet_spread),
            ("Recoil", f32::from(w.recoil)),
            ("Push", w.push),
            ("Style", f32::from(w.bullet_style as u8)),
            ("Inh. Speed", w.inherited_velocity),
        ]
    };
    // FormatFloat('0.####')
    let number = |v: f32| {
        let text = format!("{:.4}", f64::from(v));
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut changed = false;
    let tips = WeaponKind::values()[..14]
        .iter()
        .map(|&kind| {
            let (cur, def) = (current.get(kind), default.get(kind));
            let mut tip = format!(
                "{}\n   Settings      : change% (present/default)\n",
                cur.name
            );
            for ((name, now), (_, was)) in attrs(&cur).into_iter().zip(attrs(&def)) {
                if now != was {
                    changed = true;
                    let change = if was != 0.0 {
                        format!("{}%", (now / was * 100.0).round_ties_even() as i64)
                    } else {
                        "NEW".to_string()
                    };
                    tip += &format!("    |-{name} : {change} ({}/{})", number(now), number(was));
                }
                tip.push('\n');
            }
            tip
        })
        .collect();
    changed.then_some(tips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weapons_mod_shows_its_changes() {
        let default = WeaponTable::new(false, None).unwrap();
        assert_eq!(weapon_tips(&default, &default), None);
        let damage = default.get(WeaponKind::DesertEagles).hit_multiply;
        let ini = format!("[Info]\n[Desert Eagles]\nDamage={}\nAmmo=9\n", damage * 2.0);
        let modded = WeaponTable::new(false, Some(&ini)).unwrap();
        let tips = weapon_tips(&modded, &default).unwrap();
        let lines: Vec<&str> = tips[0].lines().collect();
        assert_eq!(lines[0], "Desert Eagles");
        assert!(
            lines[2].starts_with("    |-Damage : 200% ("),
            "{}",
            lines[2]
        );
        assert_eq!(lines[3], "    |-Ammo : 129% (9/7)");
        // unchanged settings leave their lines empty, other weapons too
        assert_eq!(lines[4], "");
        assert!(tips[1].lines().skip(2).all(str::is_empty));
    }
}
