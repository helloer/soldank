//! Recording and playing demos (`Demo.pas`). Of network games: the server's messages each tick
//! and the player's input, played back through the client's own message handling. Single
//! player: from a map's start, the player's input and what the player did to the world
//! ([`LocalAction`], through [`Game::act`]), played again by the world itself. The camera
//! follows the recording player; fire and jets switch to the next and previous player, jump to
//! the free camera.

use super::*;
use soldank_core::demo::*;
use soldank_core::net::{
    NetClient, PlayerInfo, PlayerNum, ServerMessage, Snapshot, SoldierState, ThingState,
};
use std::fs::File;
use std::io::BufWriter;

/// Demo files.
pub const DEMO_EXTENSION: &str = "sdemo";

/// A played frame: what happened, and the recording player's input.
pub type PlayedFrame = (Vec<GameEvent>, Vec<(SoldierId, Input)>);

pub struct Recorder {
    writer: DemoWriter<BufWriter<File>>,
    name: String,
    /// This tick's frame so far.
    frame: DemoFrame,
    /// Started by `demo_autorecord`: it ends with the map.
    pub auto: bool,
    /// Single player: the server cvars as last recorded.
    local: Option<Vec<(String, String)>>,
}

/// How often a single-player demo has a [`world_check`] (a second).
const CHECK_FRAMES: u64 = 60;

/// Playback keeps the game as it was every this many frames (ten seconds), for seeking.
const KEYFRAME_FRAMES: usize = 600;

/// The game at a frame of the demo, to seek from.
#[derive(Clone)]
struct Keyframe {
    next: usize,
    world: World,
    net: NetClient,
    own: Option<SoldierId>,
    check: Option<u64>,
    /// The game's cvars (single-player demos change them).
    cvars: Vec<(String, String)>,
}

impl Recorder {
    /// A message as it came from the server (map downloads aren't kept).
    pub fn message(&mut self, message: &ServerMessage) {
        if !matches!(message, ServerMessage::FileChunk { .. }) {
            self.frame.messages.push(message.clone());
        }
    }
}

pub struct Playback {
    pub demo: Demo,
    pub name: String,
    pub net: NetClient,
    /// The next frame.
    next: usize,
    /// The buttons last tick, for the camera keys.
    buttons: Buttons,
    /// The viewer took the free camera.
    free_cam: bool,
    finished: bool,
    /// The demo's map and match are loaded.
    pub started: bool,
    /// Single player: the recording player, and the world check due after the last tick.
    own: Option<SoldierId>,
    check: Option<u64>,
    /// The playback went another way than the game (said once).
    pub out_of_sync: bool,
    /// The game every [`KEYFRAME_FRAMES`] so far, by frame.
    keyframes: Vec<Keyframe>,
}

impl Playback {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn new(demo: Demo, name: String) -> Playback {
        Playback {
            demo,
            name,
            net: NetClient::default(),
            next: 0,
            buttons: Buttons::default(),
            free_cam: false,
            finished: false,
            started: false,
            own: None,
            check: None,
            out_of_sync: false,
            keyframes: Vec::new(),
        }
    }

    /// A single-player demo.
    pub fn local(&self) -> bool {
        self.demo.header.local
    }
}

/// `demos/<name>.sdemo` in the config directory, or a path.
pub fn demo_path(config_dir: &std::path::Path, name: &str) -> PathBuf {
    let path = PathBuf::from(name);
    if path.exists() || path.components().count() > 1 {
        return path;
    }
    let file = if name.ends_with(&format!(".{DEMO_EXTENSION}")) {
        name.to_string()
    } else {
        format!("{name}.{DEMO_EXTENSION}")
    };
    config_dir.join("demos").join(file)
}

impl Game {
    /// The RecordDemo key: a demo starts, or stops; with `demo_autorecord` the running one
    /// makes way for a new one.
    pub(crate) fn record_demo_key(&mut self) {
        if self.playback.is_some() {
            return;
        }
        if self.recorder.is_none() || self.console.cvars.bool("demo_autorecord") {
            self.start_recording(None, false);
        } else {
            self.stop_recording();
        }
    }

    /// `record`: from now on (the match as it is now starts the demo).
    pub(crate) fn start_recording(&mut self, name: Option<&str>, auto: bool) {
        if self.playback.is_some() {
            return;
        }
        // single player: from the map's start
        if self.connection.is_none() {
            let map = self.map_name();
            self.stop_recording();
            self.begin_local_recording(name, auto, &map);
            self.change_map(&map);
            return;
        }
        self.stop_recording();
        let Some(connection) = &self.connection else {
            return;
        };
        let map = std::path::Path::new(&self.world.map.filename)
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let name = name.map_or_else(
            || format!("{}{map}", platform::local_time("%Y-%m-%d_%H-%M-%S_")),
            str::to_string,
        );
        let path = demo_path(&self.console.config_dir, &name);

        // the match so far, as the server would tell a newcomer
        let net = &connection.net;
        let mut nums: Vec<PlayerNum> = net.players.keys().copied().collect();
        nums.sort_unstable();
        let soldier = |num: &PlayerNum| self.world.soldiers.get(net.players[num]);
        let mut start: Vec<ServerMessage> = nums
            .iter()
            .filter_map(|num| {
                let s = soldier(num)?;
                Some(ServerMessage::PlayerJoined(PlayerInfo {
                    num: *num,
                    name: s.name.clone(),
                    team: s.team as u8,
                    looks: soldank_core::net::NetLooks::of(s),
                    bot: false,
                }))
            })
            .collect();
        let num_of = |id: SoldierId| net.players.iter().find(|(_, s)| **s == id).map(|(n, _)| *n);
        start.push(ServerMessage::Snapshot(Snapshot {
            // the snapshots to come are newer
            tick: net.snapshot_tick,
            soldiers: nums
                .iter()
                .filter_map(|num| Some(SoldierState::of(*num, soldier(num)?)))
                .collect(),
            things: self
                .world
                .things
                .iter()
                .enumerate()
                .filter(|(_, t)| t.active)
                .map(|(slot, t)| ThingState::of(slot, t, num_of))
                .collect(),
            team_scores: self.world.game.team_scores,
            time_left: self.world.game.time_left,
        }));

        let map_hash = self
            .vfs
            .read(&self.world.map.filename)
            .map_or(0, |bytes| soldank_core::net::file_hash(&bytes));
        let header = DemoHeader {
            protocol: soldank_core::net::PROTOCOL_VERSION,
            map,
            map_hash,
            cvars: self.remote.cvars.clone(),
            weapons_mods: self.world.data.weapons_mods.clone(),
            game_mod: self.remote.game_mod.clone(),
            you: net.you,
            start,
            date: platform::unix_time(),
            local: false,
        };
        self.create_recorder(&path, name, &header, auto);
    }

    /// Single player: a demo of `map`, which is about to start (over).
    pub(crate) fn begin_local_recording(&mut self, name: Option<&str>, auto: bool, map: &str) {
        let name = name.map_or_else(
            || format!("{}{map}", platform::local_time("%Y-%m-%d_%H-%M-%S_")),
            str::to_string,
        );
        let path = demo_path(&self.console.config_dir, &name);
        let cvars = self.game_cvars();
        let header = DemoHeader {
            protocol: soldank_core::net::PROTOCOL_VERSION,
            map: map.to_string(),
            map_hash: 0,
            cvars: cvars.clone(),
            weapons_mods: [None, None],
            game_mod: None,
            you: None,
            start: Vec::new(),
            date: platform::unix_time(),
            local: true,
        };
        self.create_recorder(&path, name, &header, auto);
        if let Some(recorder) = &mut self.recorder {
            recorder.local = Some(cvars);
        }
    }

    fn create_recorder(
        &mut self,
        path: &std::path::Path,
        name: String,
        header: &DemoHeader,
        auto: bool,
    ) {
        let writer = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| File::create(path))
            .and_then(|file| DemoWriter::new(BufWriter::new(file), header));
        match writer {
            Ok(writer) => {
                self.message(format!("Recording demo: {name}"), console_colors::GAME);
                self.recorder = Some(Recorder {
                    writer,
                    name,
                    frame: DemoFrame::default(),
                    auto,
                    local: None,
                });
            }
            Err(error) => {
                let text = format!("Failed to save demo file: {error}");
                self.message(text, console_colors::WARNING);
            }
        }
    }

    /// `stop`
    pub(crate) fn stop_recording(&mut self) {
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        let name = recorder.name;
        match recorder.writer.finish() {
            Ok(_) => self.message(format!("Demo stopped ({name})"), console_colors::GAME),
            Err(error) => {
                let text = format!("Failed to save demo file: {error}");
                self.message(text, console_colors::WARNING);
            }
        }
    }

    /// The tick's frame: its messages so far, and the player's input.
    pub(crate) fn record_frame(&mut self, input: &Input) {
        let own = self.player.is_some();
        let Some(recorder) = &mut self.recorder else {
            return;
        };
        let mut frame = std::mem::take(&mut recorder.frame);
        frame.set_input(own.then_some(input));
        if recorder.local.is_some() && (recorder.writer.frames + 1).is_multiple_of(CHECK_FRAMES) {
            frame.check = Some(world_check(&self.world));
        }
        if let Err(error) = recorder.writer.frame(&frame) {
            tracing::warn!(%error, "demo");
            self.stop_recording();
        }
    }

    /// Single player: something done to the world besides playing, recorded in a demo first.
    pub(crate) fn act(&mut self, action: LocalAction) -> bool {
        self.record_cvars();
        if let Some(recorder) = self.recorder.as_mut().filter(|r| r.local.is_some()) {
            recorder.frame.actions.push(action.clone());
        }
        self.apply_action(action)
    }

    /// Single player: the server cvars that changed since the demo last had them.
    pub(crate) fn record_cvars(&mut self) {
        let Some(last) = self.recorder.as_ref().and_then(|r| r.local.as_ref()) else {
            return;
        };
        let now = self.game_cvars();
        let changed: Vec<_> = now.iter().filter(|c| !last.contains(c)).cloned().collect();
        if changed.is_empty() {
            return;
        }
        if let Some(recorder) = &mut self.recorder {
            recorder.frame.actions.push(LocalAction::Cvars(changed));
            recorder.local = Some(now);
        }
    }

    /// The cvars the world follows (the server's; never its passwords).
    pub(crate) fn game_cvars(&self) -> Vec<(String, String)> {
        self.console
            .cvars
            .iter()
            .filter(|c| c.flags.contains(soldank_core::config::CvarFlags::SERVER))
            .filter(|c| !c.name.contains("password"))
            .map(|c| (c.name.clone(), c.value.to_string()))
            .collect()
    }

    /// The demo's map and match, from its start.
    pub(crate) fn begin_playback(&mut self) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        playback.next = 0;
        playback.finished = false;
        playback.started = true;
        playback.own = None;
        playback.check = None;
        let header = playback.demo.header.clone();
        // single player: the game's settings, then the frames start the map
        if header.local {
            for (cvar, value) in &header.cvars {
                let _ = self.console.cvars.set_now(cvar, value);
            }
            self.player = None;
            self.follow = None;
            self.bots.clear();
            return;
        }
        let data = self.base_data.with_weapons_mods(&header.weapons_mods);
        self.remote.data = Some(Arc::new(data));
        self.load_server_map(&header.map, &header.cvars);
        let Some(playback) = &mut self.playback else {
            return;
        };
        playback.net.you = header.you;
        for message in header.start {
            playback.net.apply(&mut self.world, message);
        }
        self.player = None;
        self.follow = playback.net.own();
    }

    /// The next frame: its messages into the world (`quiet`: without the game's reactions,
    /// when seeking), and the recording player's input. `None` once the demo's over.
    pub(crate) fn playback_frame(&mut self, quiet: bool) -> Option<PlayedFrame> {
        let playback = self.playback.as_mut()?;
        let Some(frame) = playback.demo.frames.get(playback.next).cloned() else {
            if !std::mem::replace(&mut playback.finished, true) {
                let text = format!("Demo finished ({})", playback.name);
                self.message(text, console_colors::GAME);
            }
            return None;
        };
        playback.next += 1;
        let input = frame.input();
        if playback.local() {
            return Some(self.play_local_frame(frame, input));
        }
        let mut events = Vec::new();
        for message in frame.messages {
            let Some(playback) = &mut self.playback else {
                break;
            };
            let Some(notice) = playback.net.apply(&mut self.world, message) else {
                continue;
            };
            let map = matches!(notice, soldank_core::net::Notice::MapChange { .. });
            if (!quiet || map)
                && let Some(event) = self.notice(notice)
            {
                events.push(event);
            }
        }
        let own = self.playback.as_ref()?.net.own();
        let inputs = own.zip(input).into_iter().collect();
        Some((events, inputs))
    }

    /// Single player: the frame's actions as the recording player, after checking that the
    /// world is where the game's was.
    fn play_local_frame(&mut self, frame: DemoFrame, input: Option<Input>) -> PlayedFrame {
        let Some(playback) = &mut self.playback else {
            return (Vec::new(), Vec::new());
        };
        if let Some(check) = playback.check.take()
            && check != world_check(&self.world)
            && !std::mem::replace(&mut playback.out_of_sync, true)
        {
            let text = format!("The demo went out of sync at tick {}", playback.next - 1);
            tracing::warn!("{text}");
            self.message(text, console_colors::WARNING);
        }
        let Some(playback) = &mut self.playback else {
            return (Vec::new(), Vec::new());
        };
        playback.check = frame.check;
        self.player = playback.own;
        for action in frame.actions {
            self.apply_action(action);
        }
        let own = self.player.take();
        let Some(playback) = &mut self.playback else {
            return (Vec::new(), Vec::new());
        };
        playback.own = own;
        (Vec::new(), own.zip(input).into_iter().collect())
    }

    /// `demo_tick`: to frame `tick`, played quietly from the last keyframe before it (or from
    /// the start).
    pub(crate) fn seek(&mut self, tick: usize) {
        let Some(playback) = &self.playback else {
            self.console.print("You are not playing a demo");
            return;
        };
        let keyframe = playback.keyframes.iter().rev().find(|k| k.next <= tick);
        // forward: from here, if no keyframe is nearer
        let here = playback.started
            && playback.next <= tick
            && keyframe.is_none_or(|k| k.next <= playback.next);
        match keyframe.cloned() {
            _ if here => {}
            Some(keyframe) => self.restore_keyframe(keyframe),
            None => self.begin_playback(),
        }
        while self.playback_tick().is_some_and(|next| next < tick) {
            let Some((_, inputs)) = self.playback_frame(true) else {
                break;
            };
            let local = self.playback.as_ref().is_some_and(Playback::local);
            let rules = WorldConfig::from_cvars(&self.console.cvars, &self.world.data);
            self.world.set_rules(rules);
            self.world.config.client = !local;
            // no sparks while skipping
            self.world.config.now.sparks_count = 0;
            self.world.step(&inputs);
            self.keep_keyframe();
        }
        self.sparks.clear();
        self.audio.stop_all();
    }

    /// After a played tick: the game as it is now, every [`KEYFRAME_FRAMES`].
    pub(crate) fn keep_keyframe(&mut self) {
        let cvars = self.game_cvars();
        let Some(playback) = &mut self.playback else {
            return;
        };
        let next = playback.next;
        if !next.is_multiple_of(KEYFRAME_FRAMES)
            || playback.keyframes.iter().any(|k| k.next == next)
        {
            return;
        }
        let keyframe = Keyframe {
            next,
            world: self.world.clone(),
            net: playback.net.clone(),
            own: playback.own,
            check: playback.check,
            cvars,
        };
        tracing::debug!(next, "demo keyframe kept");
        let at = playback.keyframes.partition_point(|k| k.next < next);
        playback.keyframes.insert(at, keyframe);
    }

    /// Back (or on) to a keyframe: its world, and the map's graphics if it's another map.
    fn restore_keyframe(&mut self, keyframe: Keyframe) {
        tracing::debug!(next = keyframe.next, "demo keyframe restored");
        let other_map = keyframe.world.map.filename != self.world.map.filename;
        for (cvar, value) in &keyframe.cvars {
            let _ = self.console.cvars.set_now(cvar, value);
        }
        self.world = keyframe.world;
        if other_map {
            let background = forced_background(&self.console.cvars);
            self.graphics
                .load_map(&mut self.context, &self.vfs, &self.world.map, background);
        }
        let Some(playback) = &mut self.playback else {
            return;
        };
        playback.next = keyframe.next;
        playback.net = keyframe.net;
        playback.own = keyframe.own;
        playback.check = keyframe.check;
        playback.finished = false;
        playback.started = true;
    }

    /// The frame being played.
    pub(crate) fn playback_tick(&self) -> Option<usize> {
        self.playback.as_ref().map(|p| p.next)
    }

    /// The camera while watching: the recording player, or whom fire (next) and jets
    /// (previous) pick; jump for the free camera.
    pub(crate) fn playback_follow(&mut self) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        let buttons = self.input.buttons;
        let pressed = buttons & !playback.buttons;
        playback.buttons = buttons;
        let own = match playback.local() {
            true => playback.own,
            false => playback.net.own(),
        };
        // the recording player, once there (and back when the followed one's gone)
        let gone = self
            .follow
            .is_none_or(|id| !self.world.soldiers.contains_key(id));
        if gone && !playback.free_cam {
            self.follow = own;
        }
        let ids: Vec<SoldierId> = self
            .world
            .soldiers
            .iter()
            .filter(|(_, s)| s.active && !s.is_spectator())
            .map(|(id, _)| id)
            .collect();
        let step = |by: isize, from: Option<SoldierId>| {
            if ids.is_empty() {
                return None;
            }
            let at = from.and_then(|f| ids.iter().position(|&id| id == f));
            let next = match at {
                Some(i) => (i as isize + by).rem_euclid(ids.len() as isize) as usize,
                None => 0,
            };
            Some(ids[next])
        };
        if pressed.contains(Buttons::FIRE) {
            self.follow = step(1, self.follow);
        } else if pressed.contains(Buttons::JETS) {
            self.follow = step(-1, self.follow);
        } else if pressed.contains(Buttons::JUMP) {
            self.follow = None;
        }
        if let Some(playback) = &mut self.playback {
            if pressed.contains(Buttons::JUMP) {
                playback.free_cam = true;
            } else if pressed.intersects(Buttons::FIRE | Buttons::JETS) {
                playback.free_cam = false;
            }
        }
    }
}
