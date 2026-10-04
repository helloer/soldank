//! The console lines (`MainConsole`, and `BigConsole` while typing), the chat input line
//! and the chat messages over the players (`RenderConsoleTexts`, `RenderChatInput`,
//! `RenderChatTexts`).

use super::hud::{Hud, HudState};
use super::*;
use crate::chat::{Chat, ChatKind};

/// `BigConsole.CountMax`: 85% of the screen in lines of `font_consolelineheight` (1.5)
/// times the small font size (9).
pub const HISTORY_LINES: usize = 30;
/// Line height of the console texts.
const LINE_HEIGHT: f32 = 1.5 * 9.0;
/// `MORECHATTEXT`: longer messages don't show over the player.
const MORE_CHAT_TEXT: usize = 60;

/// Opensoldat's ARGB message colors.
pub mod colors {
    pub const DEFAULT: u32 = 0xEECCFFAA;
    pub const DEBUG: u32 = 0xEEFF8989;
    pub const GAME: u32 = 0xEE71F981;
    pub const MUSIC: u32 = 0xEEADFE99;
    pub const WARNING: u32 = 0xEEE36952;
    pub const ENTER: u32 = 0xF1C3C3C3;
    pub const CHAT: u32 = 0xEEEFFEEA;
    pub const TEAM_CHAT: u32 = 0xEEFEDA7C;
    pub const SERVER: u32 = 0xF9FBDA22;
    pub const CLIENT: u32 = 0xF9FCD822;
    pub const MODE: u32 = 0xEE81DA41;
    pub const VOTE: u32 = 0xEEDDEE99;
    pub const ABOVE_CHAT: u32 = 0xFDFDF9;
}

/// A scrolling list of coloured lines (`TConsole`).
#[derive(Debug)]
pub struct TextConsole {
    lines: Vec<(String, u32)>,
    scroll_tick: i32,
    /// Ticks a line stays (`ScrollTickMax`), and the wait after a new one
    /// (`NewMessageWait`).
    scroll_tick_max: i32,
    new_message_wait: i32,
}

impl TextConsole {
    pub fn new(scroll_tick_max: i32, new_message_wait: i32) -> TextConsole {
        TextConsole {
            lines: Vec::new(),
            scroll_tick: 0,
            scroll_tick_max,
            new_message_wait,
        }
    }

    /// `ConsoleAdd`: with `count_max` lines the oldest goes.
    pub fn add(&mut self, text: String, color: u32, count_max: usize) {
        self.lines.push((text, color));
        self.scroll_tick = -self.new_message_wait;
        if self.lines.len() >= count_max.max(1) {
            self.scroll();
        }
    }

    /// `ScrollConsole`
    fn scroll(&mut self) {
        if !self.lines.is_empty() {
            self.lines.remove(0);
        }
        self.scroll_tick = 0;
    }

    pub fn tick(&mut self) {
        self.scroll_tick += 1;
        if self.scroll_tick == self.scroll_tick_max {
            self.scroll();
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = &(String, u32)> {
        self.lines.iter()
    }
}

/// A message over a player's head (`ChatMessage`, `ChatDelay`, `ChatTeam`).
#[derive(Debug, Clone)]
pub struct ChatBubble {
    pub id: SoldierId,
    pub text: String,
    pub delay: i32,
    pub team: bool,
}

fn rgb_alpha(c: u32, a: u8) -> Color {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, a)
}

/// `RenderConsoleTexts` and `RenderChatInput`.
pub(super) fn render_console(hud: &mut Hud, state: &HudState, chat: &Chat) {
    let messages = state.messages;
    let menus = state.menus;

    if state.cvars.bool("ui_console") {
        let typing = chat.active();
        let console = if typing {
            &messages.history
        } else {
            &messages.main
        };
        // (and behind the escape menu's key help)
        let help = menus.esc_active && state.cvars.int("cl_runs") < 3;
        let dim =
            menus.scores_shown() || menus.team_active || (typing && menus.limbo_active) || help;
        let alpha = if dim { 60 } else { 255 };
        for (i, (text, color)) in console.lines().enumerate() {
            if text.is_empty() {
                continue;
            }
            let style = if hud.fonts.measure(FontStyle::Small, text).x > state.size.x - 10.0 {
                FontStyle::Smallest
            } else {
                FontStyle::Small
            };
            let pos = vec2(5.0, 1.0 + i as f32 * LINE_HEIGHT);
            hud.text(style, text, pos, rgb_alpha(*color, alpha));
        }
    }

    if !chat.active() {
        return;
    }
    let (prefix, color) = match chat.kind() {
        ChatKind::Public | ChatKind::VoteReason => ("Say:", colors::CHAT),
        ChatKind::Team => ("Team Say:", colors::TEAM_CHAT),
        ChatKind::Command => ("Cmd: ", colors::ENTER),
    };
    if chat.kind() == ChatKind::VoteReason {
        let prompt = "Type reason for vote:";
        hud.text(
            FontStyle::Small,
            prompt,
            vec2(5.0, 390.0),
            rgb(254, 124, 124),
        );
    }
    let text = chat.text();
    let line = format!("{prefix}{text}");
    let style = if hud.fonts.measure(FontStyle::Small, &line).x >= state.size.x - 80.0 {
        FontStyle::Smallest
    } else {
        FontStyle::Small
    };
    // drawn on its baseline
    let ascent = hud.fonts.ascent(style);
    let baseline = 420.0;
    hud.text(
        style,
        &line,
        vec2(5.0, baseline - ascent),
        rgb_alpha(color, 255),
    );

    // the cursor blinks, starting visible after each change
    let t = state.elapsed - chat.changed_at;
    if t - t.floor() <= 0.5 {
        let before: String = prefix
            .chars()
            .chain(text.chars().take(chat.cursor()))
            .collect();
        let px = state.pixel;
        let x = ((5.0 + hud.fonts.measure(style, &before).x) / px).floor() * px + 2.0 * px;
        let y = ((baseline - ascent) / px).floor() * px;
        let h = ((1.4 * ascent) / px).floor() * px;
        let color = rgb(255, 230, 170);
        hud.batch.add_quad(
            None,
            &[
                vertex(vec2(x, y), Vec2::ZERO, color),
                vertex(vec2(x + px, y), Vec2::ZERO, color),
                vertex(vec2(x + px, y + h), Vec2::ZERO, color),
                vertex(vec2(x, y + h), Vec2::ZERO, color),
            ],
        );
    }
}

/// `RenderChatTexts`: what players said, over their heads for a while.
pub(super) fn render_chat_bubbles(hud: &mut Hud, state: &HudState, me: &Soldier) {
    let world = state.world;
    let to_screen = |p: Vec2| (p - state.camera) / state.zoom + state.size / 2.0;
    for bubble in &state.messages.chats {
        let Some(soldier) = world.soldiers.get(bubble.id).filter(|s| s.active) else {
            continue;
        };
        let same_team = soldier.team != Team::None && soldier.team == me.team;
        let hidden = world.config.realistic_mode && soldier.visible == 0 && !same_team;
        if bubble.delay <= 0
            || bubble.text.chars().count() >= MORE_CHAT_TEXT
            || (hidden && bubble.team)
        {
            continue;
        }

        let size = hud.fonts.measure(FontStyle::Small, &bubble.text);
        // bottom aligned, 25 over the head
        let p = to_screen(soldier.skeleton.pos(12)) + vec2(-size.x / 2.0, -25.0 - size.y);
        let alpha = (9 * bubble.delay).clamp(0, 255) as u8;
        hud.text(
            FontStyle::Small,
            &bubble.text,
            p,
            rgb_alpha(colors::ABOVE_CHAT, alpha),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_lines_scroll_away() {
        let mut console = TextConsole::new(150, 150);
        for i in 0..7 {
            console.add(i.to_string(), 0, 6);
        }
        let texts: Vec<_> = console.lines().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["2", "3", "4", "5", "6"],
            "6 is the limit, so 5 stay"
        );

        // scrolling restarts the countdown without the new line's wait
        for _ in 0..149 {
            console.tick();
        }
        assert_eq!(console.lines().count(), 5);
        console.tick();
        assert_eq!(console.lines().count(), 4);

        // otherwise a new line waits before the countdown starts
        console.add("7".into(), 0, 6);
        for _ in 0..299 {
            console.tick();
        }
        assert_eq!(console.lines().count(), 5);
        console.tick();
        assert_eq!(console.lines().count(), 4);
    }
}
