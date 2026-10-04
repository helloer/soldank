//! What the player sees: the camera and whom it follows, and the interface's state for the
//! renderer (menus, radio, votes, stats).

use super::*;

/// The interface's state: the menus, the console and kill messages, the weapon stats, the
/// weapons and radio menus' picks, the server's vote.
pub(crate) struct Hud {
    pub(crate) menus: Menus,
    pub(crate) messages: render::hud::HudMessages,
    pub(crate) stats: stats::WeaponStats,
    /// `SelWeapon`: the primary weapon picked in the weapons menu.
    pub(crate) sel_weapon: Option<WeaponKind>,
    /// The kick menu's player and the map menu's map (`KickMenuIndex`, `MapMenuIndex`).
    pub(crate) kick: Option<SoldierId>,
    pub(crate) map_index: usize,
    /// The radio menu (`ShowRadioMenu`, `RMenuState`), its texts and `RadioCooldown`.
    pub(crate) radio: Option<[Option<u8>; 2]>,
    pub(crate) radio_texts: HashMap<String, String>,
    pub(crate) radio_cooldown: u8,
    /// `MenuTimer`: switching the camera waits a little.
    pub(crate) menu_timer: i32,
    /// The server's running vote, and the player a kick vote is being typed against.
    pub(crate) vote: Option<vote::VoteBox>,
    pub(crate) kick_vote: Option<soldank_core::net::PlayerNum>,
    /// Debug drawing over the world, set from the dev overlay.
    pub(crate) debug_draw: render::debug::DebugDraw,
}

/// Action snap (`cl_actionsnap`): a kill on screen is caught a few frames later, without
/// the interface, to look at (the `snap` key) or keep (`screenshot`) while it's offered.
#[derive(Default)]
pub(crate) struct ActionSnap {
    /// Seconds left of the offer (`ScreenCounter`), frames to the catch (`CapScreen`).
    pub counter: Option<u8>,
    pub capture_in: Option<u8>,
    /// What was caught (`ActionSnapTaken`), and it fills the screen (`ShowScreen`).
    pub image: Option<gfx2d::Texture>,
    pub show: bool,
    /// The screenshot keeps it: it goes away after that frame.
    pub close_after_shot: bool,
}

/// What the interface shows that the game works out ([`Game::shown`]).
pub(crate) struct Shown {
    /// The kick menu's player (name, shirt colour) and the map menu's map.
    kick: Option<(String, u32)>,
    map_name: Option<String>,
    radio: Option<render::interface::RadioView>,
    vote: Option<render::interface::VoteView>,
    weapon_tips: Option<Vec<String>>,
    con_info: Option<render::interface::ConInfo>,
    recording: bool,
    wide_cut: bool,
    /// "Press F5 to View Screen Cap", and no crosshair (a demo without `demo_showcrosshair`).
    snap_offered: bool,
    no_crosshair: bool,
}

impl Hud {
    /// The interface's state for the frame: the HUD's, with what the game shows and the
    /// player's view.
    pub(crate) fn interface_state<'a>(
        &'a self,
        shown: Shown,
        chat: &'a Chat,
        world: &World,
        player: Option<SoldierId>,
        cvars: &'a Cvars,
        (mouse, follow): (Vec2, Option<SoldierId>),
    ) -> InterfaceState<'a> {
        let mut team_counts = [0; 6];
        for soldier in world.soldiers.values() {
            team_counts[soldier.team as usize] += 1;
        }
        let dead = player.is_none_or(|id| world.soldiers[id].dead_meat);
        let spectating = player.is_some_and(|id| world.soldiers[id].is_spectator());
        InterfaceState {
            menus: &self.menus,
            messages: &self.messages,
            player,
            spectating,
            sel_weapon: self.sel_weapon,
            secondary: cvars.int("cl_player_secwep"),
            team_counts,
            mouse,
            show_cursor: self.menus.any_active() || dead,
            alpha: cvars.int("ui_status_transparency") as u8,
            cvars,
            chat,
            follow,
            kick: shown.kick,
            map_name: shown.map_name,
            radio: shown.radio,
            vote: shown.vote,
            weapon_tips: shown.weapon_tips,
            stats: &self.stats,
            debug: self.debug_draw,
            con_info: shown.con_info,
            recording: shown.recording,
            wide_cut: shown.wide_cut,
            snap_offered: shown.snap_offered,
            no_crosshair: shown.no_crosshair,
        }
    }

    pub(crate) fn new(menus: Menus, radio_texts: HashMap<String, String>) -> Hud {
        Hud {
            menus,
            messages: Default::default(),
            stats: Default::default(),
            sel_weapon: None,
            kick: None,
            map_index: 0,
            radio: None,
            radio_texts,
            radio_cooldown: 3,
            menu_timer: 0,
            vote: None,
            kick_vote: None,
            debug_draw: Default::default(),
        }
    }
}

impl Game {
    /// The radio menu to show (only in the modes with flags, `RADIO_GAMESTYLES`).
    pub(crate) fn radio_view(&self) -> Option<render::interface::RadioView> {
        let state = self.hud.radio?;
        let flags = matches!(
            self.world.config.game_mode,
            GameMode::CaptureTheFlag | GameMode::Infiltration | GameMode::HoldTheFlag
        );
        if !flags {
            return None;
        }
        let text = |key: String| self.hud.radio_texts.get(&key).cloned().unwrap_or_default();
        let chosen = state[0].map(|c| usize::from(c) - 1);
        let subject = RADIO_SUBJECTS[chosen.unwrap_or(0)];
        Some(render::interface::RadioView {
            subjects: RADIO_SUBJECTS.map(|s| text(format!("Menu1{s}"))),
            chosen,
            places: RADIO_WHERE.map(|p| text(format!("Menu2{subject}{p}"))),
        })
    }

    /// The player shown in the kick menu (`KickMenuIndex`): the first one at first.
    pub(crate) fn kick_target(&self) -> Option<SoldierId> {
        self.hud
            .kick
            .filter(|&id| self.world.soldiers.contains_key(id))
            .or_else(|| self.world.soldiers.keys().next())
    }

    /// The maps of the mod's `configs/mapslist.txt`.
    pub(crate) fn map_list(&self) -> Vec<String> {
        if self.connection.is_some() {
            return self.remote.maps.clone();
        }
        self.vfs
            .read_to_string("configs/mapslist.txt")
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// What the interface shows that the game works out (owned: the frame borrows the rest).
    pub(crate) fn shown(&self) -> Shown {
        Shown {
            kick: self
                .kick_target()
                .and_then(|id| self.world.soldiers.get(id))
                .map(|s| (s.name.clone(), s.looks.shirt)),
            map_name: self.map_list().get(self.hud.map_index).cloned(),
            radio: self.radio_view(),
            vote: self.vote_view(),
            weapon_tips: self.weapon_tips(),
            con_info: self.hud.menus.con_info.then(|| self.con_info()),
            recording: self.recorder.is_some(),
            // `WideScreenCut`
            wide_cut: self.world.config.bullet_time
                && self.world.bullet_time > 0
                && !self.world.game.ended(),
            snap_offered: self.console.cvars.bool("cl_actionsnap")
                && self.action_snap.counter.is_some()
                && !self.action_snap.show,
            no_crosshair: self.playback.is_some() && !self.console.cvars.bool("demo_showcrosshair"),
        }
    }

    /// The GameStats texts: the frame rate, the player's ping, and the demo's time or the
    /// connection's.
    pub(crate) fn con_info(&self) -> render::interface::ConInfo {
        let ping = self
            .player
            .and_then(|id| self.world.soldiers.get(id))
            .map(|s| s.ping);
        let demo = self
            .playback
            .as_ref()
            .map(|p| (self.playback_tick().unwrap_or(0), p.demo.frames.len()));
        render::interface::ConInfo {
            fps: self.clock.fps,
            ping,
            demo,
            net: self.connection.as_ref().map(|c| c.stats()),
        }
    }

    /// Who the camera follows (`CameraFollowSprite`): the player while alive; dead, fire,
    /// jets or jump switch to the next teammate (jets backwards, jump to the free camera).
    pub(crate) fn update_follow(&mut self) {
        if self.hud.menu_timer > -1 {
            self.hud.menu_timer -= 1;
        }
        let me = self.player.and_then(|id| self.world.soldiers.get(id));
        let Some(me) = me else {
            // not playing yet: whom the map start found, while there
            if self
                .follow
                .is_some_and(|id| !self.world.soldiers.contains_key(id))
            {
                self.follow = None;
            }
            return;
        };

        if !me.dead_meat && !self.world.game.ended() {
            self.follow = self.player;
            return;
        }
        // the followed soldier left
        if self
            .follow
            .is_some_and(|id| !self.world.soldiers.contains_key(id))
        {
            self.follow = None;
        }

        let buttons = self.input.buttons;
        let pressed = buttons.intersects(Buttons::FIRE | Buttons::JETS | Buttons::JUMP);
        if self.free_cam_pressed && !pressed {
            self.free_cam_pressed = false;
        }
        if self.hud.menu_timer < 1 && !self.hud.menus.limbo_active && me.dead_meat && pressed {
            let target = self.camera_target(buttons.contains(Buttons::JETS));
            if target.is_some() || !self.free_cam_pressed {
                self.follow = target;
                self.camera.center_mouse();
                self.hud.menu_timer = 10;
                self.free_cam_pressed = target.is_none();
            }
        }

        // realistic survival: away from the dead now and then
        let config = &self.world.config;
        if self.world.tick.is_multiple_of(300)
            && config.realistic_mode
            && config.survival_mode
            && !self.world.game.survival_end_round
            && self
                .follow
                .and_then(|id| self.world.soldiers.get(id))
                .is_some_and(|s| s.dead_meat)
        {
            self.follow = self.camera_target(false);
        }
    }

    /// `GetCameraTarget`: the next living teammate by player number (anyone for a
    /// spectator), or the free camera.
    pub(crate) fn camera_target(&self, backwards: bool) -> Option<SoldierId> {
        let ids: Vec<SoldierId> = self.world.soldiers.keys().collect();
        let me = self.player.and_then(|id| self.world.soldiers.get(id))?;
        let up = self.input.buttons.contains(Buttons::JUMP);
        let mut cam = self
            .follow
            .and_then(|f| ids.iter().position(|&id| id == f))
            .map_or(0, |i| i as i32 + 1);
        for _ in 0..32 {
            cam += if backwards { -1 } else { 1 };
            if cam > 32 {
                cam = 1;
            } else if cam < 1 {
                cam = 32;
            }
            let Some(&id) = ids.get(cam as usize - 1) else {
                continue;
            };
            let soldier = &self.world.soldiers[id];
            if !soldier.active || soldier.dead_meat || soldier.is_spectator() {
                continue;
            }
            // spectators see everyone, and jump to the free camera even in realistic mode
            if me.is_spectator() {
                return if up { None } else { Some(id) };
            }
            if up && !self.world.config.realistic_mode {
                return None;
            }
            if soldier.team != me.team {
                continue;
            }
            return Some(id);
        }
        None
    }

    /// A deathmatch won by the kill limit: everyone watches the winner.
    pub(crate) fn follow_winner(&mut self) {
        let config = &self.world.config;
        if config.game_mode.is_team_game() {
            return;
        }
        if let Some((id, _)) = self
            .world
            .soldiers
            .iter()
            .find(|(_, s)| s.active && s.kills >= config.kill_limit)
        {
            self.follow = Some(id);
            if !self.hud.menus.any_active() {
                self.camera.center_mouse();
            }
        }
    }

    /// The screen shakes with shots (`TSprite.Fire`): your own, and with `cl_screenshake`
    /// anyone's near the followed soldier's view; heavy guns shake it harder.
    pub(crate) fn fire_shake(&mut self, events: &[GameEvent]) {
        use WeaponKind::*;
        let (Some(me), Some(follow)) = (self.player, self.follow) else {
            return;
        };
        let Some(viewer) = self.world.soldiers.get(follow) else {
            return;
        };
        let others = self.console.cvars.bool("cl_screenshake");
        // PointVisible: around halfway between the followed soldier and its aim
        let aim = vec2(
            viewer.control.mouse_aim_x as f32,
            viewer.control.mouse_aim_y as f32,
        );
        let center = viewer.particle.pos - (viewer.particle.pos - aim) / 2.0;
        let reach = vec2(self.camera.game_width, self.camera.game_height);
        let mut shooters = Vec::new();
        for event in events {
            let GameEvent::BulletFired { owner, weapon, .. } = *event else {
                continue;
            };
            let thrown = matches!(
                weapon,
                Chainsaw | FragGrenade | ClusterGrenade | Cluster | ThrownKnife | M2
            );
            if thrown || shooters.contains(&owner) || (owner != me && !others) {
                continue;
            }
            shooters.push(owner);
            let Some(shooter) = self.world.soldiers.get(owner) else {
                continue;
            };
            let d = (shooter.particle.pos - center).abs();
            if d.x < reach.x && d.y < reach.y {
                let amp = if matches!(weapon, Minimi | Spas12 | Barrett | Minigun) {
                    3
                } else {
                    1
                };
                let r = |_| (fx::random(2 * amp + 1) - amp) as f32;
                self.camera.pos += vec2(r(0), r(1));
            }
        }
    }

    /// What a weapons mod changed, for the weapons menu (while it's open).
    pub(crate) fn weapon_tips(&self) -> Option<Vec<String>> {
        if !self.hud.menus.limbo_active {
            return None;
        }
        let realistic = self.world.config.realistic_mode;
        let default = WeaponTable::new(realistic, None).ok()?;
        render::interface::weapon_tips(&self.world.config.weapons, &default)
    }

    /// The player's soldier on a server (or in the demo of one).
    pub(crate) fn own_soldier(&self) -> Option<SoldierId> {
        let net = match (&self.connection, &self.playback) {
            (Some(connection), _) => &connection.net,
            (None, Some(playback)) => &playback.net,
            (None, None) => return None,
        };
        net.own()
    }

    pub(crate) fn update_camera(&mut self) {
        let follow = self
            .follow
            .and_then(|id| self.world.soldiers.get(id))
            .map(|s| (s.particle.pos, s.aim_dist_coef));
        // r_zoom, plus the numpad zoom of soldank's dev builds
        self.dev_zoom += self.input.zoom_dir() * DT as f32;
        let zoom_goal = self.console.cvars.float("r_zoom") + self.dev_zoom;
        self.camera.update(follow, zoom_goal);
    }
}
