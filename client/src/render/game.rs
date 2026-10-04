use super::*;
use gfx::SpriteData;
use ini::Ini;
use std::str::FromStr;

pub struct GameGraphics {
    map: MapGraphics,
    debug_batch: DrawBatch,
    /// The interface layout (`ui_style`), loaded with the sprites.
    pub layout: layout::Layout,
    minimap: Option<minimap::Minimap>,
    soldier_graphics: SoldierGraphics,
    sprites: Vec<Vec<Sprite>>,
    batch: DrawBatch,
    fonts: Option<Fonts>,
    /// The sprite sheets' textures, deleted when the sprites load again.
    sheets: Vec<gfx2d::Texture>,
}

impl GameGraphics {
    pub fn new() -> GameGraphics {
        GameGraphics {
            map: MapGraphics::empty(),
            debug_batch: DrawBatch::new(),
            layout: layout::Layout::default(),
            minimap: None,
            soldier_graphics: SoldierGraphics::new(),
            sprites: Vec::new(),
            batch: DrawBatch::new(),
            fonts: None,
            sheets: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_frame(
        &mut self,
        context: &mut Gfx2dContext,
        world: &World,
        camera: &Camera,
        elapsed: f64,
        frame_percent: f32,
        interface: &InterfaceState,
        sparks: &crate::sparks::Sparks,
        grab: bool,
    ) -> Option<gfx2d::image::RgbaImage> {
        self.map.animate(elapsed);
        let zoom = f32::exp(camera.zoom);
        let cam = lerp(camera.pos_prev, camera.pos, frame_percent);
        let (w, h) = (zoom * camera.game_width, zoom * camera.game_height);
        let (dx, dy) = (cam.x - w / 2.0, cam.y - h / 2.0);
        let transform = Transform::ortho(dx, dx + w, dy, dy + h).matrix();
        let transform_bg = Transform::ortho(0.0, 1.0, dy, dy + h).matrix();

        self.batch.clear();

        // `RenderFrame`'s order: bullets, soldiers, things, sparks; the middle scenery; flag
        // cloths and kits; the front polygons and scenery
        // (a late shot's trail stays a little after it's gone)
        let shown = |b: &&Bullet| {
            (b.active || b.ping_add > 0)
                && !bullet_hidden(b, world, interface.player, camera.game_width, frame_percent)
        };
        for bullet in world.bullets.iter().filter(shown) {
            render_bullet(
                bullet,
                &self.sprites,
                &mut self.batch,
                elapsed,
                frame_percent,
            );
        }

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

        for thing in world.things.iter().filter(|t| t.active) {
            render_thing(
                thing,
                world,
                &self.sprites,
                &mut self.batch,
                frame_percent,
                elapsed,
            );
        }

        sparks::render_sparks(sparks, world, &self.sprites, &mut self.batch, frame_percent);
        let game_layer = self.batch.split();

        // (`PolygonsRender`)
        for thing in world.things.iter().filter(|t| t.active) {
            render_thing_polygons(thing, world, &self.sprites, &mut self.batch, frame_percent);
        }
        let polygons = self.batch.split();

        context.clear(rgb(0, 0, 0));
        context.draw(&mut self.map.background(), &transform_bg);
        context.draw(&mut self.map.polys_back(), &transform);
        // r_renderbackground: the scenery behind everything
        if interface.cvars.bool("r_renderbackground") {
            context.draw(&mut self.map.scenery_back(), &transform);
        }
        context.draw(&mut self.batch.slice(game_layer), &transform);
        context.draw(&mut self.map.scenery_mid(), &transform);
        context.draw(&mut self.batch.slice(polygons), &transform);
        context.draw(&mut self.map.polys_front(), &transform);
        context.draw(&mut self.map.scenery_front(), &transform);
        if interface.debug.any() {
            let px = h / context.viewport().height;
            self.debug_batch.clear();
            debug::render_debug(
                &mut self.debug_batch,
                world,
                interface.debug,
                px,
                frame_percent,
            );
            context.draw(&mut self.debug_batch.all(), &transform);
        }
        // the action snap: the frame without the interface (`GrabActionSnap`)
        let caught = grab.then(|| context.read_screen());
        let width = camera.game_width;
        // (`r_renderui`)
        if interface.cvars.bool("r_renderui") {
            self.render_interface(context, interface, world, (cam, zoom, width), elapsed);
        }
        caught
    }

    /// The action snap over the whole screen, and what to do with it
    /// (`RenderActionSnapText`).
    pub fn render_snap(
        &mut self,
        context: &mut Gfx2dContext,
        image: &gfx2d::Texture,
        width: f32,
        elapsed: f64,
    ) {
        context.clear(rgb(0, 0, 0));
        let sh = context.viewport().height;
        let fonts = self
            .fonts
            .get_or_insert_with(|| Fonts::load(&Vfs::new(), ""));
        fonts.prepare(context, sh / 480.0);
        self.batch.clear();
        let white = rgb(255, 255, 255);
        self.batch.add_quad(
            Some(image),
            &[
                vertex(vec2(0.0, 0.0), vec2(0.0, 0.0), white),
                vertex(vec2(width, 0.0), vec2(1.0, 0.0), white),
                vertex(vec2(width, 480.0), vec2(1.0, 1.0), white),
                vertex(vec2(0.0, 480.0), vec2(0.0, 1.0), white),
            ],
        );
        let alpha = 150 + ((5.1 * elapsed).sin() * 100.0).round().abs() as u8;
        let text = "[[ Press F4 to Save Screen Cap ]]     [[ Press F5 to Cancel ]]";
        let pos = vec2(30.0 * width / 640.0, 412.0);
        let color = rgba(230, 65, 60, alpha);
        fonts.draw(&mut self.batch, FontStyle::Small, text, pos, color, None);
        let screen = Transform::ortho(0.0, width, 0.0, 480.0).matrix();
        context.draw(&mut self.batch.all(), &screen);
    }

    /// `RenderGameInfo`: a text on a plain screen (connecting, downloading, refused, ...),
    /// with "Press ESC to quit the game" below.
    pub fn render_game_info(&mut self, context: &mut Gfx2dContext, text: &str, width: f32) {
        context.clear(rgb(49, 61, 79));
        let sh = context.viewport().height;
        let fonts = self
            .fonts
            .get_or_insert_with(|| Fonts::load(&Vfs::new(), ""));
        fonts.prepare(context, sh / 480.0);
        self.batch.clear();
        let lines = [
            (FontStyle::Menu, text, 0.0),
            (FontStyle::Small, "Press ESC to quit the game", 100.0),
        ];
        for (style, line, below) in lines {
            let size = fonts.measure(style, line);
            let pos = ((vec2(width, 480.0) - size) / 2.0 + vec2(0.0, below)).round();
            let (white, black) = (rgb(255, 255, 255), rgb(0, 0, 0));
            fonts.draw(&mut self.batch, style, line, pos, white, Some(black));
        }
        let screen = Transform::ortho(0.0, width, 0.0, 480.0).matrix();
        context.draw(&mut self.batch.all(), &screen);
    }

    pub fn load_fonts(&mut self, context: &mut Gfx2dContext, vfs: &Vfs, filename: &str) {
        if let Some(mut old) = self.fonts.replace(Fonts::load(vfs, filename)) {
            old.delete(context);
        }
    }

    /// Menus and the menu cursor, in interface units (480 high).
    #[allow(clippy::too_many_arguments)]
    fn render_interface(
        &mut self,
        context: &mut Gfx2dContext,
        state: &InterfaceState,
        world: &World,
        (camera_center, zoom, width): (Vec2, f32, f32),
        elapsed: f64,
    ) {
        // the interface is 480 high and as wide as the game (`GameWidth`), letterboxed
        let sh = context.viewport().height;
        let fonts = self
            .fonts
            .get_or_insert_with(|| Fonts::load(&Vfs::new(), ""));
        fonts.prepare(context, sh / 480.0);

        self.batch.clear();
        let spectator = state.player.filter(|_| state.spectating);
        let advanced = state.cvars.bool("sv_advancedspectator");
        let hud = hud::HudState {
            world,
            player: match spectator {
                Some(_) if advanced => state.follow,
                Some(_) => None,
                None => state.player,
            },
            spectator,
            camera: camera_center,
            zoom,
            size: vec2(width, 480.0),
            mouse: state.mouse,
            menus: state.menus,
            chat: state.chat,
            follow: state.follow,
            alpha: state.alpha,
            elapsed,
            messages: state.messages,
            layout: &self.layout,
            pixel: 480.0 / sh,
            cvars: state.cvars,
            con_info: state.con_info.as_ref(),
            recording: state.recording,
            wide_cut: state.wide_cut,
            snap_offered: state.snap_offered,
            no_crosshair: state.no_crosshair,
        };
        hud::render_hud(&mut self.batch, fonts, &self.sprites, &hud);
        if let Some(minimap) = self.minimap.as_ref().filter(|_| state.menus.minimap) {
            let mut h = hud::Hud::new(&mut self.batch, fonts, &self.sprites, hud.size);
            minimap::render_minimap(&mut h, &hud, minimap);
        }
        let frags = state.menus.frags;
        let stats = state.menus.stats.then_some(state.stats);
        scoreboard::render_scoreboard(&mut self.batch, fonts, &self.sprites, &hud, frags, stats);
        interface::render_menu_backgrounds(&mut self.batch, &self.sprites, state);
        hud::render_texts(&mut self.batch, fonts, &self.sprites, &hud);
        interface::render_menu_texts(&mut self.batch, fonts, &self.sprites, state);
        let screen = Transform::ortho(0.0, width, 0.0, 480.0).matrix();
        context.draw(&mut self.batch.all(), &screen);
    }

    pub fn load_map(
        &mut self,
        context: &mut Gfx2dContext,
        vfs: &Vfs,
        map: &MapFile,
        background: Option<[(u8, u8, u8); 2]>,
    ) {
        self.map.delete(context);
        self.map = MapGraphics::new(context, vfs, map, background);
        self.rebuild_minimap(context, map);
    }

    /// `soldier` alone in `target`, the game's units from `min` to `max` filling it (the
    /// settings' preview).
    pub fn render_soldier_preview(
        &mut self,
        context: &mut Gfx2dContext,
        soldier: &Soldier,
        target: &RenderTarget,
        (min, max): (Vec2, Vec2),
    ) {
        self.batch.clear();
        render_soldier(
            soldier,
            &self.soldier_graphics,
            &self.sprites,
            &mut self.batch,
            1.0,
            false,
        );
        context.begin_target(target);
        let view = Transform::ortho(min.x, max.x, min.y, max.y).matrix();
        context.draw(&mut self.batch.all(), &view);
        context.end_target();
        self.batch.clear();
    }

    /// Everything on the GPU goes (the session's over).
    pub fn delete(&mut self, context: &mut Gfx2dContext) {
        self.map.delete(context);
        if let Some(minimap) = self.minimap.take() {
            minimap.delete(context);
        }
        for texture in self.sheets.drain(..) {
            context.delete_texture(texture);
        }
        if let Some(fonts) = &mut self.fonts {
            fonts.delete(context);
        }
    }

    /// Draws the minimap of `map` again, for the window's current pixel size.
    pub fn rebuild_minimap(&mut self, context: &mut Gfx2dContext, map: &MapFile) {
        if let Some(old) = self.minimap.take() {
            old.delete(context);
        }
        let sh = context.viewport().height;
        self.minimap =
            minimap::Minimap::new(context, &mut self.map, &mut self.batch, map, 480.0 / sh);
    }

    /// The game's sprites, the interface ones from the `interface` (`ui_style`) where it
    /// has them.
    pub fn load_sprites(&mut self, context: &mut Gfx2dContext, vfs: &Vfs, interface: &str) {
        let mut main: Vec<SpriteInfo> = Vec::new();
        let mut intf: Vec<SpriteInfo> = Vec::new();

        self.layout = layout::Layout::load(vfs, interface);
        let custom = self.layout.name.clone();

        let add_to = |v: &mut Vec<SpriteInfo>, fname: &str| {
            let (name, image) = load_image(vfs, "", fname);
            v.push(SpriteInfo::new(name, image, vec2(1.0, 1.0), None));
        };
        // a custom interface's own images, green keyed when they are bitmaps
        let add_interface = |v: &mut Vec<SpriteInfo>, fname: &str| {
            let file = fname.trim_start_matches("interface-gfx/");
            let own = custom
                .as_ref()
                .filter(|_| layout::CUSTOM_FILES.contains(&file))
                .map(|name| format!("custom-interfaces/{name}/{file}"))
                .filter(|path| vfs.find_with_extensions(path, IMAGE_EXTENSIONS).is_some());
            let (name, image) = load_image(vfs, "", own.as_deref().unwrap_or(fname));
            let key = (name.ends_with(".bmp") || name.ends_with(".gif")).then(|| rgb(0, 255, 0));
            v.push(SpriteInfo::new(name, image, vec2(1.0, 1.0), key));
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
                    .for_each(|f| add_interface(&mut intf, f)),
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
                    // GetImageScale: by path, then by directory, then the default
                    let dir = sprite_info.name.rsplit_once('/').map_or("", |(d, _)| d);
                    let scale = match data.get(&sprite_info.name).or_else(|| data.get(dir)) {
                        None => default_scale,
                        Some(scale) => f32::from_str(scale).unwrap_or(default_scale),
                    };

                    sprite_info.pixel_ratio = vec2(scale, scale);
                }
            }
        }

        // a custom interface's images scale by its own mod.ini, 1 by default
        if let Some(name) = &custom {
            let dir = format!("custom-interfaces/{name}/");
            let ini = vfs
                .read_to_string(&format!("{dir}mod.ini"))
                .ok()
                .and_then(|text| Ini::load_from_str(&text).ok());
            let scales = ini.as_ref().and_then(|ini| ini.section(Some("SCALE")));
            for sprite_info in intf.iter_mut() {
                let Some(file) = sprite_info.name.strip_prefix(&dir) else {
                    continue;
                };
                let scale = scales
                    .and_then(|s| s.get(file).or_else(|| s.get("DefaultScale")))
                    .and_then(|v| f32::from_str(v).ok())
                    .unwrap_or(1.0);
                sprite_info.pixel_ratio = vec2(scale, scale);
            }
        }

        for texture in self.sheets.drain(..) {
            context.delete_texture(texture);
        }
        let main = Spritesheet::new(context, 8, FilterMethod::Trilinear, main);
        let intf = Spritesheet::new(context, 8, FilterMethod::Trilinear, intf);
        self.sheets = [&main, &intf]
            .iter()
            .flat_map(|sheet| sheet.textures.iter().copied())
            .collect();

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
