macro_rules! iif(
    ($cond:expr, $then:expr, $else:expr) => (if $cond { $then } else { $else })
);

mod anims;
mod bullet;
mod calc;
mod control;
mod mapfile;
mod particles;
mod render;
mod soldier;
mod state;
mod weapons;

use anims::*;
use bullet::*;
use calc::*;
use control::*;
use mapfile::*;
use particles::*;
use render::*;
use soldier::*;
use state::*;
use weapons::*;

use clap::Parser;
use gfx2d::mq::{self, EventHandler, KeyCode, KeyMods, MouseButton, conf, window};
use std::time::Instant;

const GRAV: f32 = 0.06;

#[derive(Parser)]
#[command(name = "Soldank", version = "0.0.1")]
#[command(about = "open source clone of Soldat engine written in rust")]
struct Cli {
    /// name of map to load
    #[arg(short, long, default_value = "ctf_Ash")]
    map: String,
}

fn main() {
    let cli = Cli::parse();

    AnimData::initialize();
    Soldier::initialize();

    let map = MapFile::load_map_file(&format!("{}.pms", cli.map));

    const W: u32 = 1280;
    const H: u32 = 720;

    let state = MainState {
        map,
        game_width: W as f32 * (480.0 / H as f32),
        game_height: 480.0,
        camera: Vec2::ZERO,
        camera_prev: Vec2::ZERO,
        mouse: Vec2::ZERO,
        mouse_prev: Vec2::ZERO,
        gravity: GRAV,
        zoom: 0.0,
        bullets: vec![],
    };

    let conf = conf::Conf {
        window_title: "Soldank".to_owned(),
        window_width: W as i32,
        window_height: H as i32,
        window_resizable: false,
        platform: conf::Platform {
            linux_backend: conf::LinuxBackend::X11WithWaylandFallback,
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };

    mq::start(conf, move || Box::new(Game::new(state)));
}

struct Game {
    context: gfx2d::Gfx2dContext,
    graphics: GameGraphics,
    state: MainState,
    soldier: Soldier,
    emitter: Vec<EmitterItem>,
    weapons: Vec<Weapon>,
    time_start: Instant,
    timecur: f64,
    timeprv: f64,
    timeacc: f64,
    zoomin_pressed: bool,
    zoomout_pressed: bool,
}

impl Game {
    fn new(mut state: MainState) -> Game {
        let soldier = Soldier::new(&state.map.spawnpoints[0]);
        state.camera = soldier.particle.pos;

        let mut context = gfx2d::Gfx2dContext::new();
        window::show_mouse(false);
        window::set_cursor_grab(true);

        let mut graphics = GameGraphics::new();
        graphics.load_sprites(&mut context);
        graphics.load_map(&mut context, &state.map);

        let weapons: Vec<Weapon> = WeaponKind::values()
            .iter()
            .map(|k| Weapon::new(*k, false))
            .collect();

        Game {
            context,
            graphics,
            state,
            soldier,
            emitter: Vec::new(),
            weapons,
            time_start: Instant::now(),
            timecur: 0.0,
            timeprv: 0.0,
            timeacc: 0.0,
            zoomin_pressed: false,
            zoomout_pressed: false,
        }
    }

    fn current_time(&self) -> f64 {
        self.time_start.elapsed().as_secs_f64()
    }
}

const DT: f64 = 1.0 / 60.0;

impl EventHandler for Game {
    fn update(&mut self) {
        let dt = DT;

        self.timecur = self.current_time();
        self.timeacc += self.timecur - self.timeprv;
        self.timeprv = self.timecur;

        while self.timeacc >= dt {
            self.timeacc -= dt;

            // remove inactive bullets

            let mut i = 0;
            while i < self.state.bullets.len() {
                if !self.state.bullets[i].active {
                    self.state.bullets.swap_remove(i);
                } else {
                    i += 1;
                }
            }

            // update soldiers

            self.soldier.update(&self.state, &mut self.emitter);

            // update bullets

            for bullet in self.state.bullets.iter_mut() {
                bullet.update(&self.state.map);
            }

            // create emitted objects

            for item in self.emitter.drain(..) {
                match item {
                    EmitterItem::Bullet(params) => self.state.bullets.push(Bullet::new(&params)),
                };
            }

            // update camera

            self.state.camera_prev = self.state.camera;
            self.state.mouse_prev = self.state.mouse;

            if self.zoomin_pressed ^ self.zoomout_pressed {
                self.state.zoom += iif!(self.zoomin_pressed, -1.0, 1.0) * dt as f32;
            }

            self.state.camera = {
                let z = f32::exp(self.state.zoom);
                let mut m = Vec2::ZERO;

                m.x = z * (self.state.mouse.x - self.state.game_width / 2.0) / 7.0
                    * ((2.0 * 640.0 / self.state.game_width - 1.0)
                        + (self.state.game_width - 640.0) / self.state.game_width * 0.0 / 6.8);
                m.y = z * (self.state.mouse.y - self.state.game_height / 2.0) / 7.0;

                let mut cam_v = self.state.camera;
                let p = self.soldier.particle.pos;
                let norm = p - cam_v;
                let s = norm * 0.14;
                cam_v += s;
                cam_v += m;
                cam_v
            };

            self.timecur = self.current_time();
            self.timeacc += self.timecur - self.timeprv;
            self.timeprv = self.timecur;
        }
    }

    fn draw(&mut self) {
        let p = f64::clamp(self.timeacc / DT, 0.0, 1.0);

        self.graphics.render_frame(
            &mut self.context,
            &self.state,
            &self.soldier,
            self.timecur - DT * (1.0 - p),
            p as f32,
        );

        self.context.present();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, repeat: bool) {
        match keycode {
            KeyCode::Escape => window::request_quit(),
            KeyCode::KpAdd => self.zoomin_pressed = true,
            KeyCode::KpSubtract => self.zoomout_pressed = true,
            KeyCode::Tab => {
                if !repeat {
                    let soldier = &mut self.soldier;
                    let index = soldier.primary_weapon().kind.index();
                    let index = (index + 1) % (WeaponKind::NoWeapon.index() + 1);
                    soldier.weapons[soldier.active_weapon] = self.weapons[index];
                }
            }
            _ => self.soldier.update_keys(keycode, true),
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        match keycode {
            KeyCode::KpAdd => self.zoomin_pressed = false,
            KeyCode::KpSubtract => self.zoomout_pressed = false,
            _ => self.soldier.update_keys(keycode, false),
        }
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        self.soldier.update_mouse_button(button, true);
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        self.soldier.update_mouse_button(button, false);
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let (w, h) = window::screen_size();
        self.state.mouse.x = x * self.state.game_width / w;
        self.state.mouse.y = y * self.state.game_height / h;
    }
}
