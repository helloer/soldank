use super::*;
use gfx::SpriteData;
use ini::Ini;
use std::str::FromStr;

pub struct GameGraphics {
    map: MapGraphics,
    soldier_graphics: SoldierGraphics,
    sprites: Vec<Vec<Sprite>>,
    batch: DrawBatch,
}

impl GameGraphics {
    pub fn new() -> GameGraphics {
        GameGraphics {
            map: MapGraphics::empty(),
            soldier_graphics: SoldierGraphics::new(),
            sprites: Vec::new(),
            batch: DrawBatch::new(),
        }
    }

    pub fn render_frame(
        &mut self,
        context: &mut Gfx2dContext,
        world: &World,
        camera: &Camera,
        elapsed: f64,
        frame_percent: f32,
    ) {
        let zoom = f32::exp(camera.zoom);
        let cam = lerp(camera.pos_prev, camera.pos, frame_percent);
        let (w, h) = (zoom * camera.game_width, zoom * camera.game_height);
        let (dx, dy) = (cam.x - w / 2.0, cam.y - h / 2.0);
        let transform = Transform::ortho(dx, dx + w, dy, dy + h).matrix();
        let transform_bg = Transform::ortho(0.0, 1.0, dy, dy + h).matrix();

        self.batch.clear();

        for soldier in world.soldiers.values() {
            render_soldier(
                soldier,
                &self.soldier_graphics,
                &self.sprites,
                &mut self.batch,
                frame_percent,
                world.config.realistic_mode,
            );
        }

        if false {
            let px = h / mq::window::screen_size().1;
            for soldier in world.soldiers.values() {
                render_skeleton(soldier, &mut self.batch, px, frame_percent);
            }
        }

        for thing in world.things.iter().filter(|t| t.active) {
            render_thing(
                thing,
                &world.config.weapons,
                &self.sprites,
                &mut self.batch,
                frame_percent,
            );
        }

        for bullet in world.bullets.iter().filter(|b| b.active) {
            render_bullet(
                bullet,
                &self.sprites,
                &mut self.batch,
                elapsed,
                frame_percent,
            );
        }

        context.clear(rgb(0, 0, 0));
        context.draw(&mut self.map.background(), &transform_bg);
        context.draw(&mut self.map.polys_back(), &transform);
        context.draw(&mut self.map.scenery_back(), &transform);
        context.draw(&mut self.batch.all(), &transform);
        context.draw(&mut self.map.scenery_mid(), &transform);
        context.draw(&mut self.map.polys_front(), &transform);
        context.draw(&mut self.map.scenery_front(), &transform);
        self.render_cursor(context, camera);
    }

    fn render_cursor(&mut self, context: &mut Gfx2dContext, camera: &Camera) {
        let zoom = f32::exp(camera.zoom);
        let (w, h) = (zoom * camera.game_width, zoom * camera.game_height);
        let (sw, sh) = mq::window::screen_size();
        let size = vec2(sw, sh);
        let x = zoom * f32::floor(camera.mouse.x * size.x / w);
        let y = zoom * f32::floor(camera.mouse.y * size.y / h);
        let screen = Transform::ortho(0.0, size.x, 0.0, size.y).matrix();

        self.batch.clear();

        self.batch.add_quad(
            None,
            &[
                vertex(vec2(x, y) + vec2(0.0, -8.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(1.0, -8.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(1.0, 9.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(0.0, 9.0), Vec2::ZERO, rgb(0, 0, 0)),
            ],
        );

        self.batch.add_quad(
            None,
            &[
                vertex(vec2(x, y) + vec2(-8.0, 0.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(9.0, 0.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(9.0, 1.0), Vec2::ZERO, rgb(0, 0, 0)),
                vertex(vec2(x, y) + vec2(-8.0, 1.0), Vec2::ZERO, rgb(0, 0, 0)),
            ],
        );

        context.draw(&mut self.batch.all(), &screen);
    }

    pub fn load_map(&mut self, context: &mut Gfx2dContext, vfs: &Vfs, map: &MapFile) {
        self.map = MapGraphics::new(context, vfs, map);
    }

    pub fn load_sprites(&mut self, context: &mut Gfx2dContext, vfs: &Vfs) {
        let mut main: Vec<SpriteInfo> = Vec::new();
        let mut intf: Vec<SpriteInfo> = Vec::new();

        let add_to = |v: &mut Vec<SpriteInfo>, fname: &str| {
            let (name, image) = load_image(vfs, "", fname);
            v.push(SpriteInfo::new(name, image, vec2(1.0, 1.0), None));
        };

        for group in gfx::Group::values() {
            match *group {
                gfx::Group::Soldier => gfx::Soldier::values()
                    .iter()
                    .map(|v| v.filename())
                    .for_each(|f| add_to(&mut main, f)),

                gfx::Group::Weapon => gfx::Weapon::values()
                    .iter()
                    .map(|v| v.filename())
                    .for_each(|f| add_to(&mut main, f)),

                gfx::Group::Spark => gfx::Spark::values()
                    .iter()
                    .map(|v| v.filename())
                    .for_each(|f| add_to(&mut main, f)),

                gfx::Group::Object => gfx::Object::values()
                    .iter()
                    .map(|v| v.filename())
                    .for_each(|f| add_to(&mut main, f)),

                gfx::Group::Interface => gfx::Interface::values()
                    .iter()
                    .map(|v| v.filename())
                    .for_each(|f| add_to(&mut intf, f)),
            }
        }

        let mod_ini = vfs
            .read_to_string("mod.ini")
            .map_err(|e| e.to_string())
            .and_then(|text| Ini::load_from_str(&text).map_err(|e| e.to_string()));

        if let Err(error) = &mod_ini {
            tracing::warn!(error, "cannot load mod.ini, using defaults");
        }

        if let Ok(cfg) = mod_ini {
            self.soldier_graphics.load_data(&cfg);

            if let Some(data) = cfg.section(Some("SCALE".to_owned())) {
                let default_scale = match data.get("DefaultScale") {
                    None => 1.0,
                    Some(scale) => f32::from_str(scale).unwrap_or(1.0),
                };

                for sprite_info in main.iter_mut().chain(intf.iter_mut()) {
                    let scale = match data.get(&sprite_info.name) {
                        None => default_scale,
                        Some(scale) => f32::from_str(scale).unwrap_or(default_scale),
                    };

                    sprite_info.pixel_ratio = vec2(scale, scale);
                }
            }
        }

        let main = Spritesheet::new(context, 8, FilterMethod::Trilinear, main);
        let intf = Spritesheet::new(context, 8, FilterMethod::Trilinear, intf);

        self.sprites.clear();
        self.sprites.resize(gfx::Group::values().len(), Vec::new());

        let mut imain = 0;
        let mut iintf = 0;

        for group in gfx::Group::values() {
            let index = group.id();

            match *group {
                gfx::Group::Soldier => {
                    for _ in gfx::Soldier::values() {
                        self.sprites[index].push(main.sprites[imain].clone());
                        imain += 1;
                    }
                }
                gfx::Group::Weapon => {
                    for _ in gfx::Weapon::values() {
                        self.sprites[index].push(main.sprites[imain].clone());
                        imain += 1;
                    }
                }
                gfx::Group::Spark => {
                    for _ in gfx::Spark::values() {
                        self.sprites[index].push(main.sprites[imain].clone());
                        imain += 1;
                    }
                }
                gfx::Group::Object => {
                    for _ in gfx::Object::values() {
                        self.sprites[index].push(main.sprites[imain].clone());
                        imain += 1;
                    }
                }
                gfx::Group::Interface => {
                    for _ in gfx::Interface::values() {
                        self.sprites[index].push(intf.sprites[iintf].clone());
                        iintf += 1;
                    }
                }
            }
        }
    }
}
