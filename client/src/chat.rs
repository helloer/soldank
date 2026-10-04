//! Chat and command input (`ChatText` with `StartChat`, `ChatKeyDown`, the text input
//! of `GameInput` and `TabComplete`). The text starts with a marker the player can't
//! delete: a space for chat, `/` for a command.

/// `MAXCHATTEXT`
pub const MAX_CHAT_TEXT: usize = 85;
/// `REASON_CHARS - 1`: a kick vote's reason.
pub const MAX_REASON_TEXT: usize = 25;
/// `MAX_PLAYERS`: the tab completion cycles through player numbers 1..=32.
const MAX_PLAYERS: usize = 32;

/// `ChatType`
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ChatKind {
    #[default]
    Public,
    Team,
    Command,
    /// The reason for a kick vote (`VoteKickReasonType`).
    VoteReason,
}

/// Keys the chat input handles itself.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChatKey {
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    Left,
    Right,
    WordLeft,
    WordRight,
    Tab,
    Enter,
}

/// What a finished line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOutcome {
    /// A chat message (without the marker).
    Say(ChatKind, String),
    /// A console command (without the `/`).
    Command(String),
}

/// What Tab completes from: other players by number, and console command names.
pub struct Completions<'a> {
    pub players: &'a [(usize, String)],
    pub commands: &'a [String],
}

#[derive(Debug, Clone)]
enum Completing {
    /// `CurrentTabCompletePlayer`
    Player(usize),
    /// Index into the matching commands.
    Command(usize),
}

#[derive(Debug, Clone)]
struct Completion {
    /// `CompletionBase`: the word being completed, and the text length before it
    /// (`CompletionBaseSeparator`).
    base: String,
    separator: usize,
    current: Completing,
}

#[derive(Debug, Default)]
pub struct Chat {
    text: Vec<char>,
    /// `CursorPosition`: characters before the cursor.
    cursor: usize,
    kind: ChatKind,
    /// `LastChatText` and its type: `/` on an empty command line brings it back.
    last: Option<(ChatKind, Vec<char>)>,
    /// `FireChatText`: a line put away by clicking, until the same chat opens again.
    fire: Option<(ChatKind, Vec<char>)>,
    completion: Option<Completion>,
    /// When the text or the cursor last changed (the cursor blinks from then).
    pub changed_at: f64,
}

impl Chat {
    pub fn active(&self) -> bool {
        !self.text.is_empty()
    }

    pub fn kind(&self) -> ChatKind {
        self.kind
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// `StartChat`
    pub fn start(&mut self, kind: ChatKind) {
        self.kind = kind;
        self.text = match self.fire.take() {
            Some((fired, text)) if fired == kind => text,
            _ if kind == ChatKind::Command => vec!['/'],
            _ => vec![' '],
        };
        self.cursor = self.text.len();
        self.completion = None;
    }

    /// `ClearChatText`
    pub fn clear(&mut self) {
        self.text.clear();
        self.fire = None;
        self.completion = None;
        self.cursor = 1;
    }

    /// A click while typing puts the line away (`FireChatText`).
    pub fn stash(&mut self) {
        if self.active() {
            self.fire = Some((self.kind, std::mem::take(&mut self.text)));
            self.completion = None;
        }
    }

    /// Typed text (`SDL_TEXTINPUT`).
    pub fn input(&mut self, text: &str) {
        if !self.active() {
            return;
        }
        let chars = filter(text);
        if self.text == ['/']
            && chars == ['/']
            && let Some((kind, last)) = self.last.clone().filter(|(_, l)| l.len() > 1)
        {
            self.kind = kind;
            self.text = last;
            self.cursor = self.text.len();
        } else if self.text.len() < self.max_len() {
            self.insert(&chars);
        }
        self.completion = None;
    }

    /// Pasted text: inserted, then cut to the maximum length.
    pub fn paste(&mut self, text: &str) {
        if !self.active() {
            return;
        }
        self.insert(&filter(text));
        let max = self.max_len();
        if self.text.len() > max {
            self.text.truncate(max);
            self.cursor = self.cursor.min(max);
        }
        self.completion = None;
    }

    fn max_len(&self) -> usize {
        if self.kind == ChatKind::VoteReason {
            MAX_REASON_TEXT
        } else {
            MAX_CHAT_TEXT
        }
    }

    fn insert(&mut self, chars: &[char]) {
        let at = self.cursor.min(self.text.len());
        self.text.splice(at..at, chars.iter().copied());
        self.cursor = at + chars.len();
    }

    /// `ChatKeyDown`: returns a finished line on Enter.
    pub fn key(&mut self, key: ChatKey, completions: &Completions) -> Option<ChatOutcome> {
        if !self.active() {
            return None;
        }
        let len = self.text.len();
        match key {
            ChatKey::Escape => self.clear(),
            ChatKey::Backspace => {
                if self.cursor > 1 || len == 1 {
                    self.completion = None;
                    self.text.remove(self.cursor.max(1) - 1);
                    self.cursor = self.cursor.saturating_sub(1);
                    if self.text.is_empty() {
                        self.clear();
                    }
                }
            }
            ChatKey::Delete => {
                if len > self.cursor {
                    self.text.remove(self.cursor);
                    self.completion = None;
                }
            }
            ChatKey::Home => self.cursor = 1,
            ChatKey::End => self.cursor = len,
            ChatKey::Right => {
                if len > self.cursor {
                    self.cursor += 1;
                }
            }
            ChatKey::Left => {
                if self.cursor > 1 {
                    self.cursor -= 1;
                }
            }
            // to the start of the next or the previous word
            ChatKey::WordRight => {
                while self.cursor < len {
                    self.cursor += 1;
                    if self.cursor == len || self.word_starts_here() {
                        break;
                    }
                }
            }
            ChatKey::WordLeft => {
                while self.cursor > 1 {
                    self.cursor -= 1;
                    if self.word_starts_here() {
                        break;
                    }
                }
            }
            ChatKey::Tab => self.tab_complete(completions),
            ChatKey::Enter => {
                let line = self.text();
                let outcome = match line.strip_prefix('/') {
                    Some(command) => ChatOutcome::Command(command.to_string()),
                    None => ChatOutcome::Say(self.kind, line.chars().skip(1).collect()),
                };
                self.last = Some((self.kind, std::mem::take(&mut self.text)));
                self.clear();
                return Some(outcome);
            }
        }
        None
    }

    fn word_starts_here(&self) -> bool {
        self.text[self.cursor - 1] == ' ' && self.text.get(self.cursor) != Some(&' ')
    }

    /// `TabComplete`: the last word becomes the next player whose name contains it. A
    /// command's first word completes to command and cvar names instead.
    fn tab_complete(&mut self, completions: &Completions) {
        // a '^' after the marker isn't part of the word
        let offset = usize::from(self.text.get(1) == Some(&'^'));
        let completion = self.completion.get_or_insert_with(|| {
            let separator = self
                .text
                .iter()
                .rposition(|&c| c == ' ')
                .map_or(0, |i| i + 1)
                .max(offset);
            let is_command = self.text[0] == '/' && separator == 0;
            let skip = usize::from(is_command);
            Completion {
                base: self.text[separator + skip..].iter().collect(),
                separator: separator + skip,
                current: if is_command {
                    Completing::Command(usize::MAX)
                } else {
                    Completing::Player(0)
                },
            }
        });
        if self.text.len() <= offset {
            return;
        }

        let base = completion.base.to_lowercase();
        let found = match completion.current {
            Completing::Player(current) => (0..MAX_PLAYERS)
                .map(|i| (current + i) % MAX_PLAYERS + 1)
                .find_map(|num| {
                    completions
                        .players
                        .iter()
                        .find(|(n, name)| *n == num && name.to_lowercase().contains(&base))
                })
                .map(|(num, name)| (Completing::Player(*num), name.clone())),
            Completing::Command(current) => {
                let matching: Vec<&String> = completions
                    .commands
                    .iter()
                    .filter(|c| c.to_lowercase().starts_with(&base))
                    .collect();
                let next = current.wrapping_add(1) % matching.len().max(1);
                matching
                    .get(next)
                    .map(|name| (Completing::Command(next), name.to_string()))
            }
        };

        if let Some((current, name)) = found {
            let room = MAX_CHAT_TEXT.saturating_sub(completion.separator);
            self.text.truncate(completion.separator);
            self.text.extend(name.chars().take(room));
            self.cursor = self.text.len();
            completion.current = current;
        }
    }
}

/// `FilterChatText`: no control characters.
fn filter(text: &str) -> Vec<char> {
    text.chars()
        .filter(|&c| c >= ' ' && c != '\u{7f}')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Completions = Completions {
        players: &[],
        commands: &[],
    };

    fn typed(kind: ChatKind, text: &str) -> Chat {
        let mut chat = Chat::default();
        chat.start(kind);
        for c in text.chars() {
            chat.input(&c.to_string());
        }
        chat
    }

    #[test]
    fn chat_lines_lose_their_marker() {
        let mut chat = typed(ChatKind::Team, "hi all");
        assert_eq!(chat.text(), " hi all");
        assert_eq!(
            chat.key(ChatKey::Enter, &NONE),
            Some(ChatOutcome::Say(ChatKind::Team, "hi all".into()))
        );
        assert!(!chat.active());

        let mut chat = typed(ChatKind::Command, "addbot Admiral");
        assert_eq!(
            chat.key(ChatKey::Enter, &NONE),
            Some(ChatOutcome::Command("addbot Admiral".into()))
        );
    }

    #[test]
    fn the_marker_stays_unless_it_is_all_there_is() {
        let mut chat = typed(ChatKind::Public, "ab");
        chat.key(ChatKey::Home, &NONE);
        chat.key(ChatKey::Backspace, &NONE);
        assert_eq!(
            chat.text(),
            " ab",
            "nothing before the cursor but the marker"
        );

        chat.key(ChatKey::End, &NONE);
        for _ in 0..2 {
            chat.key(ChatKey::Backspace, &NONE);
        }
        assert_eq!(chat.text(), " ");
        chat.key(ChatKey::Backspace, &NONE);
        assert!(!chat.active(), "deleting the marker closes the chat");
    }

    #[test]
    fn editing_in_the_middle() {
        let mut chat = typed(ChatKind::Public, "one three");
        chat.key(ChatKey::WordLeft, &NONE);
        assert_eq!(chat.cursor(), 5);
        chat.input("two ");
        assert_eq!(chat.text(), " one two three");
        chat.key(ChatKey::Delete, &NONE);
        assert_eq!(chat.text(), " one two hree");
        chat.key(ChatKey::WordRight, &NONE);
        assert_eq!(chat.cursor(), chat.text().chars().count());
    }

    #[test]
    fn slash_brings_back_the_last_command() {
        let mut chat = typed(ChatKind::Command, "kill");
        chat.key(ChatKey::Enter, &NONE);
        let mut again = typed(ChatKind::Command, "");
        again.last = chat.last.clone();
        again.input("/");
        assert_eq!(again.text(), "/kill");
    }

    #[test]
    fn clicking_puts_the_line_away() {
        let mut chat = typed(ChatKind::Public, "brb");
        chat.stash();
        assert!(!chat.active());
        chat.start(ChatKind::Team);
        assert_eq!(chat.text(), " ", "another chat type starts empty");
        chat.clear();

        let mut chat = typed(ChatKind::Public, "brb");
        chat.stash();
        chat.start(ChatKind::Public);
        assert_eq!(chat.text(), " brb");
    }

    #[test]
    fn the_line_has_a_maximum_length() {
        let mut chat = typed(ChatKind::Public, &"x".repeat(100));
        assert_eq!(chat.text().len(), MAX_CHAT_TEXT);
        chat.paste("yyyy");
        assert_eq!(chat.text().len(), MAX_CHAT_TEXT);
    }

    #[test]
    fn tab_cycles_through_matching_players() {
        let players = [
            (1, "Major".to_string()),
            (3, "Admiral".to_string()),
            (4, "Hitman".to_string()),
        ];
        let completions = Completions {
            players: &players,
            commands: &[],
        };
        let mut chat = typed(ChatKind::Public, "hi ma");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), " hi Major");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), " hi Hitman", "contains, not starts with");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), " hi Major");
    }

    #[test]
    fn tab_completes_command_names() {
        let commands = ["addbot".to_string(), "addbot1".to_string(), "kill".into()];
        let completions = Completions {
            players: &[(2, "Admiral".to_string())],
            commands: &commands,
        };
        let mut chat = typed(ChatKind::Command, "add");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), "/addbot");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), "/addbot1");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(chat.text(), "/addbot");

        chat.input(" ad");
        chat.key(ChatKey::Tab, &completions);
        assert_eq!(
            chat.text(),
            "/addbot Admiral",
            "arguments complete player names"
        );
    }
}
