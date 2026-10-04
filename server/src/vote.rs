//! Map and kick votes (`StartVote`, `CountVote`, `TimerVote`, `ServerHandleVoteKick`,
//! `CommandVotemap`).

use super::*;
use std::collections::HashSet;

/// How long a vote runs (`DEFAULT_VOTING_TIME`).
pub(crate) const VOTING_TICKS: u64 = 20 * 60;
/// No starting a vote this long after joining or starting one (`DEFAULT_VOTE_TIME`).
pub(crate) const VOTE_COOLDOWN_TICKS: u64 = 2 * 60 * 60;

/// The running vote.
pub(crate) struct Vote {
    pub kind: VoteKind,
    /// It runs out at this tick.
    ends: u64,
    /// The humans when it started (`VoteMaxVotes`).
    max_votes: usize,
    voted: HashSet<PlayerNum>,
}

impl ServerGame {
    /// A player starts a vote, or says yes to the running one.
    pub(crate) fn vote(
        &mut self,
        renet: &mut RenetServer,
        client_id: ClientId,
        kind: VoteKind,
        reason: String,
    ) {
        let Some(client) = self.clients.get(&client_id) else {
            return;
        };
        let Some(num) = client.num else { return };
        let cooling = client.vote_cooldown > self.uptime;

        if let Some(vote) = &self.vote {
            if vote.kind != kind {
                return;
            }
            if vote.kind == VoteKind::Kick(num) {
                let text = "A vote has been cast against you. You can not vote.";
                Self::send(
                    renet,
                    client_id,
                    channel::RELIABLE,
                    &ServerMessage::ServerText(text.into()),
                );
                return;
            }
            self.count_vote(renet, num);
            return;
        }

        let (kind, reason) = match kind {
            VoteKind::Map(map) => {
                let Some(map) = self
                    .maps_list()
                    .into_iter()
                    .find(|m| m.eq_ignore_ascii_case(&map))
                else {
                    let text = format!("Map not found ({map})");
                    Self::send(
                        renet,
                        client_id,
                        channel::RELIABLE,
                        &ServerMessage::ServerText(text),
                    );
                    return;
                };
                if cooling {
                    let text = "Can't vote for 2:00 minutes after joining game or last vote";
                    Self::send(
                        renet,
                        client_id,
                        channel::RELIABLE,
                        &ServerMessage::ServerText(text.into()),
                    );
                    return;
                }
                (VoteKind::Map(map), "---".to_string())
            }
            VoteKind::Kick(target) => {
                if cooling || !self.players.contains_key(&target) {
                    return;
                }
                if self.is_muted(client_id) {
                    let text = "You are muted. You can't cast a vote kick.";
                    let muted = ServerMessage::ServerText(text.into());
                    Self::send(renet, client_id, channel::RELIABLE, &muted);
                    return;
                }
                let reason: String = reason.chars().take(26).collect();
                tracing::info!(
                    "{} started votekick against {} - Reason:{reason}",
                    self.name_of(num),
                    self.name_of(target)
                );
                (VoteKind::Kick(target), reason)
            }
        };

        let now = self.uptime;
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.vote_cooldown = now + VOTE_COOLDOWN_TICKS;
        }
        self.start_vote(renet, kind, num, reason);
    }

    /// `StartVote` (over a running one), by player `starter` or 0, the server.
    pub(crate) fn start_vote(
        &mut self,
        renet: &mut RenetServer,
        kind: VoteKind,
        starter: PlayerNum,
        reason: String,
    ) {
        let max_votes = self
            .clients
            .values()
            .filter(|c| c.ready && c.num.is_some_and(|n| self.players.contains_key(&n)))
            .count();
        self.vote = Some(Vote {
            kind: kind.clone(),
            ends: self.uptime + VOTING_TICKS,
            max_votes,
            voted: HashSet::new(),
        });
        let on = ServerMessage::VoteOn {
            kind,
            starter,
            reason,
        };
        self.broadcast(renet, channel::RELIABLE, &on);
    }

    /// A yes; with enough of them (`sv_votepercent` of the humans) the vote passes.
    fn count_vote(&mut self, renet: &mut RenetServer, voter: PlayerNum) {
        let percent = self.cvars.int("sv_votepercent") as f32;
        let Some(vote) = &mut self.vote else { return };
        if !vote.voted.insert(voter) {
            return;
        }
        if (vote.voted.len() as f32 / vote.max_votes.max(1) as f32) < percent / 100.0 {
            return;
        }
        let kind = vote.kind.clone();
        self.vote = None;
        self.broadcast(renet, channel::RELIABLE, &ServerMessage::VoteOff);
        match kind {
            // no permanent bans by votes: an hour, a day for the server's suspects
            VoteKind::Kick(num) => {
                let ban = if self.cheat_tags.contains(&num) {
                    ("Vote Kicked by Server".to_string(), admin::DAY)
                } else {
                    ("Vote Kicked".to_string(), admin::HOUR)
                };
                self.kick_player(renet, num, LeaveReason::VoteKicked, Some(ban));
            }
            VoteKind::Map(map) => self.prepare_map_change(renet, &map),
        }
    }

    /// The vote runs out (the clients count down too).
    pub(crate) fn vote_timer(&mut self) {
        let Some(vote) = &self.vote else { return };
        if self.uptime >= vote.ends {
            if matches!(vote.kind, VoteKind::Map(_)) {
                tracing::info!("No map has been voted");
            }
            self.vote = None;
        }
    }

    /// The vote against a player who left is over.
    pub(crate) fn vote_target_left(&mut self, renet: &mut RenetServer, num: PlayerNum) {
        if self
            .vote
            .as_ref()
            .is_some_and(|v| v.kind == VoteKind::Kick(num))
        {
            self.vote = None;
            self.broadcast(renet, channel::RELIABLE, &ServerMessage::VoteOff);
        }
    }

    /// The match ends now and the next map is `map` (`PrepareMapChange`).
    pub(crate) fn prepare_map_change(&mut self, renet: &mut RenetServer, map: &str) {
        tracing::info!("Next map: {map}");
        self.next_map = Some(map.to_string());
        let mut events = Vec::new();
        self.world.game.end(&mut events);
        self.send_events(renet, &events);
    }

    pub(crate) fn name_of(&self, num: PlayerNum) -> String {
        self.players
            .get(&num)
            .and_then(|&id| self.world.soldiers.get(id))
            .map_or(String::new(), |s| s.name.clone())
    }
}
