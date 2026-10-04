//! What the client makes of game events beyond the simulation's own sounds: the sounds
//! and messages of things taken and flags captured (`ClientHandleThingTaken`,
//! `ClientHandleFlagInfo`), and the time left signals (`UpdateFrame`).

use crate::audio::{Audio, Listener};
use crate::render::hud::HudMessages;
use soldank_core::*;

/// `CAPTUREMESSAGEWAIT`, `CAPTURECTFMESSAGEWAIT`
const CAPTURE_MESSAGE_WAIT: i32 = 60 * 6;
const CAPTURE_CTF_MESSAGE_WAIT: i32 = 60 * 7;

mod colors {
    pub const CAPTURE: u32 = 0xFF77D334;
    pub const BONUS: u32 = 0xFFEF3121;
    pub const ALPHA: u32 = 0xFFDF3131;
    pub const BRAVO: u32 = 0xFF3131DF;
    pub const GAME: u32 = 0xEE71F981;
}

/// The client's side of a game event.
pub struct EventCtx<'a> {
    pub world: &'a World,
    pub audio: &'a mut Audio,
    pub messages: &'a mut HudMessages,
    pub listener: &'a Listener,
    /// `ui_console_length`
    pub console_length: usize,
    /// Sparks the events make.
    pub sparks: &'a mut Vec<SparkSpawn>,
}

impl EventCtx<'_> {
    fn big(&mut self, text: impl Into<String>, color: u32, wait: i32) {
        self.messages.big_message(text, color, wait);
    }

    fn console(&mut self, text: impl Into<String>, color: u32) {
        let text = text.into();
        tracing::info!(target: "console", "{text}");
        self.messages.console(text, color, self.console_length);
    }

    /// Survival mode: how many are left after a death, while the player lives (`TSprite.Die`).
    pub fn survivors(&mut self, me: Option<SoldierId>, victim: SoldierId) {
        let world = self.world;
        if !world.config.survival_mode {
            return;
        }
        let Some(me) = me
            .and_then(|id| world.soldiers.get(id))
            .filter(|s| !s.dead_meat)
        else {
            return;
        };
        let alive = |s: &&Soldier| s.active && !s.dead_meat && !s.is_spectator();
        match world.config.game_mode {
            GameMode::Deathmatch | GameMode::Rambo => {
                let left = world.soldiers.values().filter(alive).count();
                self.console(format!("Players left: {left}"), colors::GAME);
            }
            GameMode::CaptureTheFlag
            | GameMode::Infiltration
            | GameMode::HoldTheFlag
            | GameMode::Teammatch => {
                // a teammate died
                if world
                    .soldiers
                    .get(victim)
                    .is_some_and(|v| v.team == me.team)
                {
                    let team = me.team;
                    let left = world
                        .soldiers
                        .values()
                        .filter(alive)
                        .filter(|s| s.team == team);
                    let text = format!("Players left on your team: {}", left.count());
                    self.console(text, colors::GAME);
                }
            }
            GameMode::Pointmatch => {}
        }
    }

    fn sound_at(&mut self, sfx: Sfx, pos: Vec2) {
        let sound = SoundEvent::Play(Sound::new(sfx).at(pos));
        self.audio.event(None, &sound, self.listener);
    }

    fn sound_here(&mut self, sfx: Sfx) {
        self.audio.play_here(sfx, self.listener.pos);
    }

    /// `ClientHandleThingTaken`
    pub fn thing_taken(&mut self, kind: ThingKind, who: SoldierId, pos: Vec2) {
        use ThingKind::*;
        let Some(soldier) = self.world.soldiers.get(who) else {
            return;
        };
        let me = self.listener.player == Some(who);
        let name = soldier.name.clone();
        let team = soldier.team;

        match kind {
            AlphaFlag | BravoFlag | PointmatchFlag => {
                self.sound_at(Sfx::Capture, pos);
                self.flag_taken(kind, &name, team, me);
            }
            RamboBow => {
                self.sound_at(Sfx::Takebow, pos);
                let text = if me {
                    "You got the Bow!".to_string()
                } else {
                    format!("{name} got the Bow!")
                };
                self.big(text, colors::CAPTURE, CAPTURE_MESSAGE_WAIT);
            }
            MedicalKit => self.sound_at(Sfx::Takemedikit, pos),
            GrenadeKit => self.sound_at(Sfx::Pickupgun, pos),
            FlamerKit | PredatorKit | VestKit | BerserkKit | ClusterKit => {
                let (sfx, text, color) = match kind {
                    FlamerKit => (Sfx::Godflame, "Flame God Mode!", colors::BONUS),
                    PredatorKit => (Sfx::Predator, "Predator Mode!", colors::BONUS),
                    VestKit => (Sfx::Vesttake, "Bulletproof Vest!", colors::CAPTURE),
                    BerserkKit => (Sfx::Berserker, "Berserker Mode!", colors::BONUS),
                    _ => (Sfx::Pickupgun, "Cluster Grenades!", colors::CAPTURE),
                };
                self.sound_at(sfx, pos);
                if me {
                    self.big(text, color, CAPTURE_MESSAGE_WAIT);
                }
            }
            StationaryGun => self.sound_at(Sfx::M2use, soldier.particle.pos),
            kind if kind.is_gun() => self.sound_at(Sfx::Pickupgun, pos),
            _ => {}
        }
    }

    fn flag_taken(&mut self, kind: ThingKind, name: &str, team: Team, me: bool) {
        let mode = self.world.config.game_mode;
        let color = match (mode, team) {
            (GameMode::HoldTheFlag | GameMode::CaptureTheFlag, Team::Alpha) => colors::ALPHA,
            (GameMode::HoldTheFlag | GameMode::CaptureTheFlag, Team::Bravo) => colors::BRAVO,
            _ => colors::CAPTURE,
        };
        let own = matches!(
            (kind, team),
            (ThingKind::AlphaFlag, Team::Alpha) | (ThingKind::BravoFlag, Team::Bravo)
        );
        let pick = |mine: &str, theirs: &str| if me { mine } else { theirs }.to_string();

        let texts = match mode {
            GameMode::Pointmatch | GameMode::HoldTheFlag => Some((
                pick("You got the Flag!", "Yellow Flag captured!"),
                format!("{name} got the Yellow Flag"),
            )),
            GameMode::CaptureTheFlag => match (own, team) {
                (true, Team::Alpha) => Some((
                    "Red Flag returned!".to_string(),
                    format!("{name} returned the Red Flag"),
                )),
                (true, Team::Bravo) => Some((
                    "Blue Flag returned!".to_string(),
                    format!("{name} returned the Blue Flag"),
                )),
                (false, Team::Alpha) => Some((
                    pick("You got the Blue Flag!", "Blue Flag captured!"),
                    format!("{name} captured the Blue Flag"),
                )),
                (false, Team::Bravo) => Some((
                    pick("You got the Red Flag!", "Red Flag captured!"),
                    format!("{name} captured the Red Flag"),
                )),
                _ => None,
            },
            GameMode::Infiltration => match (own, team) {
                (true, Team::Bravo) => Some((
                    pick("You returned the Objective!", "Objective returned!"),
                    format!("{name} returned the Objective"),
                )),
                (false, Team::Alpha) => Some((
                    pick("You got the Objective!", "Objective captured!"),
                    format!("{name} captured the Objective"),
                )),
                _ => None,
            },
            _ => None,
        };
        if let Some((big, small)) = texts {
            self.big(big, color, CAPTURE_MESSAGE_WAIT);
            self.console(small, color);
        }
    }

    /// `ClientHandleFlagInfo` (captures)
    pub fn flag_captured(&mut self, team: Team, who: SoldierId) {
        let name = self
            .world
            .soldiers
            .get(who)
            .map_or_else(String::new, |s| s.name.clone());
        let infiltration = self.world.config.game_mode == GameMode::Infiltration;
        // the flag that was brought home lights up, and the infiltration one burns
        let home = |kind| {
            self.world
                .things
                .iter()
                .find(|t| t.active && t.kind == kind)
                .map(|t| t.skeleton.pos(2))
        };
        let (flag, life) = match team {
            Team::Alpha => (home(ThingKind::AlphaFlag), 18),
            _ => (home(ThingKind::BravoFlag), 15),
        };
        if let Some(pos) = flag {
            let owner = SparkOwner::Soldier(who);
            if infiltration && team == Team::Alpha {
                for _ in 0..10 {
                    let a =
                        pos + vec2(-10.0 + fx::random(20) as f32, -10.0 + fx::random(20) as f32);
                    self.sparks
                        .push(SparkSpawn::new(a, Vec2::ZERO, 36, SparkOwner::None, 35));
                    if fx::random(2) == 0 {
                        self.sparks
                            .push(SparkSpawn::new(a, Vec2::ZERO, 37, SparkOwner::None, 75));
                    }
                }
            }
            self.sparks
                .push(SparkSpawn::new(pos, Vec2::ZERO, 61, owner, life));
        }
        match team {
            Team::Alpha => {
                self.big(
                    "Alpha Team Scores!",
                    colors::ALPHA,
                    CAPTURE_CTF_MESSAGE_WAIT,
                );
                self.console(format!("{name} scores for Alpha Team"), colors::ALPHA);
                self.sound_here(if infiltration {
                    Sfx::Infiltmus
                } else {
                    Sfx::Ctf
                });
            }
            Team::Bravo => {
                self.big(
                    "Bravo Team Scores!",
                    colors::BRAVO,
                    CAPTURE_CTF_MESSAGE_WAIT,
                );
                self.console(format!("{name} scores for Bravo Team"), colors::BRAVO);
                self.sound_here(Sfx::Ctf);
            }
            _ => {}
        }
    }

    /// The time left, announced now and then (`UpdateFrame`).
    pub fn time_left(&mut self) {
        let game = &self.world.game;
        let t = game.time_left;
        if t <= 0 {
            return;
        }
        let text = if t < 601 {
            (t % 60 == 0 && game.map_change_counter == NO_MAP_CHANGE)
                .then(|| format!("Time Left: {} seconds", t / 60))
        } else if t < 3601 {
            (t % 600 == 0).then(|| format!("Time Left: {} seconds", t / 60))
        } else if t < 18001 {
            (t % 3600 == 0).then(|| format!("Time Left: {} minutes", t / 3600))
        } else {
            (t % 18000 == 0).then(|| format!("Time Left: {} minutes", t / 3600))
        };
        if let Some(text) = text {
            self.console(text, colors::GAME);
            self.sound_here(Sfx::Signal);
        }
    }

    /// A point every fifth score in Infiltration (blue) and Hold the Flag (both).
    pub fn team_points(&mut self, before: &[i32; 6]) {
        let scores = self.world.game.team_scores;
        let teams: &[usize] = match self.world.config.game_mode {
            GameMode::Infiltration => &[2],
            GameMode::HoldTheFlag => &[1, 2],
            _ => &[],
        };
        for &team in teams {
            if scores[team] > before[team] && scores[team] % 5 == 0 {
                self.sound_here(Sfx::InfiltPoint);
            }
        }
    }
}
