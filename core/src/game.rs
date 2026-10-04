//! The match (`Game.pas`, `ServerLoop.pas`): game mode, scores, limits, wave respawns and
//! the map change countdown.

use super::*;
use slotmap::SlotMap;

const SECOND: i32 = 60;
/// `DEFAULT_MAPCHANGE_TIME`: ticks between the end of a match and the next map.
pub const MAP_CHANGE_TIME: i32 = SECOND * 5 + 20;
/// `MapChangeCounter` while no map change is pending.
pub const NO_MAP_CHANGE: i32 = -60;
/// `MapChangeCounter` of a paused game (`CommandPause`).
pub const PAUSED: i32 = 999_999_999;
/// `MULTIKILLINTERVAL`
pub(crate) const MULTIKILL_INTERVAL: i32 = 180;

/// Game modes (`GAMESTYLE_*`), numbered like Soldat.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum GameMode {
    #[default]
    Deathmatch = 0,
    Pointmatch = 1,
    Teammatch = 2,
    CaptureTheFlag = 3,
    Rambo = 4,
    Infiltration = 5,
    HoldTheFlag = 6,
}

impl GameMode {
    pub fn from_num(n: i64) -> GameMode {
        use GameMode::*;
        match n {
            1 => Pointmatch,
            2 => Teammatch,
            3 => CaptureTheFlag,
            4 => Rambo,
            5 => Infiltration,
            6 => HoldTheFlag,
            _ => Deathmatch,
        }
    }

    /// `IsTeamGame`
    pub fn is_team_game(self) -> bool {
        use GameMode::*;
        matches!(
            self,
            Teammatch | CaptureTheFlag | Infiltration | HoldTheFlag
        )
    }

    /// The cvar holding this mode's point limit (`sv_killlimit` is set from it).
    pub fn limit_cvar(self) -> &'static str {
        use GameMode::*;
        match self {
            Deathmatch => "sv_dm_limit",
            Pointmatch => "sv_pm_limit",
            Teammatch => "sv_tm_limit",
            CaptureTheFlag => "sv_ctf_limit",
            Rambo => "sv_rm_limit",
            Infiltration => "sv_inf_limit",
            HoldTheFlag => "sv_htf_limit",
        }
    }
}

/// Match state that changes as the game goes on.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// `TeamScore`, indexed by `Team as usize` (0 unused).
    pub team_scores: [i32; 6],
    /// `TimeLimitCounter`: ticks left.
    pub time_left: i32,
    /// `MapChangeCounter`: counting down to the next map once a match ended; the
    /// simulation stands still meanwhile.
    pub map_change_counter: i32,
    pub wave_respawn_time: i32,
    pub wave_respawn_counter: i32,
    /// `SurvivalEndRound`: a survival round is over, everybody respawns soon.
    pub survival_end_round: bool,
    /// `WeaponsCleaned`: the dropped weapons went at the round's end
    /// (`sv_survivalmode_clearweapons`); a drop makes it clear again.
    pub weapons_cleaned: bool,
    /// `SortedPlayers[1]` as of the last `SortPlayers`.
    pub leader: Option<SoldierId>,
    /// `SinusCounter`: the phase of the spawn protection's blinking.
    pub sinus_counter: f32,
}

impl Match {
    pub fn new(config: &WorldConfig, players: usize) -> Match {
        let mut m = Match {
            team_scores: [0; 6],
            time_left: config.time_limit,
            map_change_counter: NO_MAP_CHANGE,
            wave_respawn_time: 0,
            wave_respawn_counter: 0,
            survival_end_round: false,
            weapons_cleaned: false,
            leader: None,
            sinus_counter: 0.0,
        };
        m.update_wave_respawn_time(config, players);
        m.wave_respawn_counter = m.wave_respawn_time;
        m
    }

    /// Whether the match ended and the map is about to change.
    pub fn ended(&self) -> bool {
        self.map_change_counter >= 0
    }

    /// An admin's `pause` (the game stands still), and `unpause`.
    pub fn pause(&mut self) {
        self.map_change_counter = PAUSED;
    }

    pub fn unpause(&mut self) {
        if self.paused() {
            self.map_change_counter = NO_MAP_CHANGE;
        }
    }

    pub fn paused(&self) -> bool {
        self.map_change_counter == PAUSED
    }

    /// `NextMap`/`PrepareMapChange`: the match is over.
    pub fn end(&mut self, events: &mut Vec<GameEvent>) {
        self.time_left = 0;
        self.map_change_counter = MAP_CHANGE_TIME;
        events.push(GameEvent::MatchEnded);
    }

    /// `UpdateWaveRespawnTime`: longer waves with more players.
    pub fn update_wave_respawn_time(&mut self, config: &WorldConfig, players: usize) {
        let mut time = players as i32 * SECOND;
        if time > config.respawn_min_wave {
            time = config.respawn_max_wave;
        }
        time -= config.respawn_min_wave;
        self.wave_respawn_time = time.max(1);
    }

    /// The kill limit part of `SortPlayers`.
    pub fn check_limits(
        &mut self,
        config: &WorldConfig,
        soldiers: &SlotMap<SoldierId, Soldier>,
        events: &mut Vec<GameEvent>,
    ) {
        if self.map_change_counter < 1
            && !config.game_mode.is_team_game()
            && soldiers
                .values()
                .any(|s| s.active && s.kills >= config.kill_limit)
        {
            self.end(events);
        }

        if self.map_change_counter < 1
            && self.team_scores[1..]
                .iter()
                .any(|&score| score >= config.kill_limit)
        {
            self.end(events);
        }
    }

    /// The per-tick counters at the end of `UpdateFrame`.
    pub fn tick(&mut self, events: &mut Vec<GameEvent>) {
        if self.map_change_counter > NO_MAP_CHANGE && self.map_change_counter < 99_999_999 {
            self.map_change_counter -= 1;
        }
        if self.map_change_counter < 0 && self.map_change_counter > NO_MAP_CHANGE + 1 {
            // ChangeMap
            events.push(GameEvent::ChangeMap);
            self.map_change_counter = NO_MAP_CHANGE;
        }

        self.wave_respawn_counter -= 1;
        if self.wave_respawn_counter < 1 {
            self.wave_respawn_counter = self.wave_respawn_time;
        }

        if self.map_change_counter < 99_999_999 && self.time_left > 0 {
            self.time_left -= 1;
        }
        if self.time_left == 1 {
            self.end(events);
        }
    }
}

/// Who killed whom, for scoring.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ScoredKill {
    pub victim: SoldierId,
    pub killer: SoldierId,
    /// What the victim held when it died.
    pub victim_weapon: WeaponKind,
    /// Weapon of the killing bullet, if any.
    pub bullet_weapon: Option<WeaponKind>,
    pub killer_holds_pointmatch_flag: bool,
}

/// Kill scoring of `TSprite.Die` for each game mode.
pub(crate) fn score_kill(
    soldiers: &mut SlotMap<SoldierId, Soldier>,
    team_scores: &mut [i32; 6],
    mode: GameMode,
    kill: ScoredKill,
) {
    let ScoredKill {
        victim,
        killer,
        victim_weapon,
        bullet_weapon,
        killer_holds_pointmatch_flag,
    } = kill;
    use GameMode::*;

    if victim == killer {
        return;
    }
    let victim_team = soldiers.get(victim).map(|s| s.team);
    let anyone_rambo = soldiers.values().any(|s| {
        s.primary_weapon()
            .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
    });
    let Some(k) = soldiers.get_mut(killer) else {
        return;
    };
    let enemy = victim_team != Some(k.team);
    let bow = |w: Option<WeaponKind>| matches!(w, Some(WeaponKind::Bow | WeaponKind::FlameBow));

    let multikill = |k: &mut Soldier| {
        k.multi_kill_time = MULTIKILL_INTERVAL;
        k.multi_kills += 1;
    };

    match mode {
        Deathmatch => {
            k.kills += 1;
            multikill(k);
        }
        Pointmatch => {
            let mut points = 1;
            if killer_holds_pointmatch_flag {
                points *= 2;
            }
            if k.multi_kill_time > 0 {
                points *= match k.multi_kills {
                    2 => 2,
                    3 => 4,
                    4 => 8,
                    5 => 16,
                    n if n > 5 => 32,
                    _ => 1,
                };
            }
            k.kills += points;
            multikill(k);
        }
        Teammatch => {
            if enemy {
                k.kills += 1;
                team_scores[k.team as usize] += 1;
                multikill(k);
            }
        }
        CaptureTheFlag | Infiltration | HoldTheFlag => {
            if enemy {
                k.kills += 1;
                multikill(k);
            }
        }
        Rambo => {
            if bow(bullet_weapon) || bow(Some(victim_weapon)) {
                k.kills += 1;
                multikill(k);
            } else if anyone_rambo && k.kills > 0 {
                // punished for killing non-Rambos while someone is Rambo
                k.kills -= 1;
            }
        }
    }
}

/// `SortPlayers`: active players by captures, then kills, then fewer deaths; spectators
/// last. Soldat's swap sort isn't stable, so it's kept as it is.
pub fn sort_players(soldiers: &SlotMap<SoldierId, Soldier>) -> Vec<SoldierId> {
    let mut sorted: Vec<(SoldierId, i32, i32, i32)> = soldiers
        .iter()
        .filter(|(_, s)| s.active)
        .map(|(id, s)| ranking(id, s))
        .collect();
    sort_ranked(&mut sorted);
    sorted.into_iter().map(|(id, ..)| id).collect()
}

/// (player, flags, kills, deaths)
type Ranked<T> = (T, i32, i32, i32);

/// What a player is sorted by: spectators have no flags or kills and endless deaths.
pub fn ranking<T>(id: T, soldier: &Soldier) -> Ranked<T> {
    if soldier.is_spectator() {
        (id, 0, 0, i32::MAX)
    } else {
        (id, soldier.flags, soldier.kills, soldier.deaths)
    }
}

/// The swaps of `SortPlayers` on (player, flags, kills, deaths).
pub fn sort_ranked<T: Copy>(sorted: &mut [Ranked<T>]) {
    let n = sorted.len();
    // by flags, then kills among equal flags, then deaths among equal both
    let swaps = |pass: u8, a: &Ranked<T>, b: &Ranked<T>| match pass {
        0 => b.1 > a.1,
        1 => b.1 == a.1 && b.2 > a.2,
        _ => b.1 == a.1 && b.2 == a.2 && b.3 < a.3,
    };
    for pass in 0..3 {
        for i in 0..n {
            for j in i + 1..n {
                if swaps(pass, &sorted[i], &sorted[j]) {
                    sorted.swap(i, j);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_sort_by_flags_kills_then_deaths() {
        let mut ranked = [(0, 0, 3, 1), (1, 1, 0, 5), (2, 0, 3, 0), (3, 0, 5, 9)];
        sort_ranked(&mut ranked);
        let order: Vec<_> = ranked.iter().map(|r| r.0).collect();
        assert_eq!(order, [1, 3, 2, 0]);
    }
}
