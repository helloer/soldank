//! The player's HUD (`RenderInterface`, `RenderPlayerInterfaceTexts`): bars and icons,
//! ammo and weapon, rank and kills, respawn and survival messages, the crosshair and the
//! player indicator. Opensoldat's default interface, in its 640x480 coordinates spread over
//! the interface width (`_iscala`).

use super::console::{ChatBubble, TextConsole, colors as console_colors};
use super::layout::{Bar, BarStyle, Icon, Layout};
use super::*;
use crate::chat::Chat;
use crate::menus::Menus;
use gfx::{Interface, SpriteData};

/// `DEFAULTVEST`
const DEFAULT_VEST: f32 = 100.0;

/// What the HUD shows.
pub struct HudState<'a> {
    pub world: &'a World,
    /// Whose HUD it is: the local player, or the followed soldier of a spectator
    /// (`sv_advancedspectator`).
    pub player: Option<SoldierId>,
    /// The local player when it watches from the spectators.
    pub spectator: Option<SoldierId>,
    /// The camera center (interpolated) and zoom factor, to place things over the world.
    pub camera: Vec2,
    pub zoom: f32,
    /// Interface size (480 high) and the cursor in it.
    pub size: Vec2,
    pub mouse: Vec2,
    pub menus: &'a Menus,
    pub chat: &'a Chat,
    pub follow: Option<SoldierId>,
    /// `ui_status_transparency`
    pub alpha: u8,
    pub elapsed: f64,
    pub messages: &'a HudMessages,
    /// Where the panel's parts go (`ui_style`).
    pub layout: &'a Layout,
    /// Interface units per screen pixel.
    pub pixel: f32,
    pub cvars: &'a Cvars,
    /// The GameStats texts, when shown.
    pub con_info: Option<&'a super::interface::ConInfo>,
    /// A demo is being recorded.
    pub recording: bool,
    /// An action snap is offered; the crosshair is off (a demo).
    pub snap_offered: bool,
    pub no_crosshair: bool,
    /// Bullet time: the screen cut wide.
    pub wide_cut: bool,
}

impl HudState<'_> {
    /// The local player, also while it watches someone else's HUD as a spectator.
    pub fn local_player(&self) -> Option<SoldierId> {
        self.spectator.or(self.player)
    }
}

/// Opensoldat's ARGB message colors.
mod colors {
    pub const KILL: u32 = 0xFFEA3530;
    pub const DIE: u32 = 0xFFC53025;
    pub const DEATH: u32 = 0xEE801304;
    pub const KILLER: u32 = 0xEE52D119;
    pub const ALPHA_K: u32 = 0xEBFFE3E3;
    pub const BRAVO_K: u32 = 0xEBD3E3FF;
    pub const CHARLIE_K: u32 = 0xEBFFFFE3;
    pub const DELTA_K: u32 = 0xEBD3FFE3;
    pub const ALPHA_D: u32 = 0xEBDAB0B0;
    pub const BRAVO_D: u32 = 0xEBA0B0DA;
    pub const CHARLIE_D: u32 = 0xEBD0D0B0;
    pub const DELTA_D: u32 = 0xEBA0D0BA;
    pub const SPECTATOR_D: u32 = 0xEBD3B727;
}

const MULTIKILL_MESSAGES: [&str; 16] = [
    "DOUBLE KILL",
    "TRIPLE KILL",
    "MULTI KILL",
    "MULTI KILL X2",
    "SERIAL KILL",
    "INSANE KILLS",
    "GIMME MORE!",
    "MASTA KILLA!",
    "MASTA KILLA!",
    "MASTA KILLA!",
    "STOP IT!!!!",
    "MERCY!!!!!!!!!!",
    "CHEATER!!!!!!!!",
    "Phased-plasma rifle in the forty watt range",
    "Hey, just what you see, pal",
    "just what you see, pal...",
];
/// `KILLMESSAGEWAIT`
const KILL_MESSAGE_WAIT: i32 = 60 * 4;
/// `ScrollTickMax`, `NewMessageWait`
const KILL_CONSOLE_SCROLL_TICKS: i32 = 240;
const KILL_CONSOLE_NEW_MESSAGE_WAIT: i32 = 70;
const KILL_CONSOLE_SEPARATE_HEIGHT: f32 = 8.0;

fn argb(c: u32) -> Color {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

struct KillLine {
    text: String,
    color: u32,
    /// The weapon icon of a kill (`NumMessage`).
    icon: Option<Interface>,
}

/// The kill console and the big message in the middle (`KillConsole`, `BigMessage`), the
/// console (`MainConsole`, `BigConsole`) and chat messages over players' heads.
pub struct HudMessages {
    kills: Vec<KillLine>,
    /// `ui_killconsole_length`
    pub kill_length: usize,
    scroll_tick: i32,
    big: Option<(String, u32, i32)>,
    pub main: TextConsole,
    /// Everything said recently, shown while typing.
    pub history: TextConsole,
    pub chats: Vec<ChatBubble>,
    /// The player's killing shot, and how long it shows (`ShotDistanceShow`).
    shot: Option<(Shot, i32)>,
    /// `mute all` (`MuteAll`): nobody's chat shows.
    pub mute_all: bool,
}

impl Default for HudMessages {
    fn default() -> HudMessages {
        HudMessages {
            kills: Vec::new(),
            kill_length: 15,
            scroll_tick: 0,
            big: None,
            main: TextConsole::new(150, 150),
            history: TextConsole::new(1_500_000, 0),
            chats: Vec::new(),
            shot: None,
            mute_all: false,
        }
    }
}

/// `SPACECHARDELAY`, `CHARDELAY`, `MAX_CHATDELAY`: how long a message stays over the head.
const SPACE_CHAR_DELAY: i32 = 68;
const CHAR_DELAY: i32 = 25;
const MAX_CHAT_DELAY: i32 = 60 * 7 + 40;

/// The kill console icon of a weapon (`Deathsnap.KillBullet`).
fn weapon_icon(weapon: WeaponKind) -> Option<Interface> {
    use WeaponKind::*;
    Some(match weapon {
        USSOCOM => Interface::GunsSocom,
        DesertEagles => Interface::GunsDeagles,
        MP5 => Interface::GunsMp5,
        Ak74 => Interface::GunsAk74,
        SteyrAUG => Interface::GunsSteyr,
        Spas12 => Interface::GunsSpas,
        Ruger77 => Interface::GunsRuger,
        M79 => Interface::GunsM79,
        Barrett => Interface::GunsBarrett,
        Minimi => Interface::GunsMinimi,
        Minigun => Interface::GunsMinigun,
        Flamer => Interface::GunsFlamer,
        NoWeapon => Interface::GunsFist,
        Bow | FlameBow => Interface::GunsBow,
        Cluster | ClusterGrenade => Interface::ClusterNade,
        Knife | ThrownKnife => Interface::GunsKnife,
        Chainsaw => Interface::GunsChainsaw,
        FragGrenade => Interface::Nade,
        LAW => Interface::GunsLaw,
        M2 => Interface::GunsM2,
    })
}

impl HudMessages {
    fn add(&mut self, text: String, color: u32, icon: Option<Interface>) {
        self.kills.push(KillLine { text, color, icon });
        self.scroll_tick = -KILL_CONSOLE_NEW_MESSAGE_WAIT;
        if self.kills.len() == self.kill_length {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        if !self.kills.is_empty() {
            self.kills.remove(0);
        }
        self.scroll_tick = 0;
    }

    /// A tick went by (`UpdateFrame`).
    pub fn tick(&mut self) {
        self.scroll_tick += 1;
        if self.scroll_tick == KILL_CONSOLE_SCROLL_TICKS {
            self.scroll();
            // a kill takes two lines: the killer's (with the icon) and the victim's; Soldat
            // looks at the last line to decide
            if self.kills.last().is_some_and(|l| l.icon.is_none()) {
                self.scroll();
            }
        }
        self.main.tick();
        for chat in &mut self.chats {
            chat.delay -= 1;
        }
        self.chats.retain(|c| c.delay > 0);
        if let Some((_, _, delay)) = &mut self.big {
            *delay -= 1;
            if *delay <= 0 {
                self.big = None;
            }
        }
        if let Some((_, show)) = &mut self.shot {
            *show -= 1;
            if *show <= 0 {
                self.shot = None;
            }
        }
    }

    /// `BigMessage`: the message in the middle for `wait` ticks.
    pub fn big_message(&mut self, text: impl Into<String>, color: u32, wait: i32) {
        self.big = Some((text.into(), color, wait));
    }

    /// A console line (`MainConsole.Console`): `length` is `ui_console_length`.
    pub fn console(&mut self, text: impl Into<String>, color: u32, length: usize) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        self.main.add(text.clone(), color, length);
        self.history.add(text, color, super::console::HISTORY_LINES);
    }

    /// A player said something (`ClientHandleChatMessage`).
    pub fn chat(&mut self, world: &World, id: SoldierId, text: &str, team: bool, length: usize) {
        self.chat_message(world, id, text, team, false, length);
    }

    /// A radio message (team chat from the radio menu).
    pub fn radio(&mut self, world: &World, id: SoldierId, text: &str, length: usize) {
        self.chat_message(world, id, text, false, true, length);
    }

    fn chat_message(
        &mut self,
        world: &World,
        id: SoldierId,
        text: &str,
        team: bool,
        radio: bool,
        length: usize,
    ) {
        let Some(soldier) = world.soldiers.get(id).filter(|s| s.active) else {
            return;
        };
        if soldier.muted || self.mute_all {
            return;
        }
        let spaces = text.chars().filter(|&c| c == ' ').count() as i32;
        let delay = if spaces == 0 {
            text.chars().count() as i32 * CHAR_DELAY
        } else {
            spaces * SPACE_CHAR_DELAY
        };
        self.chats.retain(|c| c.id != id);
        self.chats.push(ChatBubble {
            id,
            text: text.to_string(),
            delay: delay.min(MAX_CHAT_DELAY),
            team,
        });

        let (prefix, color) = if radio {
            ("(RADIO) ", console_colors::TEAM_CHAT)
        } else if team {
            ("(TEAM) ", console_colors::TEAM_CHAT)
        } else {
            ("", console_colors::CHAT)
        };
        let name = format!("{prefix}[{}] ", soldier.name);
        if text.chars().count() < 60 {
            self.console(format!("{name}{text}"), color, length);
        } else {
            self.console(name, color, length);
            self.console(format!(" {text}"), color, length);
        }
    }

    /// A death (`ClientSpriteDeath`): the kill console lines and, for the local player,
    /// the big message.
    pub fn killed(
        &mut self,
        world: &World,
        player: Option<SoldierId>,
        victim: SoldierId,
        killer: SoldierId,
        weapon: Option<WeaponKind>,
        shot: Shot,
    ) {
        let (Some(v), Some(k)) = (world.soldiers.get(victim), world.soldiers.get(killer)) else {
            return;
        };
        // the shot's distance, air time and ricochets
        if Some(killer) == player && victim != killer {
            self.shot = Some((shot, KILL_MESSAGE_WAIT - 30));
        }
        if Some(victim) == player && victim != killer {
            self.big = Some((
                format!("Killed by {}", k.name),
                colors::DIE,
                KILL_MESSAGE_WAIT,
            ));
        }
        if Some(killer) == player {
            if victim != killer {
                self.big = Some((
                    format!("You killed {}", v.name),
                    colors::KILL,
                    KILL_MESSAGE_WAIT,
                ));
                // MULTIKILL_MESSAGE[2..17]; more than 17 shows the 9th
                let text = match k.multi_kills {
                    n @ 2..=17 => Some(MULTIKILL_MESSAGES[n as usize - 2]),
                    n if n > 17 => Some(MULTIKILL_MESSAGES[9 - 2]),
                    _ => None,
                };
                if let Some(text) = text {
                    self.big = Some((text.to_string(), colors::KILL, KILL_MESSAGE_WAIT));
                }
            } else {
                self.big = Some((
                    "You killed yourself".to_string(),
                    colors::DIE,
                    KILL_MESSAGE_WAIT,
                ));
            }
        }

        let icon = weapon.and_then(weapon_icon);
        let killer_color = match k.team {
            Team::None => colors::KILLER,
            Team::Alpha => colors::ALPHA_K,
            Team::Bravo => colors::BRAVO_K,
            Team::Charlie => colors::CHARLIE_K,
            Team::Delta => colors::DELTA_K,
            Team::Spectator => 0,
        };
        let victim_color = match v.team {
            Team::None => colors::DEATH,
            Team::Alpha => colors::ALPHA_D,
            Team::Bravo => colors::BRAVO_D,
            Team::Charlie => colors::CHARLIE_D,
            Team::Delta => colors::DELTA_D,
            Team::Spectator => 0,
        };
        let killer_line = format!("{} ({})", k.name, k.kills);
        if victim != killer {
            self.add(killer_line, killer_color, icon);
            self.add(v.name.clone(), victim_color, None);
        } else {
            self.add(killer_line, colors::SPECTATOR_D, icon);
        }
    }
}

pub(super) struct Hud<'a> {
    pub batch: &'a mut DrawBatch,
    pub fonts: &'a Fonts,
    sprites: &'a [Vec<Sprite>],
    /// `_iscala.x`
    pub iscale: f32,
}

impl<'a> Hud<'a> {
    pub fn new(
        batch: &'a mut DrawBatch,
        fonts: &'a Fonts,
        sprites: &'a [Vec<Sprite>],
        size: Vec2,
    ) -> Hud<'a> {
        Hud {
            batch,
            fonts,
            sprites,
            iscale: size.x / 640.0,
        }
    }

    pub fn sprite(&self, id: Interface) -> &Sprite {
        &self.sprites[id.group().id()][id.id()]
    }

    /// A sprite (or the part of it given by `u0..u1` of its width) at `pos`.
    pub fn draw(&mut self, id: Interface, pos: Vec2, size_scale: f32, color: Color, u: (f32, f32)) {
        let sprite = &self.sprites[id.group().id()][id.id()];
        let (tx0, tx1) = sprite.texcoords_x;
        let (ty0, ty1) = sprite.texcoords_y;
        let (t0, t1) = (tx0 + (tx1 - tx0) * u.0, tx0 + (tx1 - tx0) * u.1);
        let size = vec2(sprite.width * (u.1 - u.0), sprite.height) * size_scale;
        let p0 = pos.floor();
        let p1 = p0 + size;
        self.batch.add_quad(
            sprite.texture.as_ref(),
            &[
                vertex(p0, vec2(t0, ty0), color),
                vertex(vec2(p1.x, p0.y), vec2(t1, ty0), color),
                vertex(p1, vec2(t1, ty1), color),
                vertex(vec2(p0.x, p1.y), vec2(t0, ty1), color),
            ],
        );
    }

    /// `GfxDrawSprite(s, x, y, 0, 0, r, color, rc)`: the part `rect` of a sprite (u0, v0,
    /// u1, v1 as fractions) at `pos`, turned by `-r` radians about it.
    fn draw_part(&mut self, id: Interface, pos: Vec2, rect: [f32; 4], r: f32, color: Color) {
        let sprite = &self.sprites[id.group().id()][id.id()];
        let (tx0, tx1) = sprite.texcoords_x;
        let (ty0, ty1) = sprite.texcoords_y;
        let part = Sprite {
            width: sprite.width * (rect[2] - rect[0]),
            height: sprite.height * (rect[3] - rect[1]),
            texcoords_x: (tx0 + (tx1 - tx0) * rect[0], tx0 + (tx1 - tx0) * rect[2]),
            texcoords_y: (ty0 + (ty1 - ty0) * rect[1], ty0 + (ty1 - ty0) * rect[3]),
            texture: sprite.texture,
        };
        self.batch.add_sprite(
            &part,
            color,
            Transform::origin(pos, Vec2::ONE, (-r, Vec2::ZERO)),
        );
    }

    /// A sprite stretched over a rectangle.
    pub fn stretch(&mut self, id: Interface, p0: Vec2, size: Vec2, color: Color) {
        let sprite = self.sprite(id);
        let (tx, ty) = (sprite.texcoords_x, sprite.texcoords_y);
        let texture = sprite.texture;
        let p1 = p0 + size;
        self.batch.add_quad(
            texture.as_ref(),
            &[
                vertex(p0, vec2(tx.0, ty.0), color),
                vertex(vec2(p1.x, p0.y), vec2(tx.1, ty.0), color),
                vertex(p1, vec2(tx.1, ty.1), color),
                vertex(vec2(p0.x, p1.y), vec2(tx.0, ty.1), color),
            ],
        );
    }

    /// A rectangle of one colour.
    pub fn fill(&mut self, p0: Vec2, size: Vec2, color: Color) {
        let p1 = p0 + size;
        self.batch.add_quad(
            None,
            &[
                vertex(p0, Vec2::ZERO, color),
                vertex(vec2(p1.x, p0.y), Vec2::ZERO, color),
                vertex(p1, Vec2::ZERO, color),
                vertex(vec2(p0.x, p1.y), Vec2::ZERO, color),
            ],
        );
    }

    /// `DrawLine`: a horizontal line one pixel high.
    pub fn line(&mut self, pos: Vec2, width: f32, color: Color, pixel: f32) {
        let p0 = pos.floor();
        let p1 = vec2((p0.x + width).floor(), p0.y + pixel);
        self.batch.add_quad(
            None,
            &[
                vertex(p0, Vec2::ZERO, color),
                vertex(vec2(p1.x, p0.y), Vec2::ZERO, color),
                vertex(p1, Vec2::ZERO, color),
                vertex(vec2(p0.x, p1.y), Vec2::ZERO, color),
            ],
        );
    }

    /// `GfxDrawSprite(s, x, y, sx, sy, rx, ry, r, color)`: scaled, turned by `-r` about
    /// `center`.
    pub fn draw_turned(
        &mut self,
        id: Interface,
        pos: Vec2,
        scale: Vec2,
        center: Vec2,
        r: f32,
        color: Color,
    ) {
        let sprite = &self.sprites[id.group().id()][id.id()];
        self.batch
            .add_sprite(sprite, color, Transform::origin(pos, scale, (-r, center)));
    }

    pub fn text(&mut self, style: FontStyle, text: &str, pos: Vec2, color: Color) {
        self.fonts.draw(self.batch, style, text, pos, color, None);
    }

    fn text_right(&mut self, style: FontStyle, text: &str, pos: Vec2, color: Color) {
        let width = self.fonts.measure(style, text).x;
        self.text(style, text, pos - vec2(width, 0.0), color);
    }
}

pub fn render_hud(batch: &mut DrawBatch, fonts: &Fonts, sprites: &[Vec<Sprite>], state: &HudState) {
    if let Some(spectator) = state.spectator.and_then(|id| state.world.soldiers.get(id)) {
        let mut hud = Hud::new(batch, fonts, sprites, state.size);
        render_spectator_names(&mut hud, state, spectator);
    }
    let Some((my_id, me)) = state
        .player
        .and_then(|id| state.world.soldiers.get(id).map(|me| (id, me)))
    else {
        return;
    };
    let mut hud = Hud::new(batch, fonts, sprites, state.size);
    let world = state.world;

    // bonus: the whole screen tinted (`ui_bonuscolors`)
    let tint = match me.bonus_style {
        _ if !state.cvars.bool("ui_bonuscolors") => None,
        Bonus::Flamegod => Some(rgba(0xFF, 0xFF, 0x00, 62)),
        Bonus::Predator => Some(rgba(0xFE, 0x00, 0xDC, 82)),
        Bonus::Berserker => Some(rgba(0xFE, 0x00, 0x00, 82)),
        Bonus::None => None,
    };
    if let Some(color) = tint {
        // stretched over the whole interface
        hud.stretch(Interface::Overlay, Vec2::ZERO, state.size, color);
    }

    render_panel(&mut hud, state, me);
    render_panel_texts(&mut hud, state, me);

    render_status(&mut hud, state, me);
    if state.spectator.is_none() {
        render_respawn_texts(&mut hud, state, me);
    }
    super::console::render_chat_bubbles(&mut hud, state, me);

    // spawn protection countdown above the player (survival mode)
    let to_screen = |p: Vec2| (p - state.camera) / state.zoom + state.size / 2.0;
    if world.config.survival_mode && me.ceasefire_counter > 0 {
        let p = to_screen(me.skeleton.pos(9) + vec2(-2.0, -15.0));
        let text = (me.ceasefire_counter / 60 + 1).to_string();
        hud.text(FontStyle::Small, &text, p, rgb(0xEE, 0xEE, 0xEE));
    }

    // the crosshair, bigger while binked or moving inaccurately, half size with the sniper
    // line, red or green over a player
    if !state.menus.any_active() && !me.dead_meat && !world.game.ended() && !state.no_crosshair {
        let sniper_line = state.cvars.bool("ui_sniperline") && state.cvars.bool("sv_sniperline");
        let cursor = hud.sprite(Interface::Cursor);
        let size = vec2(cursor.width, cursor.height);
        let inaccuracy = f32::from(me.hit_spray_counter) + me.moveacc() * 100.0;
        let base = if sniper_line { 0.5 } else { 1.0 };
        let mut scale = base;
        if inaccuracy > 0.0 {
            scale += inaccuracy.powf(0.6) / 20.0 * base;
        }
        let color = match cursor_target(state, my_id, me) {
            Some((_, true)) => rgba(0x33, 0xFF, 0x33, state.alpha.saturating_sub(50)),
            Some((_, false)) => rgba(0xFF, 0x33, 0x33, state.alpha.saturating_sub(50)),
            None if sniper_line => rgba(255, 255, 255, state.alpha / 2),
            None => rgba(255, 255, 255, state.alpha),
        };

        // the sniper line from the hand to the cursor
        if sniper_line {
            let hand = to_screen(me.skeleton.pos(15));
            let roto = (state.mouse - hand).length();
            if roto < 1200.0 {
                let p = hand.floor();
                let turn = angle2points(p, state.mouse);
                let alpha = (roto / 240.0 * 32.0).round_ties_even().clamp(0.0, 255.0) as u8;
                hud.draw_turned(
                    Interface::Sight,
                    p - vec2(1.0, 1.0),
                    vec2(roto / 240.0, roto / 480.0),
                    vec2(1.0, 1.0),
                    -turn,
                    rgba(255, 255, 255, alpha),
                );
            }
        }

        let pos = state.mouse - size / 2.0 * scale;
        hud.draw(Interface::Cursor, pos, scale, color, (0.0, 1.0));
    }

    // the player indicator: an arrow over your head
    if state.cvars.bool("ui_playerindicator") {
        let arrow = hud.sprite(Interface::Arrow);
        let half = vec2(arrow.width, arrow.height) / 2.0;
        let mut pos = to_screen(me.skeleton.pos(12)) - half - vec2(0.0, 15.0);
        let alpha = if me.alpha < 255 && !world.config.survival_mode {
            (me.ceasefire_counter * 2 + 75).clamp(0, 255) as u8
        } else {
            pos.y += 2.0 * (5.1 * state.elapsed).sin() as f32;
            100
        };
        let color = rgba(255, 255, 255, alpha);
        hud.draw(Interface::Arrow, pos, 1.0, color, (0.0, 1.0));
    }

    if state.spectator.is_none() {
        render_player_names(&mut hud, state, my_id, me);
    }

    // the ping dot: bigger and redder with more lag (invisible without)
    let layout = state.layout;
    if layout.shown.ping && state.menus.player_names {
        let ping = f32::from(me.ping);
        let color = match me.ping {
            0..=50 => 0x00FF00,
            51..=100 => 0x22FF00,
            101..=150 => 0x54C700,
            151..=200 => 0x76A700,
            201..=250 => 0x938800,
            251..=300 => 0xA17700,
            301..=350 => 0xCC4800,
            _ => 0xFF0000,
        };
        let alpha = me.ping.min(255) as u8;
        let pos = vec2(layout.ping.0 * hud.iscale, layout.ping.1);
        let scale = vec2(0.5 + ping / 600.0, 0.45 + ping / 600.0);
        let color = rgba((color >> 16) as u8, (color >> 8) as u8, color as u8, alpha);
        hud.draw_turned(Interface::Dot, pos, scale, Vec2::ZERO, 0.0, color);
    }
}

/// `CURSORSPRITE_DISTANCE`
const CURSOR_SPRITE_DISTANCE: f32 = 15.0;

/// `CursorText`, `CursorFriendly`: the player under the crosshair, a teammate with its
/// health. Standing players only, unless they're teammates or someone is dead.
fn cursor_target(state: &HudState, my_id: SoldierId, me: &Soldier) -> Option<(String, bool)> {
    let world = state.world;
    let mode = world.config.game_mode;
    let cursor = state.camera + (state.mouse - state.size / 2.0) * state.zoom;
    // IsNotSolo and IsInSameTeam
    let teammate =
        |s: &Soldier| mode != GameMode::Deathmatch && s.team != Team::None && s.team == me.team;
    let (_, target) = world.soldiers.iter().find(|&(id, s)| {
        s.active
            && id != my_id
            && s.bonus_style != Bonus::Predator
            && (s.position == POS_STAND || teammate(s) || me.dead_meat || s.dead_meat)
            && (s.visible > 40 || !world.config.realistic_mode)
            && s.particle.pos.distance(cursor) < CURSOR_SPRITE_DISTANCE
    })?;
    Some(if mode.is_team_game() && target.team == me.team {
        let health = (target.health / world.config.start_health() * 100.0).round_ties_even();
        (format!("{} {health}%", target.name), true)
    } else {
        (target.name.clone(), false)
    })
}

/// `Angle2Points`
fn angle2points(p1: Vec2, p2: Vec2) -> f32 {
    use std::f32::consts::PI;
    if p2.x != p1.x {
        let a = ((p2.y - p1.y) / (p2.x - p1.x)).atan();
        if p1.x > p2.x { a + PI } else { a }
    } else if p2.y > p1.y {
        PI / 2.0
    } else if p2.y < p1.y {
        -PI / 2.0
    } else {
        0.0
    }
}

/// `RenderPlayerName`'s colour: flag holders, the dead, the others.
fn player_name_color(soldier: &Soldier, alpha: u8) -> Color {
    let color = if soldier.holds_flag {
        0xDCDC33
    } else if soldier.dead_meat {
        0x983333
    } else {
        0x99DF99
    };
    rgba((color >> 16) as u8, (color >> 8) as u8, color as u8, alpha)
}

/// `RenderPlayerNames` of a spectator: everyone in a team, named over their heads (kept on
/// the screen), fainter the farther from the spectator's body.
fn render_spectator_names(hud: &mut Hud, state: &HudState, spectator: &Soldier) {
    let world = state.world;
    if !state.menus.player_names {
        return;
    }
    let dy = 20.0 / state.zoom.ln().max(1.0);
    for soldier in world.soldiers.values() {
        let in_team = !matches!(soldier.team, Team::None | Team::Spectator);
        let hidden = world.config.realistic_mode && soldier.visible == 0;
        if !soldier.active || !in_team || hidden {
            continue;
        }
        let head = soldier.skeleton.pos(7);
        let p = (head - state.camera) / state.zoom + state.size / 2.0 + vec2(0.0, dy);
        let size = hud.fonts.measure(FontStyle::WeaponsMenu, &soldier.name);
        let pos = vec2(
            (p.x - size.x / 2.0).min(state.size.x - size.x).max(0.0),
            (p.y - size.y / 2.0).min(state.size.y - size.y).max(0.0),
        );
        let d = (spectator.skeleton.pos(7) - head).abs().max(Vec2::ONE);
        let alpha = (50 + (100_000.0 / (d.x + d.y / 2.0)).round() as i64).min(255) as u8;
        let color = player_name_color(soldier, alpha);
        hud.text(FontStyle::WeaponsMenu, &soldier.name, pos, color);
    }
}

/// `RenderPlayerNames`: teammates out of the screen, their names at its edge.
fn render_player_names(hud: &mut Hud, state: &HudState, my_id: SoldierId, me: &Soldier) {
    if !state.menus.player_names {
        return;
    }
    let world = state.world;
    if me.team == Team::None {
        return;
    }
    let dy = 5.0 / state.zoom.ln().max(1.0);
    for (id, soldier) in world.soldiers.iter() {
        if id == my_id || !soldier.active || soldier.team != me.team {
            continue;
        }
        let head = soldier.skeleton.pos(7);
        let p = (head - state.camera) / state.zoom + state.size / 2.0 + vec2(0.0, dy);
        let inside = p.x >= 0.0 && p.x <= state.size.x && p.y >= 0.0 && p.y <= state.size.y;
        if inside {
            continue;
        }

        let size = hud.fonts.measure(FontStyle::WeaponsMenu, &soldier.name);
        let pos = vec2(
            (p.x - size.x / 2.0).min(state.size.x - size.x).max(0.0),
            p.y.min(state.size.y - size.y).max(0.0),
        );
        let d = (me.skeleton.pos(7) - head).abs().max(Vec2::ONE);
        let alpha = (50 + (100_000.0 / (d.x + d.y / 2.0)).round() as i64).min(255) as u8;
        let color = player_name_color(soldier, alpha);
        hud.text(FontStyle::WeaponsMenu, &soldier.name, pos, color);
    }
}

/// The whole of a sprite (`rc` of `GfxDrawSprite`: u0, v0, u1, v1).
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// A layout position moving with `anchor` (`RelInfo`): the anchor follows the screen
/// width, the offset from it doesn't.
fn anchored(hud: &Hud, (x, y): (f32, f32), anchor: (f32, f32)) -> Vec2 {
    vec2(
        (anchor.0 * hud.iscale).floor() + (x - anchor.0),
        anchor.1.floor() + (y - anchor.1),
    )
}

/// `RenderBar`: the filled part `p` of a bar sprite, horizontal or vertical (a text-style
/// bar is a number, drawn with the texts).
#[allow(clippy::too_many_arguments)]
fn layout_bar(
    hud: &mut Hud,
    id: Interface,
    bar: &Bar,
    anchor: (f32, f32),
    p: f32,
    left: bool,
    color: Color,
) {
    if bar.style == BarStyle::Text {
        return;
    }
    let p = p.clamp(0.0, 1.0);
    let height = hud.sprite(id).height;
    let mut pos = anchored(hud, (bar.x, bar.y), anchor);
    let vertical = bar.style == BarStyle::Vertical;
    let rect = match (left, vertical) {
        (true, false) => [0.0, 0.0, p, 1.0],
        (false, false) => [1.0 - p, 0.0, 1.0, 1.0],
        (true, true) => {
            pos.y += height * (1.0 - p);
            [0.0, 1.0 - p, 1.0, 1.0]
        }
        (false, true) => {
            pos.y += height * (1.0 - p);
            [0.0, 0.0, 1.0, p]
        }
    };
    hud.draw_part(id, pos, rect, bar.rotate.to_radians(), color);
}

/// The panel's icons and bars (`RenderInterface`): health, vest, ammo or reload, fire
/// interval, jets and grenades, where the interface layout puts them.
fn render_panel(hud: &mut Hud, state: &HudState, me: &Soldier) {
    let world = state.world;
    let layout = state.layout;
    let (shown, right, anchors) = (layout.shown, layout.right, layout.anchors);
    let color = rgba(255, 255, 255, layout.alpha);
    let icon = |hud: &mut Hud, id: Interface, icon: &Icon| {
        let pos = vec2(icon.x * hud.iscale, icon.y).floor();
        hud.draw_part(id, pos, WHOLE, icon.rotate.to_radians(), color);
    };

    if shown.health {
        icon(hud, Interface::Health, &layout.health_ico);
        let p = me.health / world.config.start_health();
        let left = !right.health_bar;
        layout_bar(
            hud,
            Interface::HealthBar,
            &layout.health_bar,
            anchors.health_bar,
            p,
            left,
            color,
        );
    }
    if shown.vest && me.vest > 0.0 {
        let p = me.vest / DEFAULT_VEST;
        let left = !right.vest_bar;
        layout_bar(
            hud,
            Interface::VestBar,
            &layout.vest_bar,
            anchors.health_bar,
            p,
            left,
            color,
        );
    }

    // ammo, or the reload progress
    let weapon = *me.primary_weapon();
    if shown.ammo {
        icon(hud, Interface::Ammo, &layout.ammo_ico);
        let bar = &layout.ammo_bar;
        if weapon.ammo_count == 0 && weapon.kind != WeaponKind::Spas12 {
            let p =
                1.0 - f32::from(weapon.reload_time_count) / f32::from(weapon.reload_time.max(1));
            let left = !right.reload_bar;
            layout_bar(
                hud,
                Interface::ReloadBar,
                bar,
                anchors.ammo_bar,
                p,
                left,
                color,
            );
        } else if weapon.ammo_count > 0 {
            let p = f32::from(weapon.ammo_count) / f32::from(weapon.ammo.max(1));
            let left = !right.ammo_bar;
            layout_bar(
                hud,
                Interface::ReloadBar,
                bar,
                anchors.ammo_bar,
                p,
                left,
                color,
            );
        }
    }

    // fire interval: the frame at the bar's place, the bar at the icon's
    if shown.fire {
        let fire = &layout.fire_bar;
        let back = anchored(hud, (fire.x, fire.y), anchors.fire_bar).floor();
        hud.draw_part(
            Interface::FireBarR,
            back,
            WHOLE,
            fire.rotate.to_radians(),
            color,
        );
        let bar = Bar {
            x: layout.fire_ico.x,
            y: layout.fire_ico.y,
            rotate: layout.fire_ico.rotate,
            ..*fire
        };
        let p = f32::from(weapon.fire_interval_count) / f32::from(weapon.fire_interval.max(1));
        layout_bar(
            hud,
            Interface::FireBar,
            &bar,
            anchors.fire_bar,
            p,
            !right.fire_bar,
            color,
        );
    }

    if shown.jet {
        icon(hud, Interface::Jet, &layout.jet_ico);
        if world.map.start_jet > 0 {
            let p = me.jets_count as f32 / world.map.start_jet as f32;
            let left = !right.jet_bar;
            layout_bar(
                hud,
                Interface::JetBar,
                &layout.jet_bar,
                anchors.jet_bar,
                p,
                left,
                color,
            );
        }
    }

    // grenades, in a row or a column
    let nade = match me.tertiary_weapon().kind {
        WeaponKind::FragGrenade => Some(Interface::Nade),
        WeaponKind::ClusterGrenade => Some(Interface::ClusterNade),
        _ => None,
    };
    let nades = &layout.nades;
    if let Some(nade) = nade.filter(|_| shown.nades && nades.style != BarStyle::Text) {
        let size = vec2(hud.sprite(nade).width, hud.sprite(nade).height);
        let base = anchored(hud, (nades.x, nades.y), anchors.nades_bar);
        for j in 1..=me.tertiary_weapon().ammo_count {
            let j = f32::from(j);
            let pos = if nades.style == BarStyle::Vertical {
                base + vec2(0.0, -size.y * j + size.y * 6.0)
            } else {
                base + vec2(size.x * j, 0.0)
            };
            hud.draw_part(nade, pos.floor(), WHOLE, 0.0, color);
        }
    }
}

/// The panel's texts (`RenderPlayerInterfaceTexts`): text-style bars as percentages, the
/// grenade count, the bullets left and the weapon's name.
fn render_panel_texts(hud: &mut Hud, state: &HudState, me: &Soldier) {
    if me.dead_meat {
        return;
    }
    let world = state.world;
    let layout = state.layout;
    let (shown, anchors) = (layout.shown, layout.anchors);
    let alpha = layout.alpha;
    let white = rgba(255, 255, 255, alpha);
    let weapon = *me.primary_weapon();
    let percent = |t: f32| format!("{}%", (t * 100.0).trunc() as i32);
    let text = |hud: &mut Hud, at: (f32, f32), anchor: (f32, f32), s: &str| {
        let pos = anchored(hud, at, anchor);
        hud.text(FontStyle::Menu, s, pos, white);
    };

    let text_bar = |bar: &Bar| (bar.style == BarStyle::Text).then_some((bar.x, bar.y));
    if shown.health
        && let Some(at) = text_bar(&layout.health_bar)
    {
        let t = me.health / world.config.start_health();
        text(hud, at, anchors.health_bar, &percent(t));
    }
    if shown.ammo
        && weapon.ammo_count == 0
        && weapon.kind != WeaponKind::Spas12
        && let Some(at) = text_bar(&layout.ammo_bar)
    {
        let t = f32::from(weapon.reload_time_count) / f32::from(weapon.reload_time.max(1));
        text(hud, at, anchors.ammo_bar, &percent(1.0 - t));
    }
    if shown.jet
        && world.map.start_jet > 0
        && let Some(at) = text_bar(&layout.jet_bar)
    {
        let t = me.jets_count as f32 / world.map.start_jet as f32;
        text(hud, at, anchors.jet_bar, &percent(t));
    }
    if shown.vest
        && me.vest > 0.0
        && let Some(at) = text_bar(&layout.vest_bar)
    {
        text(
            hud,
            at,
            anchors.health_bar,
            &percent(me.vest / DEFAULT_VEST),
        );
    }
    if shown.nades
        && let Some(at) = text_bar(&layout.nades)
    {
        let count = me.tertiary_weapon().ammo_count.to_string();
        text(hud, at, anchors.nades_bar, &count);
    }

    if shown.bullets {
        let pos = anchored(hud, layout.bullets, anchors.ammo_bar);
        let count = weapon.ammo_count.to_string();
        let color = rgba(242, 244, 40, alpha);
        if layout.right.bullets {
            hud.text_right(FontStyle::Menu, &count, pos, color);
        } else {
            hud.text(FontStyle::Menu, &count, pos, color);
        }
    }
    if shown.weapon && weapon.kind != WeaponKind::NoWeapon {
        let pos = anchored(hud, layout.weapon, anchors.ammo_bar);
        let color = rgba(255, 245, 177, alpha);
        if layout.right.weapon {
            hud.text_right(FontStyle::WeaponsMenu, weapon.name, pos, color);
        } else {
            hud.fonts.draw(
                hud.batch,
                FontStyle::WeaponsMenu,
                weapon.name,
                pos,
                color,
                None,
            );
        }
    }
}

/// Rank, kills and the kill limit; the bonus and its time left.
fn render_status(hud: &mut Hud, state: &HudState, me: &Soldier) {
    let world = state.world;
    let layout = state.layout;
    if !layout.shown.status {
        return;
    }
    let (x, y) = (layout.status.0 * hud.iscale, layout.status.1);

    let sorted = super::scoreboard::sorted_players(world);
    let spectators = world.soldiers.values().filter(|s| s.is_spectator()).count();
    let players = sorted.len() - spectators;
    let rank = sorted.iter().position(|r| Some(r.id) == state.player);
    if let Some(rank) = rank.filter(|&r| r < players) {
        hud.text(
            FontStyle::Small,
            &format!("{}/{players}", rank + 1),
            vec2(x, y),
            rgb(88, 255, 90),
        );
    }
    let lead = if rank == Some(0) && players > 1 {
        let diff = me.kills - sorted[1].kills;
        format!("{} ({}{diff})", me.kills, if diff > 0 { "+" } else { "" })
    } else {
        let best = sorted.first().map_or(0, |r| r.kills);
        format!("{} ({})", me.kills, me.kills - best)
    };
    hud.text(FontStyle::Small, &lead, vec2(x, y + 10.0), rgb(255, 55, 50));
    hud.text(
        FontStyle::Small,
        &world.config.kill_limit.to_string(),
        vec2(x, y + 20.0),
        rgb(114, 120, 255),
    );

    let bonus = match me.bonus_style {
        Bonus::Flamegod => "Flame God",
        Bonus::Predator => "Predator",
        Bonus::Berserker => "Berserker",
        Bonus::None => return,
    };
    let text = format!("{bonus} - {:.1}", me.bonus_time as f32 / 60.0);
    hud.text(
        FontStyle::Menu,
        &text,
        vec2(190.0 * hud.iscale, 435.0),
        rgb(245, 40, 50),
    );
}

/// `RenderRespawnAndSurvivalTexts`
fn render_respawn_texts(hud: &mut Hud, state: &HudState, me: &Soldier) {
    let world = state.world;
    let survival = world.config.survival_mode;
    let round_over = world.game.survival_end_round;

    if me.dead_meat || round_over {
        let scale = hud.sprite(Interface::Back).width / 64.0;
        let color = rgba(255, 255, 255, (f32::from(state.alpha) * 0.56) as u8);
        let pos = vec2(180.0 * hud.iscale, 1.0);
        hud.stretch(Interface::Back, pos, vec2(300.0, 22.0) * scale, color);
    }

    let alive = |s: &&Soldier| s.active && !s.dead_meat;
    let (text, color) = if !survival && me.respawn_counter > 0 {
        let seconds = me.respawn_counter as f32 / 60.0;
        (format!("Respawn in... {seconds:.1}"), rgb(255, 65, 55))
    } else if survival && me.dead_meat && !round_over {
        let text = if world.config.game_mode.is_team_game() {
            let left = world
                .soldiers
                .values()
                .filter(alive)
                .filter(|s| s.team == me.team)
                .count();
            format!("{left} team players left")
        } else {
            format!(
                "{} players left",
                world.soldiers.values().filter(alive).count()
            )
        };
        (text, rgb(115, 255, 100))
    } else if round_over && me.dead_meat {
        let seconds = me.respawn_counter as f32 / 60.0;
        (format!("End of round...{seconds:.1}"), rgb(115, 255, 100))
    } else if round_over {
        ("You have survived".to_string(), rgb(155, 245, 100))
    } else {
        return;
    };
    hud.text(FontStyle::Menu, &text, vec2(200.0 * hud.iscale, 4.0), color);
}

/// Texts shown with or without a player: the big message, the kill console and the
/// console with the chat input.
pub fn render_texts(
    batch: &mut DrawBatch,
    fonts: &Fonts,
    sprites: &[Vec<Sprite>],
    state: &HudState,
) {
    let mut hud = Hud::new(batch, fonts, sprites, state.size);

    // the name under the crosshair
    let menus = state.menus;
    let me = state
        .player
        .and_then(|id| state.world.soldiers.get(id).map(|s| (id, s)));
    if let Some((my_id, me)) = me
        && !state.world.game.ended()
        && !menus.team_active
        && !menus.esc_active
        && let Some((text, _)) = cursor_target(state, my_id, me)
    {
        let width = hud.fonts.measure(FontStyle::Small, &text).x;
        let pos = vec2(state.mouse.x - 0.5 * width, state.mouse.y + 10.0);
        hud.text(FontStyle::Small, &text, pos, rgba(255, 255, 255, 0x77));
    }

    render_messages(&mut hud, state);
    super::console::render_console(&mut hud, state, state.chat);

    // free camera / following player
    let followed = state.follow.and_then(|id| state.world.soldiers.get(id));
    let text = match (state.follow, followed) {
        (None, _) => Some(("Free Camera".to_string(), rgb(205, 205, 205))),
        (Some(id), Some(soldier)) if Some(id) != state.local_player() => {
            let fade = if soldier.dead_meat { 105 } else { 0 };
            let color = rgb(205, 205 - fade, 205 - fade);
            Some((format!("Following {}", soldier.name), color))
        }
        _ => None,
    };
    if let Some((text, color)) = text {
        let width = hud.fonts.measure(FontStyle::Small, &text).x;
        let pos = vec2((state.size.x - width) / 2.0, 430.0);
        hud.text(FontStyle::Small, &text, pos, color);
    }

    if let Some(info) = state.con_info {
        render_con_info(&mut hud, info);
    }

    // recording
    if state.recording {
        let alpha = ((5.1 * state.elapsed / 2.0).sin().abs() * 255.0) as u8;
        let pos = vec2(612.0 * hud.iscale, 1.0);
        hud.text(FontStyle::Small, "REC", pos, rgba(195, 0, 0, alpha));
    }

    // action snap
    if state.snap_offered {
        let alpha = 150 + ((5.1 * state.elapsed).sin() * 100.0).round().abs() as u8;
        let text = "[[ Press F5 to View Screen Cap ]]";
        let pos = vec2(30.0 * hud.iscale, 412.0);
        hud.text(FontStyle::Small, text, pos, rgba(230, 65, 60, alpha));
    }

    // shot distance
    if let Some((shot, _)) = state.messages.shot {
        let alpha = 150 + ((5.1 * state.elapsed).sin() * 100.0).round().abs() as u8;
        let color = rgba(230, 65, 60, alpha);
        let distance = format!("DISTANCE: {:.2}m", shot.distance);
        hud.text(
            FontStyle::Small,
            &distance,
            vec2(390.0 * hud.iscale, 431.0),
            color,
        );
        let air = format!("AIRTIME: {:.2}s", shot.life);
        hud.text(
            FontStyle::Small,
            &air,
            vec2(228.0 * hud.iscale, 431.0),
            color,
        );
        if shot.ricochets > 0 {
            let ricochets = format!("RICOCHETS: {}", shot.ricochets);
            hud.text(
                FontStyle::Small,
                &ricochets,
                vec2(62.0 * hud.iscale, 431.0),
                color,
            );
        }
    }

    // bullet time: black bars over the top and bottom
    if state.wide_cut {
        let black = rgba(0, 0, 0, 255);
        hud.fill(Vec2::ZERO, vec2(state.size.x, 80.0), black);
        hud.fill(vec2(0.0, 400.0), vec2(state.size.x, 80.0), black);
    }
}

/// The GameStats texts (`ConInfoShow`): the frame rate and ping, and a demo's time or the
/// connection's numbers.
fn render_con_info(hud: &mut Hud, info: &super::interface::ConInfo) {
    let color = rgb(239, 170, 200);
    let x = 460.0 * hud.iscale;
    hud.text(
        FontStyle::Small,
        &format!("FPS: {}", info.fps),
        vec2(x, 10.0),
        color,
    );
    if let Some(ping) = info.ping {
        let pos = vec2(550.0 * hud.iscale, 10.0);
        hud.text(FontStyle::Small, &format!("Ping: {ping}"), pos, color);
    }
    if let Some((tick, length)) = info.demo {
        let clock = |ticks: usize| format!("{:02}:{:02}", ticks / 60 / 60, ticks / 60 % 60);
        let text = format!(
            "Demo: {} / {} ({tick} / {length})",
            clock(tick),
            clock(length)
        );
        hud.text(FontStyle::Small, &text, vec2(x, 80.0), color);
    } else if let Some(net) = info.net {
        let lines = [
            format!("Ping: {}", (net.rtt * 1000.0).round()),
            format!("Packet Loss: {:.0}%", net.packet_loss * 100.0),
            format!("Traffic Out: {:.1} B/s", net.sent),
            format!("Traffic In: {:.1} B/s", net.received),
        ];
        for (i, line) in lines.iter().enumerate() {
            let pos = vec2(x, 40.0 + 10.0 * i as f32);
            hud.text(FontStyle::Smallest, line, pos, color);
        }
    }
}

/// The big message and the kill console (`RenderKillConsoleTexts` and its icons).
fn render_messages(hud: &mut Hud, state: &HudState) {
    let messages = state.messages;

    if let Some((text, color, delay)) = &messages.big {
        let alpha = (3 * delay + 25).clamp(0, (color >> 24) as i32) as u8;
        let mut c = argb(*color);
        c.set_a(alpha);
        // texts wider than 70% of the screen shrink to that (`BigScale`)
        let width = hud.fonts.measure(FontStyle::Big, text).x;
        let zoom = (0.7 * state.size.x / width).min(1.0);
        // (above the bullet time bar)
        let dy = if state.wide_cut { -30.0 } else { 0.0 };
        let pos = vec2(
            (state.size.x - width * zoom) / 2.0,
            420.0 - hud.fonts.ascent(FontStyle::Big) * zoom + dy,
        );
        let shadow = rgba(
            0,
            0,
            0,
            ((alpha as f32 / 255.0).powi(4) * alpha as f32) as u8,
        );
        hud.fonts
            .draw_scaled(hud.batch, FontStyle::Big, text, pos, c, Some(shadow), zoom);
    }

    if !state.cvars.bool("ui_killconsole") {
        return;
    }
    // narrow screens: it fades behind the scoreboard and while typing
    let (text_alpha, icon_alpha) = match () {
        _ if state.size.x >= 1024.0 => (245, 255),
        _ if state.chat.active() => (180, 150),
        _ if state.menus.scores_shown() => (80, 50),
        _ => (245, 255),
    };
    let mut dy = 0.0;
    for (i, line) in messages.kills.iter().enumerate() {
        if line.icon.is_some() {
            dy += KILL_CONSOLE_SEPARATE_HEIGHT;
        }
        let style = if line.text.chars().count() > 14 {
            FontStyle::Smallest
        } else {
            FontStyle::WeaponsMenu
        };
        let y = 60.0 + i as f32 * 10.0 + dy;
        let mut color = argb(line.color);
        color.set_a(text_alpha);
        let width = hud.fonts.measure(style, &line.text).x;
        hud.fonts.draw(
            hud.batch,
            style,
            &line.text,
            vec2(595.0 * hud.iscale - width, y),
            color,
            None,
        );

        if let Some(icon) = line.icon {
            let y = i as f32 * 10.0 + 59.0 + dy;
            hud.draw(
                icon,
                vec2(605.0 * hud.iscale, y),
                0.8,
                rgba(255, 255, 255, icon_alpha),
                (0.0, 1.0),
            );
        }
    }
}
