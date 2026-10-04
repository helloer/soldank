//! Votes on a server: the vote box, its countdown and F12/F11 (`StartVote`, `TimerVote`,
//! `RenderVoteMenuTexts`), and starting map and kick votes from the menus.

use super::*;
use soldank_core::net::{ClientMessage, PlayerNum, VoteKind};

/// How long a vote runs (`DEFAULT_VOTING_TIME`).
const VOTING_TICKS: u32 = 20 * 60;

/// The running vote.
pub struct VoteBox {
    pub kind: VoteKind,
    pub starter: String,
    pub reason: String,
    ticks_left: u32,
    /// Not voted on or put away yet (`VoteActive`).
    pub shown: bool,
}

impl Game {
    /// A vote started. The player's own kick vote counts as a yes right away.
    pub(crate) fn vote_on(&mut self, kind: VoteKind, starter: Option<SoldierId>, reason: String) {
        self.hud.menus.stats = false;
        let mine = starter.is_some() && starter == self.player;
        let shown = !(mine && matches!(kind, VoteKind::Kick(_)));
        if !shown {
            let name = self.vote_target(&kind);
            self.message(
                format!("You have voted to kick {name} from the game"),
                console_colors::VOTE,
            );
            self.send_vote(kind.clone(), String::new());
        }
        let starter = starter
            .and_then(|id| self.world.soldiers.get(id))
            .map_or("Server".to_string(), |s| s.name.clone());
        self.hud.vote = Some(VoteBox {
            kind,
            starter,
            reason,
            ticks_left: VOTING_TICKS,
            shown,
        });
    }

    /// `TimerVote`: the vote runs out (voted on or put away, it still counts down).
    pub(crate) fn vote_timer(&mut self) {
        let Some(vote) = &mut self.hud.vote else {
            return;
        };
        vote.ticks_left = vote.ticks_left.saturating_sub(1);
        if vote.ticks_left == 0 {
            if matches!(vote.kind, VoteKind::Map(_)) {
                self.message("No map has been voted", console_colors::VOTE);
            }
            self.hud.vote = None;
        }
    }

    /// F12 (yes) or F11 (no) while the vote shows; whether the key was for it.
    pub(crate) fn vote_key(&mut self, yes: bool) -> bool {
        let Some(vote) = self.hud.vote.as_mut().filter(|v| v.shown) else {
            return false;
        };
        vote.shown = false;
        if yes {
            let kind = vote.kind.clone();
            let text = match &kind {
                VoteKind::Map(map) => format!("You have voted on {map}"),
                VoteKind::Kick(_) => format!("You have voted to kick {}", self.vote_target(&kind)),
            };
            self.send_vote(kind, String::new());
            self.message(text, console_colors::VOTE);
        }
        true
    }

    pub(crate) fn send_vote(&mut self, kind: VoteKind, reason: String) {
        if let Some(connection) = &mut self.connection {
            connection.send(&ClientMessage::Vote { kind, reason });
        }
    }

    /// The kick menu's Kick on a server: the reason is typed first (`VoteKickReasonType`).
    pub(crate) fn start_kick_vote(&mut self, id: SoldierId) {
        let Some(connection) = &self.connection else {
            return;
        };
        let num = connection
            .net
            .players
            .iter()
            .find(|(_, s)| **s == id)
            .map(|(num, _)| *num);
        if let Some(num) = num {
            self.hud.kick_vote = Some(num);
            self.hud.menus.show_esc(false);
            self.chat.start(ChatKind::VoteReason);
        }
    }

    /// The typed reason: four letters at least, else nothing happens.
    pub(crate) fn kick_vote_reason(&mut self, reason: &str) {
        if let Some(num) = self.hud.kick_vote.take()
            && reason.chars().count() >= 3
        {
            // the reason keeps the chat line's leading space, like Soldat's
            self.send_vote(VoteKind::Kick(num), format!(" {reason}"));
        }
    }

    /// The vote box (`RenderVoteMenuTexts`).
    pub(crate) fn vote_view(&self) -> Option<render::interface::VoteView> {
        let vote = self.hud.vote.as_ref().filter(|v| v.shown)?;
        Some(render::interface::VoteView {
            kick: matches!(vote.kind, VoteKind::Kick(_)),
            target: self.vote_target(&vote.kind),
            starter: vote.starter.clone(),
            reason: vote.reason.clone(),
        })
    }

    /// The map, or the name of the player a kick vote is against.
    fn vote_target(&self, kind: &VoteKind) -> String {
        match kind {
            VoteKind::Map(map) => map.clone(),
            VoteKind::Kick(num) => self.player_name(*num).unwrap_or_else(|| num.to_string()),
        }
    }

    fn player_name(&self, num: PlayerNum) -> Option<String> {
        let id = self.connection.as_ref()?.net.soldier(num)?;
        Some(self.world.soldiers.get(id)?.name.clone())
    }
}
